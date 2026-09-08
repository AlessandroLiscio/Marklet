# Marklet — agent instructions

Lightweight, open-source Markdown viewer. Tauri 2 (Rust core) + Svelte 5 (chrome only).
Targets Windows 11 (primary) and Linux/WSL2 (development).

This repository is **Claude Code only**. Agent configuration lives in `.claude/`.
Do not create `.github/skills/`, `.github/agents/`, `.github/instructions/`, or any
`*.agent.md` / `*.prompt.md` / `*.instructions.md` file. `.github/` holds workflows and
nothing else.

## Two editions — read this before deciding anything

Marklet ships **two products from one codebase**. They have different rules, and applying
one product's rule to the other is the most expensive mistake available here.
[`docs/editions.md`](docs/editions.md) is the canonical table.

| | **Marklet Lite** | **Marklet** (full) |
|---|---|---|
| Installer ceiling | 2\_936\_013 B (2.8 MiB) — **a promise** | 12\_582\_912 B (12 MiB) — **a tripwire** |
| Cold start ceiling | 1200 ms median | 1800 ms median |
| Governing rule | lightweight wins over every feature | make it good; the ceiling only catches accidents |

**In lite, lightweight wins.** When a feature and the budget disagree, the budget wins and
the feature is cut or moved to full. That constraint is the reason the project exists.

**Full is deliberately not bound by it.** It may spend bytes on webfonts, motion, tabs,
regex search, CJK detection, an updater — and on being genuinely well designed. Its ceiling
is there to catch a 40 MB dependency added by mistake, not to shape decisions, and it is
cheap to raise because nobody was promised it.

What full is *not* allowed to do is put weight on the boot path. Everything heavy stays
behind `import()` in **both** editions: a document with no diagram must not pay for the
diagram engine in either product.

Both switches default to lite — `MARKLET_EDITION` for the bundle, `--features full` for the
binary. A feature that forgets to declare its edition lands in the constrained build and
fails the gate loudly, rather than landing in the generous one unnoticed.

Before adding **any** dependency — crate or npm package — read
`.claude/skills/size-budget/SKILL.md`, say which edition it is for, and report the measured
xz size in your diff receipt. A dependency without that measurement is an incomplete change
in either edition.

## Architecture invariants

1. **`src-tauri/src/render/**` must not import `tauri`.** It runs headless for `MD_HTML=1`.
2. **The rendered document never passes through Svelte.** It is `innerHTML` on a plain
   `<article id="doc">`. Svelte owns the chrome — sidebar, outline, settings, search — and
   nothing else. A 3 MB markdown file must not go through a reactive renderer.
3. **Nothing heavy is imported at module top level in `src/main.ts`.** Mermaid, KaTeX,
   highlight.js and CodeMirror are reached only through `import()` gated on document
   content or on entering edit mode. A read-only session downloads zero bytes of them.
4. **The frontend has no filesystem, shell, or http permission.** See
   `src-tauri/capabilities/default.json`. Every privileged action is a named, validated
   `#[tauri::command]` in `src/ipc.rs`; path validation lives there and in `protocol.rs`,
   nowhere else.
5. **Markdown is never converted to another representation and back.** No serializer. The
   editor is CodeMirror 6 over the markdown source; edits are byte splices through
   `splice_range`, never whole-file rewrites.
6. **`data-l` is a contract.** Every block-level element carries its 1-based source line.
   Scroll sync, scroll restore, the outline and block editing all read it. Changing how it
   is emitted breaks four features at once — see `.claude/skills/render-pipeline/SKILL.md`.

## Working agreements

- **No agent runs git.** No commit, stage, push, or branch. Changes stay in the working
  tree; the main thread handles git with Alessandro's explicit approval.
- **One file, one agent, one wave.** If a handoff names a file, that file belongs to that
  agent until the wave ends.
- **Every change returns a diff receipt** with file paths and line ranges.
- **Read the named skills first.** Do not work from memory on repository conventions.

## Branch workflow

`main` is protected by `.github/rulesets/protect-main.json`: no direct pushes, no
force-pushes, pull request required, squash merge only, branch must be up to date, and five
required checks. There are no bypass actors — an emergency fix goes through a branch like
everything else.

So: branch, push, open a PR, let CI run, squash-merge. Branch names are
`<type>/<short-description>`. Commit and PR-title conventions are in
[`CONTRIBUTING.md`](CONTRIBUTING.md); the trigger model and what each check does are in
[`docs/ci-cd.md`](docs/ci-cd.md).

**The required-check contexts are load-bearing strings.** Each is
`<job id in main.yml> / <name: of the job inside the reusable workflow>`. Renaming either
side blocks every merge until the ruleset is updated and re-applied.

Never add an AI attribution trailer to a tag message.

## Development

```bash
./scripts/setup-linux.sh   # once per machine (needs sudo)
./scripts/dev.sh           # dev loop; sets the WEBKIT_* vars WSLg requires
npm test                   # vitest
cargo test                 # render fixtures and Rust units
cargo clippy -- -D warnings
```

Windows binaries come from GitHub Actions `windows-latest`, not from cross-compilation.
Tauri's own documentation calls Linux to Windows cross-compilation a last resort. Push a
branch to get an `.exe`.

## Two traps that cost an afternoon each

**No console.** The binary is built with `#![windows_subsystem = "windows"]`, so `--help`
and `MD_HTML=1` print into the void unless `AttachConsole(ATTACH_PARENT_PROCESS)` runs
first. Every CLI path must also exit before `tauri::Builder::build()`.

**White window under WSLg.** Without `WEBKIT_DISABLE_DMABUF_RENDERER=1` and
`WEBKIT_DISABLE_COMPOSITING_MODE=1`, Tauri paints an empty window with no error.
`scripts/dev.sh` sets both.

## Skills

| Skill | Read it before |
|---|---|
| `size-budget` | adding any dependency, or touching `vite.config.ts` chunking |
| `render-pipeline` | changing anything under `src-tauri/src/render/**` |
| `tauri-ipc` | adding or changing a `#[tauri::command]` |
| `win-integration` | touching the registry, file association, or the NSIS hooks |
| `release` | cutting a version or changing the CI matrix |
| `version-sync-check` | auditing release bookkeeping — read-only, safe any time |

Design work additionally uses the `ui-ux-pro-max` plugin, enabled for this project in
`.claude/settings.json`.

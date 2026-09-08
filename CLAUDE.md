# Marklet — agent instructions

Lightweight, open-source Markdown viewer. Tauri 2 (Rust core) + Svelte 5 (chrome only).
Targets Windows 11 (primary) and Linux/WSL2 (development).

This repository is **Claude Code only**. Agent configuration lives in `.claude/`.
Do not create `.github/skills/`, `.github/agents/`, `.github/instructions/`, or any
`*.agent.md` / `*.prompt.md` / `*.instructions.md` file. `.github/` holds workflows and
nothing else.

## The one rule that outranks the others

**Lightweight wins.** When a feature and the size budget disagree, the budget wins and the
feature gets cut or deferred. The budget is enforced by `.github/workflows/size-gate.yml`,
not by good intentions:

| Artifact | Installer ceiling |
|---|---|
| `marklet-setup.exe` (full) | 5\_033\_165 B (4.8 MiB) |
| `marklet-lite-setup.exe` (no Mermaid) | 3\_984\_589 B (3.8 MiB) |

Cold start ceiling: 1200 ms median of 5 runs on `windows-latest`.

Before adding **any** dependency — crate or npm package — read
`.claude/skills/size-budget/SKILL.md` and report the measured xz size in your diff receipt.
Adding a dependency without that measurement is an incomplete change.

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

Design work additionally uses the `ui-ux-pro-max` plugin, enabled for this project in
`.claude/settings.json`.

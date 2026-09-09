# Contributing to Marklet

Read [`CLAUDE.md`](CLAUDE.md) first. It states the architecture invariants, and most of them
exist because breaking one costs the project its reason to exist.

## The rule that shapes everything else

Marklet ships **two products from one codebase**, and they have different rules. Read
[`docs/editions.md`](docs/editions.md) before proposing a feature — it decides which edition
your change belongs to, and that decides almost everything else.

**Marklet Lite** is worth writing only because it stays small and starts fast; there are
already good heavyweight Markdown editors. **Marklet (full)** is deliberately not bound by
that, and may spend bytes on typography, motion, tabs, regex search and an updater.

Either way:

**Any new dependency arrives with its measured compressed size, and its edition.** Not an
estimate — the number from the command:

```bash
npm pack <pkg> --pack-destination /tmp >/dev/null && tar -xOf /tmp/<pkg>-*.tgz | xz -9 | wc -c
```

Put it in the pull request's **Size impact** section. A change that adds a dependency without
that number is incomplete, however well the code works.

| Ceiling, enforced by CI | Marklet Lite | Marklet (full) |
|---|---:|---:|
| Installer | **2.8 MiB** — a promise | **12 MiB** — a tripwire for accidents |
| Cold start, median | 1200 ms | 1800 ms |

Lite has roughly 1.45 MiB uncommitted. Spending some of it is fine; spending it silently is
not. Full has slack by design — but weight there still has to earn its place, and **nothing
heavy goes on the boot path in either edition**.

`.claude/skills/size-budget/SKILL.md` has the measurement commands, the current allocation,
and a table of alternatives already rejected on size — check it before proposing `syntect`,
`tantivy`, `ammonia`, `clap` or `rusqlite`, all of which are also blocked mechanically in
`src-tauri/deny.toml`.

## Getting set up

```bash
git clone https://github.com/AlessandroLiscio/Marklet
cd Marklet
./scripts/setup-linux.sh   # once per machine; needs sudo for the system packages
./scripts/dev.sh           # dev loop
```

`scripts/dev.sh` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` and
`WEBKIT_DISABLE_COMPOSITING_MODE=1`. Under WSL2 those are not optional — without them Tauri
paints an empty white window and prints no error.

Windows binaries come from GitHub Actions, not from cross-compilation. Tauri's own
documentation calls Linux-to-Windows cross-compilation a last resort. Push a branch and take
the artifact.

## Branches and commits

`main` is protected: no direct pushes, no force-pushes, pull request required, squash merge
only, branch must be up to date. Work on a branch.

Branch names: `<type>/<short-description>`, e.g. `feat/vault-search`, `fix/footnote-backlink`.

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body: why, not what — the diff already says what>

<footers>
```

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`, `revert`.
Subject is imperative, lowercase, no trailing period, 72 characters or fewer. A breaking
change is marked **twice**: `!` after the type or scope, and a `BREAKING CHANGE:` footer.

`git config commit.template .gitmessage` gives you the shape in your editor.

The pull request title becomes the squash-merge subject, so it follows the same rules.

## What CI will check

| Check | What it runs |
|---|---|
| `code-quality` | `cargo fmt`, `clippy -D warnings`, `svelte-check`, `check:imports`, `contrast` |
| `code-security` | `cargo-deny`, Trivy, TruffleHog |
| `code-test` | `cargo test` and `--no-default-features`, on Linux and Windows; `vitest` |
| `size-gate` | both installers against their ceilings, with the delta commented on the PR |

Run the fast ones locally before pushing:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run check && npm run check:imports && npm run contrast
cargo test --manifest-path src-tauri/Cargo.toml
npm test
./scripts/check-windows.sh    # if you touched anything under #[cfg(windows)]
```

`check-windows.sh` type-checks the Windows-only code from Linux in about twelve
seconds. Without it the only way to learn whether that code compiles is to push and
wait for `code-test / Tests (windows-latest)`; two round trips were spent that way
before it existed, one of them on an error that is *only* wrong on Windows.

It type-checks and nothing more — it does not link, does not run, and cannot tell you
whether a registry write lands. Only `windows-latest` answers that.

[`docs/ci-cd.md`](docs/ci-cd.md) explains the trigger model and why each check lives where
it does.

## Two invariants worth restating

**Nothing heavy in a static import.** Mermaid, KaTeX, highlight.js and CodeMirror are reached
only through `import()` gated on document content or on entering edit mode. `check:imports`
fails the build otherwise — the symptom of getting this wrong is not an error but a cold
start that quietly got slower.

**The rendered document never passes through Svelte.** It is `innerHTML` on a plain
`<article id="doc">`. Svelte owns the chrome. A 3 MB markdown file must not go through a
reactive renderer.

## Reporting a security issue

Not here. See [SECURITY.md](SECURITY.md) — privately, through a security advisory.

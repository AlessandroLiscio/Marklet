# Size budget

The numbers CI enforces. [`editions.md`](editions.md) explains why there are two sets of
them; `.claude/skills/size-budget/SKILL.md` holds the measurement commands and the decision
procedure. This file is the reference table.

## Ceilings

Marklet ships two products. **The two ceilings mean different things**, and confusing them
defeats both.

| Gate | Marklet Lite | Marklet (full) | Enforced by |
|---|---:|---:|---|
| Installer | **2.8 MiB** (2\_936\_013 B) | **12 MiB** (12\_582\_912 B) | `size-gate.yml` |
| Cold start, median of 5 | 1200 ms | 1800 ms | `release.yml` |
| `src/styles/**`, gzipped | 12 KiB (12\_288 B) | not gated | `npm run size` |

Lite's ceiling is **a promise**: it is why the project exists, and raising it is a decision
taken on a measurement, in its own change. Full's is **a tripwire**: it catches a dependency
added by mistake, does not shape design decisions, and is cheap to raise because nobody was
promised it.

## Where the numbers came from

The plan estimated 2.6–3.2 MB for the Rust binary alone and set the ceilings from that. The
first CI build measured the actual empty shell:

| Artifact, v0.1.0 foundation | Measured |
|---|---:|
| `Marklet_0.1.0_x64-setup.exe` | **903.75 KiB** |
| `Marklet_0.1.0_amd64.deb` | 1.42 MiB |
| `Marklet_0.1.0_amd64.AppImage` | 75.62 MiB |

The estimate was wrong by roughly 3×, in our favour: `default-features = false` on `tauri`,
`opt-level = "z"` with fat LTO and `strip`, and NSIS/LZMA compressing better than assumed.

Lite's ceiling was tightened onto that measurement. A gate with 4× headroom cannot fail, and
a gate that cannot fail is decoration.

## What lite spends

| Component | Installer | On disk | Loaded when |
|---|---:|---:|---|
| Rust exe + Tauri shell | **904 KiB measured** | 7.0–8.0 MB | always |
| Svelte chrome, CSS, tokens | ~60 KB | ~200 KB | always |
| highlight.js subset | ~28 KB | ~95 KB | document has a fenced code block, after first paint |
| KaTeX + subset fonts | ~200 KB | ~430 KB | document has math delimiters |
| CodeMirror 6 | ~180 KB | ~600 KB | `F2` / `F3` / `Ctrl+E` |
| WebView2 bootstrapper | 0 | 0 | never on Windows 11 |
| **Committed so far** | **≈1.35 MiB** | | |
| **Headroom to 2.8 MiB** | **≈1.45 MiB** | | for render core, vault, watcher, platform, export |

Comfortable, not generous. The Rust side is where most of the remaining growth will come
from, and none of it is written yet.

## What full adds

| Component | Installer | Why lite cannot have it |
|---|---:|---|
| Mermaid | ~750 KB | 27% of lite's entire ceiling for one feature |
| `regex-search` (ripgrep stack) | +1.2–1.8 MB | literal multi-term AND covers the common case at no cost |
| `full-encodings` (`encoding_rs` + `chardetng`) | ~+500 KB | BOM/UTF-8/UTF-16/cp1252 covers virtually all Western `.md` |
| Bundled typography | 100–300 KB per variable font | lite is system fonts only |
| Motion | tens of KB | lite has CSS state changes and nothing else |
| `tauri-plugin-updater` | ~200 KB plus a signing key | lite users download manually |
| `html-to-image` (PNG of any block) | ~30 KB | unreliable with webfonts, and lite has none |

Expected full total: roughly 4–5 MiB, against a 12 MiB tripwire. The gap is intentional
slack, not a target to fill.

## Build switches

Weight is gated **where it lives**, which is not always Rust. That is the whole reason there
are two switches rather than one.

| Switch | Read by | Default | Controls |
|---|---|---|---|
| `MARKLET_EDITION=full\|lite` | Vite (`vite.config.ts`) | `lite` | the bundle — Mermaid aliased to a stub, full-only UI eliminated |
| `--features full` | Cargo (`src-tauri/Cargo.toml`) | off | the binary — `full-encodings`, `regex-search` |

Mermaid is **not** a cargo feature, and the distinction cost a CI failure to learn. Its
750 KB are in the frontend bundle; a cargo feature would have gated nothing while looking
like it gated the largest single item in the budget.

Both switches default to lite. A feature that forgets to declare its edition lands in the
constrained build and fails the gate loudly, rather than landing in the generous one and
never being noticed.

## Honesty about the mdview comparison

`mdview` ships a 2 MB installer. The comparable artifact is **Marklet Lite**, not the full
edition — comparing full to it would be meaningless, since full deliberately carries things
mdview does not have.

Lite has a real chance of coming in under 2 MB. Nothing claims that yet, and nothing will
until it is measured with the features in. A tool that calls itself lightweight and quietly
redefines the word is not one.

## On the AppImage

75.62 MiB, because AppImage bundles the entire GTK and WebKit stack. It is not gated and
should not be compared to the Windows number; `.deb` at 1.42 MiB is the honest Linux figure
for a system that already has those libraries. If AppImage stays this expensive it is worth
reconsidering as a shipped target.

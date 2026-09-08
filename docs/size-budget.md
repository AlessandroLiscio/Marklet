# Size budget

The numbers CI enforces. `.claude/skills/size-budget/SKILL.md` holds the measurement
commands and the decision procedure; this file is the reference table.

## Ceilings

| Artifact | Ceiling | Bytes | Enforced by |
|---|---:|---:|---|
| `marklet-setup.exe` (full) | 4.8 MiB | 5\_033\_165 | `size-gate.yml` |
| `marklet-lite-setup.exe` | 3.8 MiB | 3\_984\_589 | `size-gate.yml` |
| Cold start, median of 5 on Windows | 1200 ms | — | `release.yml` |
| `src/styles/**`, gzipped | 12 KiB | 12\_288 | `ci.yml` via `npm run size` |

## Where the budget goes

| Component | Installer | On disk | Loaded when |
|---|---:|---:|---|
| Rust exe | 2.6–3.2 MB | 7.0–8.0 MB | always |
| Svelte chrome, CSS, tokens | ~60 KB | ~200 KB | always |
| highlight.js subset | ~28 KB | ~95 KB | document has a fenced code block, after first paint |
| KaTeX + subset fonts | ~200 KB | ~430 KB | document has math delimiters |
| CodeMirror 6 | ~180 KB | ~600 KB | `F2` / `F3` / `Ctrl+E` |
| Mermaid | ~750 KB | ~3,570 KB | document matches ` ```mermaid ` — full build only |
| WebView2 bootstrapper | 0 | 0 | never on Windows 11 |
| **Full** | **≈3.8–4.5 MB** | ≈12–13 MB | |
| **Lite** | **≈3.1–3.7 MB** | ≈8.5–9.5 MB | |

Mermaid is about 37% of the full installer for one feature. That is the whole reason `lite`
is a first-class artifact from the same CI run.

## Honesty about the comparison

`mdview` ships a 2 MB installer. We do not match it, and the comparison is not like for
like: a Tauri 2 hello-world is already 2.5–3 MB before any feature exists, and `mdview` has
no bundled Mermaid, no vault layer and no editor. The number to compare against ours is the
lite build.

Saying so in the README is deliberate. A "lightweight" tool that quietly redefines
lightweight is not one.

## Cargo features

| Feature | Default | Cost | What it buys |
|---|---|---:|---|
| `mermaid` | on | +750 KB | diagram rendering; off produces the lite artifact |
| `full-encodings` | off | ~+500 KB | `encoding_rs` + `chardetng` for CJK detection |
| `regex-search` | off | +1.2–1.8 MB | full regex vault search via the ripgrep stack |

Deferring a feature behind a cargo feature is the third option when the gate fails. The
first two are gating an import and moving weight to the full build only. Raising a ceiling
is not on the list.

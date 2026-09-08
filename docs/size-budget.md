# Size budget

The numbers CI enforces. `.claude/skills/size-budget/SKILL.md` holds the measurement
commands and the decision procedure; this file is the reference table.

## Ceilings

| Artifact | Ceiling | Bytes | Enforced by |
|---|---:|---:|---|
| `marklet-setup.exe` (full) | 3.5 MiB | 3\_670\_016 | `size-gate.yml` |
| `marklet-lite-setup.exe` | 2.8 MiB | 2\_936\_013 | `size-gate.yml` |
| Cold start, median of 5 on Windows | 1200 ms | — | `release.yml` |
| `src/styles/**`, gzipped | 12 KiB | 12\_288 | `ci.yml` via `npm run size` |

### Where these came from

The original plan set 4.8 / 3.8 MiB from an estimate of 2.6–3.2 MB for the Rust binary
alone. The first CI build measured the actual empty shell:

| Artifact, v0.1.0 foundation | Measured |
|---|---:|
| `Marklet_0.1.0_x64-setup.exe` | **903.75 KiB** |
| `Marklet_0.1.0_amd64.deb` | 1.42 MiB |
| `Marklet_0.1.0_amd64.AppImage` | 75.62 MiB |

The estimate was wrong by roughly 3×, in our favour: `default-features = false` on `tauri`,
`opt-level = "z"` with fat LTO and `strip`, and NSIS/LZMA compressing better than assumed.

The ceilings were tightened to match. A gate with 4× headroom cannot fail, and a gate that
cannot fail is decoration. 3.5 MiB leaves about 2.6 MiB for every remaining feature — ample
against the ~1.2 MiB the lazy chunks are expected to cost — while still biting if something
unplanned lands.

**The lite build now has a real chance of coming in under `mdview`'s 2 MB.** That is not
promised anywhere yet, and it will not be until it is measured with the features in.

### On the AppImage

75.62 MiB, because AppImage bundles the entire GTK and WebKit stack. It is not gated and
should not be compared to the Windows number; `.deb` at 1.42 MiB is the honest Linux figure
for a system that already has those libraries. If AppImage stays this expensive it is worth
reconsidering as a shipped target.

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

## Build switches

Weight is gated where it actually lives, which is not always Rust.

| Switch | Where | Default | Cost | What it buys |
|---|---|---|---:|---|
| `MARKLET_LITE=1` | environment, read by Vite | off | +750 KB | Mermaid. Set it and Vite aliases the package to `src/lib/rich/mermaid-stub.ts`, so the chunk is never emitted — this is what produces the lite artifact |
| `full-encodings` | cargo feature | off | ~+500 KB | `encoding_rs` + `chardetng` for CJK detection |
| `regex-search` | cargo feature | off | +1.2–1.8 MB | full regex vault search via the ripgrep stack |

Mermaid is **not** a cargo feature, and the distinction matters. Its 750 KB are in the
frontend bundle; a cargo feature would have gated nothing while looking like it gated
something. Gate weight where it lives.

Deferring behind a build switch is the third option when the gate fails. The first two are
gating an import and moving weight to the full build only. Raising a ceiling is not on the
list.

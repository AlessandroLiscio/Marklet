---
name: size-budget
description: The installer size and cold-start budget for Marklet, how to measure both, and what gets cut when they are breached. Read before adding any Rust crate or npm package, before touching manualChunks in vite.config.ts, before adding a font or an icon set, and before changing the release profile in Cargo.toml.
---

# Size budget

Marklet exists because `mdview` is 2 MB and starts in under a second. If we lose that, the
project has no reason to exist — there are a dozen good heavyweight Markdown editors already.
This skill is how the constraint stays real instead of aspirational.

## The numbers

| Artifact | Ceiling | Bytes |
|---|---|---|
| `marklet-setup.exe` (full, with Mermaid) | 4.8 MiB | 5\_033\_165 |
| `marklet-lite-setup.exe` (no Mermaid) | 3.8 MiB | 3\_984\_589 |
| Cold start, median of 5 on `windows-latest` | 1200 ms | — |
| `src/styles/**` combined, gzipped | 12 KiB | 12\_288 |

`.github/workflows/size-gate.yml` fails the pull request when any of these is exceeded and
comments the byte delta against `main`. Do not raise a ceiling to make a build pass. Raising
a ceiling is a decision for Alessandro, taken on a measurement, in its own change.

## Where the budget currently goes

| Component | Installer | Loaded when |
|---|---|---|
| Rust exe | 2.6 – 3.2 MB | always |
| Svelte chrome + CSS + tokens | ~60 KB | always |
| highlight.js subset | ~28 KB | document has a fenced code block, after first paint |
| KaTeX + subset fonts | ~200 KB | document has math delimiters |
| CodeMirror 6 | ~180 KB | F2 / F3 / Ctrl+E pressed |
| Mermaid | ~750 KB | document matches /```mermaid/ — full build only |

Mermaid is 37% of the full installer for one feature. That is why `lite` exists as a
first-class artifact from the same CI run, and why the Mermaid chunk has its own gate.

## Measuring

The installer uses NSIS with LZMA compression, so `xz -9` approximates what a dependency
actually costs a user far better than the raw file size does.

```bash
# What one npm package costs, compressed
npm pack <pkg> --pack-destination /tmp >/dev/null && \
  tar -xOf /tmp/<pkg>-*.tgz | xz -9 | wc -c

# What a built chunk costs
xz -9c dist/assets/mermaid-*.js | wc -c

# All stylesheets together, against the 12 KiB gate
cat src/styles/**/*.css | gzip -9 | wc -c

# The stripped binary
cargo build --release && du -b src-tauri/target/release/marklet

# What a crate adds: build twice and diff
cargo build --release && cp src-tauri/target/release/marklet /tmp/before
# ... add the dependency ...
cargo build --release && du -b /tmp/before src-tauri/target/release/marklet
```

`npm run size` runs the full report. Put its output in your diff receipt.

## Rules

1. **Every new dependency arrives with its measured xz size.** A change that adds a
   dependency without that number is incomplete, regardless of how well the code works.
2. **Nothing heavy is imported at module top level in `src/main.ts`.** Mermaid, KaTeX,
   highlight.js and CodeMirror are reached only through `import()`, gated on document
   content or on entering edit mode. A read-only session on a plain document must show zero
   of these chunks in its network waterfall — there is a wdio assertion for exactly this.
3. **Zero webfonts** except the KaTeX subset. One bundled variable font is 100–300 KB: a
   Mermaid-sized decision taken by accident. Font stacks are system-only.
4. **Icons are inline SVG**, never an icon font and never an icon package.
5. **The release profile in `Cargo.toml` is not negotiable.** `opt-level = "z"`,
   `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true` are worth 30–40% of
   the stripped binary. Slow release builds are the price.
6. **Prefer a Rust crate over a JS package for anything that runs once per document**, and a
   JS package over a Rust crate for anything that runs in the webview. Rust code that ships
   is paid for on every launch; a lazy JS chunk is paid for only by the users who need it.

## Already-rejected alternatives — do not re-propose without new measurements

| Wanted | Rejected | Cost |
|---|---|---|
| HTML sanitizing | `ammonia` | +1.2 MB (pulls `html5ever`) |
| Syntax highlighting | `syntect` | +2–3 MB exe, 100–200 ms dump deserialize |
| Syntax highlighting | `shiki` | 603 KB core + 467 KB `onig.wasm` + grammars |
| Vault search | `tantivy` | +4–6 MB exe |
| Vault search | `minisearch` / `lunr` | small, but the whole vault must enter webview RAM |
| CLI parsing | `clap` / `tauri-plugin-cli` | +350–450 KB exe |
| Persistence | `rusqlite` bundled | +1.5 MB |
| WYSIWYG editing | ProseMirror / Tiptap / Milkdown | ~400 KB **and** a lossy markdown serializer |
| Diagram pan/zoom | `panzoom` | replaced by ~60 lines of pointer events |
| Block screenshot | `html-to-image` | ~30 KB and unreliable with webfonts |

## When the gate fails

In order, stop at the first that works:

1. Is the new weight reachable without an `import()` gate? Gate it.
2. Can it move behind a build switch that is off by default? Move it — but put the switch
   where the weight lives. `full-encodings` and `regex-search` are cargo features because
   they are Rust. Mermaid is `MARKLET_LITE=1`, read by Vite, because its 750 KB are in the
   frontend bundle; a cargo feature there would have gated nothing while appearing to.
3. Can it move to the `full` build only, the way Mermaid did? Move it.
4. Cut the feature and say so in the pull request.

Raising the ceiling is not on this list.

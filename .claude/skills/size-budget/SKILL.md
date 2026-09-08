---
name: size-budget
description: The installer size and cold-start budget for Marklet, how to measure both, and what gets cut when they are breached. Read before adding any Rust crate or npm package, before touching manualChunks in vite.config.ts, before adding a font or an icon set, and before changing the release profile in Cargo.toml.
---

# Size budget

**Marklet ships two products, and only one of them is on a diet.** Getting this backwards
either cripples the full edition or quietly ruins the lite one, so start here:

| | **Marklet Lite** | **Marklet** (full) |
|---|---|---|
| Installer ceiling | 2.8 MiB — 2\_936\_013 B | 12 MiB — 12\_582\_912 B |
| Cold start, median of 5 on Windows | 1200 ms | 1800 ms |
| What the ceiling **means** | a promise | a tripwire for accidents |

**Lite** exists because `mdview` is 2 MB and starts in under a second. Without that, the
project has no reason to exist — there are a dozen good heavyweight Markdown editors already.
Every feature argues against the ceiling, and arguments win unless something is holding a
scale. That is this skill.

**Full** is deliberately outside the lightweight rule. It may spend bytes on typography,
motion, tabs, regex search, CJK detection, an updater. Its ceiling catches a dependency added
by mistake; it does not shape design decisions, and raising it is cheap because nobody was
promised it. See `docs/editions.md`.

`src/styles/**` gzipped must stay under 12 KiB **in the lite build**. The full build's
stylesheets are not gated.

`.github/workflows/size-gate.yml` fails the pull request when a ceiling is exceeded and
comments the byte delta against `main`. Do not raise the **lite** ceiling to make a build
pass — that is a decision for Alessandro, taken on a measurement, in its own change.

## The one rule that applies to both

**Nothing heavy on the boot path, in either edition.** Full has a looser budget, not a
licence to import Mermaid eagerly. A document with no diagram must not pay for the diagram
engine in either product, and `npm run check:imports` fails the build either way.

## Where the lite budget goes

| Component | Installer | Loaded when |
|---|---|---|
| Rust exe + Tauri shell | **904 KiB measured** at v0.1.0, before features | always |
| Svelte chrome + CSS + tokens | ~60 KB | always |
| highlight.js subset | ~28 KB | document has a fenced code block, after first paint |
| KaTeX + subset fonts | ~200 KB | document has math delimiters |
| CodeMirror 6 | ~180 KB | F2 / F3 / Ctrl+E pressed |

That leaves roughly 1.6 MiB of the 2.8 MiB ceiling for the Rust side to grow into — the
render core, vault, watcher, platform integration and export are all still to be written.
It is comfortable, not generous.

## What full adds on top

| Component | Cost | Why lite cannot have it |
|---|---|---|
| Mermaid | ~750 KB | 27% of lite's entire ceiling for one feature |
| `regex-search` (ripgrep stack) | +1.2–1.8 MB | literal multi-term AND covers the common case at no cost |
| `full-encodings` (`encoding_rs` + `chardetng`) | ~+500 KB | BOM/UTF-8/UTF-16/cp1252 covers virtually all Western `.md` |
| Bundled typography | 100–300 KB per variable font | lite is system fonts only |
| Motion | tens of KB | lite has CSS state changes and nothing else |
| `tauri-plugin-updater` | ~200 KB plus a signing key | lite users download manually |

Adding one of these to **lite** is not a size discussion, it is a product change. Say so and
ask.

The 904 KiB figure is measured from the first CI build, not estimated — the original plan
guessed 2.6–3.2 MB for the binary and was wrong by roughly 3×, which is why the ceilings
were tightened. When you quote a number here, quote a measured one; `docs/size-budget.md`
records where each came from.

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

1. **Every new dependency arrives with its measured xz size *and its edition*.** A change
   that adds a dependency without both is incomplete, regardless of how well the code works.
2. **Nothing heavy is imported at module top level in `src/main.ts` — in either edition.**
   Mermaid, KaTeX, highlight.js and CodeMirror are reached only through `import()`, gated on
   document content or on entering edit mode. A read-only session on a plain document must
   show zero of these chunks in its network waterfall, in full as well as lite; there is a
   wdio assertion for exactly this, and `npm run check:imports` fails the build.
3. **Zero webfonts in lite** except the KaTeX subset — one bundled variable font is
   100–300 KB, a Mermaid-sized decision taken by accident. Lite's font stacks are
   system-only. Full may bundle curated typography, lazily.
4. **Icons are inline SVG**, never an icon font and never an icon package, in either edition.
   This one is about render cost and flash-of-unstyled-icon, not only bytes.
5. **The release profile in `Cargo.toml` is not negotiable.** `opt-level = "z"`,
   `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true` are worth 30–40% of
   the stripped binary. Slow release builds are the price.
6. **Prefer a Rust crate over a JS package for anything that runs once per document**, and a
   JS package over a Rust crate for anything that runs in the webview. Rust code that ships
   is paid for on every launch; a lazy JS chunk is paid for only by the users who need it.

## Already-rejected alternatives

Each was rejected on cost. **Some are rejected for lite only** — the full edition can afford
what lite cannot, so re-proposing one *for full* is a legitimate conversation, not a repeat.
Do not re-propose anything for **lite** without new measurements.

| Wanted | Rejected | Cost | Rejected for |
|---|---|---|---|
| HTML sanitizing | `ammonia` | +1.2 MB (pulls `html5ever`) | **both** — `sanitize.rs` is better scoped anyway |
| Syntax highlighting | `syntect` | +2–3 MB exe, 100–200 ms dump deserialize | **both** — the cost is startup, not only size |
| Syntax highlighting | `shiki` | 603 KB core + 467 KB `onig.wasm` + grammars | lite; arguable for full if the fidelity is wanted |
| Vault search | `tantivy` | +4–6 MB exe | **both** — an index brings staleness we do not need |
| Vault search | `minisearch` / `lunr` | small, but the whole vault enters webview RAM | **both** — RSS, not bytes |
| CLI parsing | `clap` / `tauri-plugin-cli` | +350–450 KB exe | **both** — 120 lines already do it |
| Persistence | `rusqlite` bundled | +1.5 MB | **both** — two JSON files are enough |
| WYSIWYG editing | ProseMirror / Tiptap / Milkdown | ~400 KB **and** a lossy markdown serializer | **both** — the serializer is the disqualifier, not the size |
| Diagram pan/zoom | `panzoom` | ~60 lines of pointer events replace it | **both** |
| Block screenshot | `html-to-image` | ~30 KB, unreliable with webfonts | lite; **shipped in full** for "export any block as PNG" |

Note which rows say "both". `ammonia`, `syntect`, `tantivy`, `clap` and `rusqlite` are also
denied by name in `src-tauri/deny.toml`, so they cannot return through a transitive
dependency in either edition.

## When the gate fails

In order, stop at the first that works:

1. Is the new weight reachable without an `import()` gate? Gate it. This applies in **both**
   editions — full has a looser budget, not a licence to load Mermaid eagerly.
2. Can it move behind a build switch? Move it — but put the switch **where the weight lives**.
   `full-encodings` and `regex-search` are cargo features because they are Rust. Mermaid is
   `MARKLET_EDITION`, read by Vite, because its 750 KB are in the frontend bundle; a cargo
   feature there would have gated nothing while appearing to.
3. Can it move to the **full** edition only, the way Mermaid did? Move it, and add a row to
   `docs/editions.md` — an undocumented difference between the two products is worse than
   either product having the feature.
4. Cut the feature and say so in the pull request.

Raising the ceiling is not on this list.

# Architecture

Marklet is a Tauri 2 application: a Rust core that owns parsing, the filesystem and the OS
integration, and a web frontend that owns presentation. This document records **why** each
piece is what it is, including what was rejected — that is the part that stops a decision
from being relitigated every six months.

The governing constraint is stated once and applies everywhere: **lightweight wins**. When a
feature and the size budget disagree, the feature is cut or deferred.

## The stack, and its price

| Layer | Choice | Cost | Rejected |
|---|---|---|---|
| Shell | Tauri 2.11 | 2.6–3.2 MB exe | Electron (~85 MB) |
| Markdown | `pulldown-cmark` 0.13 in Rust | ~250 KB | `comrak` (AST allocation, 2–3× slower); `markdown-it` in JS (no offset map, 30–80 ms on the cold-start path) |
| Sanitizing | in-house allowlist, ~150 LoC | 0 KB | `ammonia` (+1.2 MB via `html5ever`) |
| Highlighting | `highlight.js` subset, lazy, post-paint | ~28 KB | `syntect` (+2–3 MB exe); `shiki` (467 KB `onig.wasm` plus grammars) |
| Math | KaTeX, lazy, subset fonts | ~200 KB | `temml` (~150 KB cheaper but MathML renders inconsistently in print-to-PDF) |
| Diagrams | Mermaid, bundled, lazy | ~750 KB | fetching at runtime (breaks offline) |
| Chrome | Svelte 5 | ~10 KB | vanilla TS (more code); Preact (VDOM cost on a 5k-node tree) |
| Editor | CodeMirror 6, lazy | ~180 KB | ProseMirror/Tiptap (~400 KB **and** a lossy serializer) |
| Vault search | `walkdir` + `aho-corasick`, streaming | ~250 KB | `tantivy` (+4–6 MB); a JS index (whole vault into webview RAM) |
| CLI | hand-rolled, ~120 LoC | ~2 KB | `clap` (+350–450 KB) |
| Persistence | two JSON files, `serde_json` | 0 KB | `rusqlite` (+1.5 MB) |
| WebView2 | `downloadBootstrapper` | 0 MB | `embedBootstrapper` (+1.8 MB); `offlineInstaller` (+127 MB, tag builds only) |

## The one primitive everything rests on

`pulldown-cmark`'s `Parser::into_offset_iter()` yields the byte range of every event. From
that single iterator, in one pass, come four separate features:

- scroll sync between source and preview,
- scroll **restore** across a reflow after a font or column-width change,
- the outline,
- byte-exact block editing.

It surfaces in the HTML as `data-l="<1-based line>"` on every block-level element. Five
frontend modules read it. Anything that discards those offsets breaks all four features at
once, which is why `data-l` is documented as a contract rather than an implementation
detail.

This is also the reason the parser is in Rust rather than in JavaScript. `markdown-it` is
actually *smaller in the installer* — 38 KB compressed against ~250 KB of Rust — but it has
no offset map, and it would force a webview to exist for headless `MD_HTML=1` export.

## Why the document is not a component

The rendered markdown is `innerHTML` on a plain `<article id="doc">`, outside the Svelte
tree entirely.

A 3 MB markdown file is tens of thousands of nodes. Passing that through a reactive renderer
costs memory for the component instances and time on every update, to buy reactivity that a
static document does not use. Svelte owns the chrome — sidebar, outline, settings, search —
where state actually changes.

This is the single most consequential performance decision in the UI.

## Cold start

WebView2 initialization dominates and is not ours to optimize. Two things that are:

1. **Boot-HTML injection.** The file is parsed to HTML in Rust *before the window is
   created* and injected as `window.__MARKLET_BOOT__` through `initialization_script`. The
   first paint already has content — no IPC round trip, no spinner.
2. **Show on first frame.** The window is created `visible: false` and shown on the first
   `requestAnimationFrame` after that HTML is in the DOM. This removes the white flash that
   makes an app *feel* slow even when it is not.

Everything heavy is behind `import()` gated on document content, so a plain document pays
for none of it. Targets: 350–600 ms warm, 0.9–1.4 s truly cold, gated in CI at 1200 ms
median.

RSS is 120–180 MB. That is the Chromium floor and is identical for any WebView2 application;
the Rust process itself is 15–25 MB.

## Security model

A `.md` file from the internet is untrusted input, and it is rendered with `innerHTML` in a
process that can reach the filesystem. Four layers, each assuming the others might fail:

1. **Raw HTML is filtered by an allowlist** in `render/sanitize.rs` — eight tags, six
   attributes, everything else dropped, including all `on*` handlers and every `javascript:`
   URL. Tested against a fixture corpus of real payloads.
2. **CSP is strict**: `script-src 'self'`, no `unsafe-eval`. If Mermaid ever needs `eval`,
   it goes in a sandboxed iframe rather than weakening the app.
3. **The `marklet://` protocol canonicalizes** and rejects any path outside the document's
   directory or the vault root. Validation happens *after* canonicalization — checking a raw
   string for `..` first is a bypass waiting to happen.
4. **The capabilities file grants the frontend nothing** — no fs, no shell, no http. Every
   privileged action is a named, validated `#[tauri::command]`. An injected script can only
   call what the UI can call.

## The CLI console trap

The binary is built with `#![windows_subsystem = "windows"]`, so it has no console and
`--help` prints into the void.

Before any CLI-mode output on Windows, `AttachConsole(ATTACH_PARENT_PROCESS)` runs and
stdout/stderr are reopened; if it fails, because the process was launched from Explorer,
`--help` falls back to a message box. Every CLI path then exits **before**
`tauri::Builder::build()`, so `--install`, `--unbind`, `--help` and `MD_HTML=1` complete in
under 50 ms with no window and no WebView2 initialization.

That early exit is the entire reason a GUI binary carries a CLI at all.

## Editing without a serializer

The user asked for the Obsidian experience. Obsidian does not use ProseMirror: its Live
Preview is **CodeMirror 6** with view decorations that hide markdown syntax on lines the
cursor is not on.

That distinction decides the design. The in-memory document stays markdown, so there is no
serializer — and serializers are where hand-aligned tables get reflowed and reference links
get inlined. Edits are byte splices through `splice_range`, validated against the file's
current length, never whole-file rewrites. Round-trip fidelity is a property of the
architecture rather than a quality of the implementation.

It is also 180 KB instead of 400 KB, and the same component serves both `F2` and `F3`.

## Search without an index

`walkdir` + `aho-corasick`, streaming, no index at all.

An index means staleness, a build step, a cache that can corrupt, and disk to keep in sync.
Literal multi-term AND over 5,000 notes runs in 60–150 ms on a warm cache, and results
stream to the UI as they are found, so the first hit appears before the last file is read.
`tantivy` would cost 4–6 MB to solve a problem we do not have; a JavaScript index would pull
the entire vault into webview memory.

Full regex search sits behind the `regex-search` cargo feature, off by default.

## Development platform reality

Development happens on WSL2 against WebKitGTK; users run Windows 11 against Chromium. The
two disagree on font rasterization, MathML support and printing.

**Windows rendering is the source of truth.** WebKitGTK is for iteration speed. The
`windows-latest` smoke test runs on every pull request from phase P2 onward — not saved for
the end — and KaTeX was chosen over Temml specifically because it does not depend on
divergent MathML implementations.

Two WSL2 environment variables are mandatory and live in `scripts/dev.sh`: without
`WEBKIT_DISABLE_DMABUF_RENDERER=1` and `WEBKIT_DISABLE_COMPOSITING_MODE=1`, Tauri paints an
empty white window with no error on stdout.

## Emergency path: cross-compiling to Windows

Not supported, documented only. Tauri's own guide calls Linux-to-Windows cross-compilation a
last resort that is "not tested as much"; it needs NSIS stubs, LLVM/lld, `llvm-rc`,
`cargo-xwin` and the MSVC CRT. Use GitHub Actions `windows-latest`. A Windows host is worth
having only for interactive Explorer and registry debugging.

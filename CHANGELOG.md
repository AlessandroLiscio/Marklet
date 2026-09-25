# Changelog

All notable changes to Marklet are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every entry says what a user can now do, not which files changed. Size and cold-start deltas
against the previous release belong in the entry too — users of a tool that calls itself
lightweight care about that number, and publishing it keeps us honest.

## [Unreleased]

Nothing yet.

## [0.1.0] — 2026-09-25

First release. No previous release to compare against, so these are the
baseline every later entry is measured against:

| | Installer | Ceiling | Cold start | Ceiling |
|---|---:|---:|---:|---:|
| Marklet Lite | 1.55 MiB | 2.81 MiB | ~0.6 s | 1200 ms |
| Marklet | 3.00 MiB | 12.00 MiB | ~0.6 s | 1800 ms |

Cold start is the fastest of nine launches on `windows-latest` opening a 478 KB
document — the fastest, because filesystem warming and a shared runner's
scheduling can only make a launch slower, so the minimum is what the work costs
and an average is a measurement of the machine's mood. Three release runs of
identical bytes made that concrete: medians of 625 ms, 617 ms and 2153 ms, the
last from `5802, 4391, 2153, 1723, 1222` — a warming curve, not an application.

The **first** launch after an install is 3.3–9.8 s. WebView2 creates its user
data directory once; it is reported on its own line and never folded into the
figure above. Rendering the same document with no window is 15–19 ms.

Also published: `marklet-0.1.0.deb` (4.18 MB), `marklet-0.1.0.AppImage`
(78.78 MB — AppImage carries the whole GTK and WebKit stack and is not
comparable to the Windows figures), and `marklet-offline-setup-0.1.0.exe`
(208 MB, air-gapped machines only).

Unverified at release, and stated here rather than discovered later: nobody has
reviewed the interface visually, `PrintToPdf` has never produced a file in CI,
and the binaries are unsigned, so SmartScreen warns on first run.

### Fixed during the release itself

- The offline installer's only config difference was passed as an inline JSON
  string, which pwsh stripped the quotes out of before `npx` saw it. Now a
  committed file passed by path.
- The cold-start gate looked for the installed binary at a hard-coded
  `%LOCALAPPDATA%\Programs\Marklet\`. It installs to `%LOCALAPPDATA%\Marklet\`.
  The path is now resolved rather than assumed — the guess had never run,
  because a build leg failed ahead of it.

### Added

- **The interface, after the first person looked at it.** Links are blue and underline on
  hover rather than riding the themeable accent; tables have rounded corners; the default
  reading face is the platform's own UI sans rather than a bundled serif, with three serif
  and monospace pairings one click away in the full edition; the diagram viewer has `+` /
  `−` / reset controls, keyboard zoom and double-click-to-reset, because wheel zoom alone
  was reported doing nothing.
- **Editing, in the Obsidian sense.** `F2` shows the markdown source with its syntax hidden
  on every line the cursor is not on; `F3` splits source and preview with two-way scroll
  sync; `F4` or `Ctrl+E` opens your own editor at the cursor; `Esc` goes back to reading.
  The document stays markdown text the whole time — there is no serializer, so a
  hand-aligned table elsewhere in the file is byte-identical after a save. Writes replace one
  byte range and copy the rest through untouched, and a file that changed underneath is
  reported as a conflict rather than overwritten. The editor is 159.4 KiB xz and is
  downloaded on the first keypress that needs it, never by a read-only session.
- **Export.** `Ctrl+P` writes a PDF beside the document, `Ctrl+Shift+S` a standalone HTML
  file that opens anywhere with no network requests. Both are produced from what is on
  screen, so maths and diagrams are in them. A Mermaid diagram's fullscreen viewer can save
  itself as SVG or PNG into `assets/`.
- **`marklet --settings`** opens with the settings panel already showing, on the first frame
  rather than a moment after it.
- **Vault.** Open a folder and get a virtualized folder tree, full-text search across it,
  `[[wiki-links]]` that resolve, and a backlinks panel. Measured on 5,000 notes: tree paints
  immediately, first search result in 1 ms, search completes in 31 ms, 622 bytes of index per
  note, 10 MB of RSS for the whole vault. No search index on disk — nothing to go stale.
- **Live reload, outline and reading position.** The outline panel jumps and scroll-spies;
  the position survives a font, density or column-width change because it is anchored to the
  nearest source line rather than to a pixel offset.
- **Settings panel** for theme, accent, density, motion and column width, persisted to disk.
  The full edition adds three typefaces and four accent palettes.
- **Rich rendering, all lazily loaded**: syntax highlighting after first paint, KaTeX when a
  document contains math, and — full edition only — Mermaid with a fullscreen zoom-and-pan
  viewer. A document with none of those downloads none of them.
- **Windows file association.** `--install` adds Marklet to the "Open with" list for `.md`
  without stealing the default handler, and `--unbind` leaves zero registry keys behind.
  "Open folder as Vault" appears on directories.
- **App shell.** Double-clicking a `.md` opens it. The document is rendered in Rust
  *before the window exists* and injected as `window.__MARKLET_BOOT__`, so the first paint
  already has content — no IPC round trip, no spinner. The window is created hidden and
  shown on the first frame that has that HTML in it, which removes the white flash. Boot
  measured at 236 ms in a debug build.
- **`marklet://` asset scheme** for local images: canonicalize, then check containment. It
  refuses traversal, percent-encoded traversal, absolute paths and directories, and answers
  404 for a merely missing image so an unfinished document does not look like an attack.
- **Render core.** Markdown becomes HTML in Rust in a single pass over
  `pulldown-cmark`'s offset iterator: aligned tables, footnotes with backlinks, task lists,
  YAML frontmatter, GFM alerts, wiki-links, and `data-l` on every block — the line map that
  scroll sync, scroll restore, the outline and byte-exact editing all read. Encoding falls
  back BOM → UTF-8 → Windows-1252. 51 golden fixtures, 5 MB of prose in 68.5 ms.
- **Design system**, generated from `ui-ux-pro-max` rather than invented. Two themes plus an
  `oklch()` accent generator whose lightness/chroma constants were swept across the whole
  hue circle, so any accent a user picks clears WCAG AA — worst case 5.49:1. Lite's
  boot stylesheet gzips to 3.9 KiB against a 6 KiB gate; the full edition adds three selectable
  type pairings and four accent palettes through a dynamic import lite never fetches.
- **CLI and headless export.** `--help`, `--version`, `--benchmark`, `--install`,
  `--uninstall`, `--unbind`, `--settings`, plus `MD_HTML=1`, `MD_HTML_OUTPUT` and
  `MD_EDITOR`.
  Every headless path exits before Tauri starts: `--help` completes in 29 ms. Standalone
  HTML export inlines CSS and images as `data:` URIs and makes no network requests.
- Project foundation: Tauri 2 shell, Rust render core contract, Svelte 5 chrome, and the
  CI matrix that builds Windows and Linux artifacts from the first commit.

### Security

- **An external link leaves the application instead of replacing it.** An `http` link in a
  document used to navigate Marklet's own webview: the app became a website, with no address
  bar and no way back. That is also the shape of an attack — a markdown file that quietly
  swaps the app for a page dressed as it. Links now open in the system browser through a
  command that accepts `http` and `https` and refuses everything else, `file:` and
  `ms-msdt:` included, because the handler on the other side is the operating system's and a
  registered protocol handler can be an arbitrary program.
- **Choosing which program to run is Rust's job, not the webview's.** The editor-launch
  command takes a line and a column; `MD_EDITOR` and a fixed table decide the rest. The
  earlier shape, in which the frontend sent a program name and its arguments, would have let
  anything that achieved script execution inside the webview run an arbitrary executable.
  Naming a pasted image moved to Rust for the same reason.

### Fixed

- The Windows PDF path did not compile: `webview2-com` pins `windows-core` 0.61 and the crate
  asked for 0.62, so `.cast()` on an `ICoreWebView2` resolved to a trait the interface does
  not implement. Unified downward, which also removes a duplicate `windows` facade from the
  binary.
- `PORT` was parsed and ignored. Marklet has no HTTP server — local images are served by the
  `marklet://` scheme and live reload is a filesystem watcher — so the variable is gone
  rather than documented as working.
- Block math was emitted inside `<p>`, which browsers repair by closing the paragraph early
  — desynchronising every `data-l` after it, and with it all four features that read the
  line map.
- `<svg><script>alert(1)</script></svg>` rendered `alert(1)` as visible prose.
  `pulldown-cmark` splits inline HTML into `InlineHtml`/`Text`/`InlineHtml`, so the script
  body never reached the sanitizer. Dropped tags now swallow their contents, bounded to the
  enclosing block so an unclosed `<script>` cannot blank the rest of the document.
### Infrastructure

- **Two editions from one codebase.** Marklet Lite carries the size promise — 2.8 MiB
  installer, 1200 ms cold start, both enforced in CI. Marklet (full) is deliberately not
  bound by the lightweight rule and is where Mermaid, regex vault search, CJK detection,
  bundled typography, motion, tabs and the updater live. `docs/editions.md` has the full
  comparison; the README carries a short version.
- Size gate enforcing both ceilings, with the byte delta commented on every pull request.
  With the feature set above complete, the installers measure **1.56 MiB** (lite, ceiling
  2.81) and **3.01 MiB** (full, ceiling 12.00).
- Agent configuration under `.claude/` — six skills and four agents encoding the size
  budget, the render contract, the IPC boundary, Windows integration, the release flow and
  the release-bookkeeping audit.
- CI split into four independently required checks — quality, security, tests, size gate —
  behind an orchestrator that runs the fast three on every branch push and the expensive two
  only where a merge depends on them.
- Security scanning: `cargo-deny` (RustSec advisories, licences, banned crates), Trivy
  across both lock files, TruffleHog over full history, plus a weekly schedule so advisories
  against unchanged dependencies are still caught.
- `npm run check:imports` fails the build if Mermaid, KaTeX, highlight.js or CodeMirror ever
  reach a static import — the budget invariant a reviewer cannot see.
- Branch protection as code in `.github/rulesets/protect-main.json`, release write-targets
  in `.github/RELEASE_TARGETS.yml`, and contributor governance: PR template, three issue
  forms, commit template, `CONTRIBUTING.md`, `SECURITY.md`.

[unreleased]: https://github.com/AlessandroLiscio/Marklet/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AlessandroLiscio/Marklet/releases/tag/v0.1.0

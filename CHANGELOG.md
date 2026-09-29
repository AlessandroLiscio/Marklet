# Changelog

All notable changes to Marklet are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every entry says what a user can now do, not which files changed. Size and cold-start deltas
against the previous release belong in the entry too — users of a tool that calls itself
lightweight care about that number, and publishing it keeps us honest.

## [Unreleased]

### Fixed

- **The search match is visible in the excerpt again.** It was marked with
  `--accent-muted` as a background and nothing else, and in dark mode that
  token is `oklch(0.24 …)` — a near-black amber on a near-black panel. The
  excerpt read as one uniform block with no sign of what had matched. The match
  now takes the accent colour and a heavier weight, with the wash only giving
  the run an edge.
- **The backlinks rows say what points where.** Every row led with the note's
  H1 over a bare `[[target]] · line 3` — and the target is always the note you
  are already reading, so each row repeated the same thing and named the source
  it came from last, or not at all. The path leads now, with its line, laid out
  exactly like a search hit; the heading reads "Linked from" rather than
  "Backlinks", which is jargon that does not say which way the arrow points.

- **Scrolling the preview in split view now moves the editor with it**, and
  stops undoing itself. `visibleLine()` — the one number scroll sync, scroll
  restore and the outline's scroll-spy all read — measured each block against
  `#doc`'s own box. `#doc` scrolls with the document, so what it computed was
  the block's offset *inside* the document, which does not change when you
  scroll: it returned 0 at every position. Split view therefore told the editor
  to go to line 0 on every preview scroll and then pulled the preview to the top
  to match. Scroll restore across a reflow restored to the top for the same
  reason, and the outline highlighted the first heading forever.
- **Search results appear again.** Rust emitted each hit as a `(id, hit)`
  *tuple*, which is a JSON array, while the webview destructured `{ id, hit }`
  from it — so every hit arrived as `undefined`. A second fault sat behind it:
  the panel only accepted hits while a `running` flag was set, and the command's
  own response (which clears it) arrives before the hit events do, so even a
  correct payload would have been dropped. Hits are now matched on the search id
  Rust already tags them with.
- **Backlinks and wiki-links work at all.** Two independent reasons they could
  not: nothing in the application ever called `index_vault`, so the wiki-link
  index was never built; and `OpenedDocument` had no vault-relative path, so the
  webview passed the absolute one to a lookup keyed on the relative one. The
  index is now built in the background when a folder is opened, and the document
  carries its `rel`. The open note is also highlighted in the tree now, which
  failed on the same mismatch.

### Changed

- **One scrollbar in split view**, at the window's right edge. The editor still
  scrolls by wheel, keyboard and the line link; it no longer draws a second bar
  down the middle of the window for a second view of the same document. The
  border and the drag handle are the division that carries meaning.

- **Clicking a heading in Contents while split view is open** now moves both
  columns to it. It used to move the preview back to wherever the editor still
  was: the outline scrolls *smoothly*, a smooth scroll runs for several hundred
  milliseconds, and the two columns are linked by an echo window sized at
  150 ms. Mid-animation the link arbitrated between two moving positions and
  answered with an instant scroll, which cancelled the animation. A jump is
  announced on `document` now (`JUMP_EVENT` in `doc.ts`), so the link mutes
  itself for the animation and moves the editor to the target line directly.
- **The settings gear no longer touches the status line** in the bottom-right
  corner.

- **Opening a note from the explorer, and every wiki-link.** `open_note` declares
  its argument `rel`; `src/lib/ipc.ts` was calling it with `path`. Tauri rejects a
  call whose payload does not match the signature *before* it reaches Rust, as a
  rejected promise — and the two call sites both discarded it, so a double-click
  and a wiki-link click were silently inert. `backlinks_for` had the same fault
  (`note` called as `path`), which is why the backlinks panel was always empty.
  Both call sites now surface a failure instead of swallowing it, and
  `npm run check:ipc` cross-checks every `invoke()` against the commands so this
  class of bug cannot ship again: it type-checks on one side, compiles on the
  other, and appears only at runtime.
- **Settings is a floating card in the bottom-right corner again**, not a panel
  docked into the activity bar. The explorer and the outline change what you are
  looking at and want the document to move aside; settings is opened, changed and
  dismissed, and should sit over the document instead.

- **The accent colour, everywhere.** Settings crossed the IPC boundary in
  `kebab-case`, so the hue arrived at the webview as `accent-hue` while the
  webview read `accent_hue`. `--accent-hue` was set to the string `undefined`,
  every `oklch()` derived from it became invalid, and the consequences looked
  unrelated to each other: selected buttons lost their background, sliders fell
  back to the browser's blue whatever palette was chosen, and the settings panel
  showed "Accent hue — °" with no number. A settings file written by 0.1.0 is
  still read.
- **The settings panel no longer scrolls sideways.** Five palette names do not
  fit across a 260px column; they are a wrapping grid now, and every segmented
  control truncates rather than widening the panel.
- **The Contents panel stays open** when you click a heading, so skimming three
  sections is three clicks instead of three round trips through the activity
  bar. Its rows also span the panel's full width — the hit area used to stop
  short of both edges.
- **Search no longer wipes the file tree.** Opening the Search tab ran the full
  walk *into* the tree's own array, which replaced the lazily-listed tree;
  coming back to Files showed the walk's output instead of the folders you had
  open. The walk now builds only the index it exists for.
- **The split editor starts at the activity bar**, not at the window edge
  underneath it. With a panel open it starts after the panel.
- **The preview updates as you type in split view.** Only the file Marklet was
  *launched* with was watched, so a document opened from the explorer or the
  picker had a preview that never changed. It now re-renders when the save
  returns, about a tenth of a second after you stop typing.
- **Opening a different file while editing** rebuilds the editor for it.
  Previously the session kept the old file's text and spliced its edits into the
  old file's path.

### Changed

- **Double-click to open** in the explorer, and one click to select — the
  gesture every file explorer uses. `Enter` opens from the keyboard.
- **Non-Markdown files are listed**, dimmed and not openable. A folder shown
  with its other files stripped out looks empty or wrong to whoever put them
  there. Dotfiles stay hidden, as ignored directories already were.
- **The split-view toggle moved to the document's top-right corner** and is
  filled when it is on. It changes how the document is shown rather than what is
  beside it, which is not what the activity bar is for.

### Added

- **A draggable divider between the split columns.** Double-click, `Home` or
  `Enter` re-centres it; with it focused the arrow keys move it (`Shift` for
  larger steps).

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

### Removed

- **The Animation toggle.** It gated four transitions totalling under 200ms, and the honest
  answer to "what does this do?" was "very little". `prefers-reduced-motion` is still
  honoured — someone who has asked their system for reduced motion has asked every
  application, and should not have to ask again here.

### Fixed during the release itself

- The offline installer's only config difference was passed as an inline JSON
  string, which pwsh stripped the quotes out of before `npx` saw it. Now a
  committed file passed by path.
- The cold-start gate looked for the installed binary at a hard-coded
  `%LOCALAPPDATA%\Programs\Marklet\`. It installs to `%LOCALAPPDATA%\Marklet\`.
  The path is now resolved rather than assumed — the guess had never run,
  because a build leg failed ahead of it.

### Added

- **An activity bar, an explorer, and a way in from nothing.** A VS Code-style strip on the
  left edge switches between the file explorer, the outline and settings, and toggles the
  split editor. Launching Marklet with no file now offers the OS file picker instead of an
  empty window. The explorer roots itself at the open document's folder, so the folder tree
  is reachable by anyone who double-clicked a file — before this it existed but could only
  be reached by launching on a directory, and even then it rendered *below* the document
  rather than beside it, because nothing had ever laid it out.
- **The explorer lists one folder level at a time.** Opening a note roots the tree at that
  note's folder, which may be a home directory; walking it to show six rows is work nobody
  asked for. The full walk still runs, on the Search tab, which cannot answer without it.
- **Zoom, scoped to the document.** `Ctrl` and the wheel, or `Ctrl` `+` / `-` / `0`, or the
  slider in settings. It scales the column being read — text, spacing, images, tables — and
  leaves the sidebar, the outline and the settings panel alone. For one release it was the
  root font size, which took the whole interface with it.
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

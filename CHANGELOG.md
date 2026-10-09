# Changelog

All notable changes to Marklet are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every entry says what a user can now do, not which files changed. Size and cold-start deltas
against the previous release belong in the entry too — users of a tool that calls itself
lightweight care about that number, and publishing it keeps us honest.

## [Unreleased]

## [0.1.2] — 2026-10-09

Test release for the updater, signed with the project's update key. Full installer about 3.8 MB; lite unchanged. (0.1.1 was tagged but its build never completed: the signing secret was wrong, so it was never published.)

## [0.1.1] — 2026-10-09

Test release for the updater: the first published build that carries one. Full installer 3,827,125 B (12 MiB ceiling); lite unchanged.

### Added

- **Clicking a task-list checkbox ticks or unticks it** in the file, in reading mode (#74).

- **Updates, in the full edition**: Settings → Updates checks the release manifest and installs a newer version, verifying its signature against a key built into the app. One quiet check runs two minutes after launch, never at launch; lite has no updater (#38).

- **A *Linked from* section in the Links panel** lists the notes pointing at the open one, so `backlinks_for` has a caller again and the docs match the app (#39).

- **A richer print theme in the full edition** (`src/styles/full/print-theme.css`, loaded only at export): the document always prints light whatever the reading theme, the frontmatter becomes a title block, margins widen, pages are numbered and long code lines wrap instead of being cut (#37). Lite is unchanged.

- **A Copy button beside Edit and Split** puts the open document's whole markdown source on the clipboard, ready to paste into another app (#62).
- **Marklet's icon beside its entries in Explorer's right-click menu.** *Open
  folder as Vault* and *Open with Marklet* were the only lines in the menu with
  a blank where every neighbour — Git, VS Code, VLC — had a picture, which read
  as something half-installed. Each verb now carries the executable's own icon.
  It is a value on the existing key rather than a new key, so uninstalling
  removes it with everything else, and the Windows CI round-trip now fails if
  any of the three goes missing.
- **The keyboard sheet covers the rest of the audit.** The explorer tree's own
  keys (`Enter` opens, `Space` selects), the editing keys that were left off
  (`Shift+Tab`, `Ctrl+[` / `Ctrl+]`, `Alt+L`, `Ctrl+Enter`), clicking outside a
  diagram to close it, and that `F2` and `F3` swap straight to each other
  keeping the cursor and the undo history.

  A closing group covers what Marklet does without being asked — the document
  reloading when the file changes, each tab holding the line you left it on,
  a note opened from Explorer joining this window as a tab. None of those is a
  shortcut, and nothing else on screen says they happen.
- **An Edit button, and a sheet with every key on it.** Live-preview editing
  had existed for months and nobody had found it, because Split had a button
  in the document's corner and its sibling had nothing. An audit of the whole
  window put a number on the general case: of 66 things Marklet can do, 34 had
  no on-screen control of any kind — image paste, the snippets, most of the tab
  commands, opening a file in your own editor. The application showed about a
  third of itself.

  Two answers, because 34 buttons would be a worse window than 34 secrets.
  **Edit** joins Split in the document's top-right corner, filled when on,
  because that is the one control with an obvious home already built. Everything
  else goes on a **keyboard sheet**, on the activity bar under Export and on
  `F1`, grouped by what you are doing rather than by which file binds it. The
  snippet group is rendered from the snippet table itself, so the list of
  trigger words cannot be wrong.

  The sheet loads only when it is opened — 2.4 KiB of its own chunk, nothing on
  the boot path. `npm run check:shortcuts` fails the build when the sheet and
  the keymap disagree in either direction: a key listed but no longer bound, or
  bound but unlisted. The second is how this happened in the first place.

- **A way back to the picker, and the two exports, both on the activity bar.**
  Choosing a file or a folder used to be a one-way door: the welcome screen
  offered it, and the welcome screen is gone the moment anything is open — so
  changing your mind about which folder you were in meant restarting the
  application. `Ctrl+P` and `Ctrl+Shift+S` had the opposite problem: they
  worked, and nothing on screen said so, which for a feature nobody can guess
  is the same as not having it.

  Both now live at the **bottom of the activity bar**, below the panel
  toggles — an **Open** button and an **Export** button, each opening a
  two-row menu. Two rows rather than one action, because each is genuinely two
  commands: no native dialog picks a file *or* a folder in one call (Windows'
  folder mode is folder-only), and an export cannot start before it knows the
  format. `Ctrl+O` and `Ctrl+Shift+O` do the same from anywhere. They were
  briefly in the explorer's header and in the document's top-right corner,
  where neither could be found before you already had something open.
- **The exports ask where to save.** They used to write `<document>.pdf` and
  `<document>.html` into the document's own folder with no say in it, on the
  argument that a native dialog costs 300 KB of plugin — an argument that
  stopped being true when the file and folder pickers brought
  `tauri-plugin-dialog` in anyway. Both formats now open your system's save
  dialog, pre-filled with the document's name, and write wherever you point
  them. The dialog comes first, before `print.css` loads and before the
  document is serialized: cancelling should cost nothing.

- **Frontmatter is rendered, as a table at the top of the document.** It was
  parsed for the note's title and then dropped, so a file whose first fifteen
  lines say what it *is* — an agent definition, a skill, a prompt — opened on
  its body with that part missing. One row per key, in the order the file wrote
  them; a list is flattened to `a, b, c` whether it was written inline or as a
  block sequence. Rendered from the raw text rather than from the parsed value,
  because `serde_json::Map` is a `BTreeMap` and would have sorted
  `argument-hint` above `name`.

- **A jump points at what it went to.** Clicking a row in "Links" scrolls to
  the link and flashes it; clicking a search hit opens the note, scrolls to the
  line and flashes the **whole block** it landed in — the passage the search
  row showed, not the few characters that matched inside it. Scrolling alone
  was half an answer: a viewport that has moved does not say which of the forty
  things now on screen was the one asked for. The search hit's line was being
  dropped entirely, so a hit opened its file at the top.

- **A draggable divider between the split columns.** Double-click, `Home` or
  `Enter` re-centres it; with it focused the arrow keys move it (`Shift` for
  larger steps).

### Security

- **An export writes where the save dialog said, and nowhere else.** Letting
  the exports take a destination turned `target` into a path chosen by the
  webview, while both commands already write content the webview supplies — so
  a rendered document that got script execution could have written a file of
  its choosing anywhere the user can write, a shell profile or a startup entry
  included. The consent that makes a path outside the vault acceptable is the
  *dialog*, not the argument, so Rust now remembers what the dialog answered
  and refuses any other destination. One dialog authorises one write: the path
  is taken rather than read, so a remembered location is never left lying
  around for whatever renders next.

### Changed

- **Saving is explicit.** Edits stay in the editor until `Ctrl+S` or the new Save button; the button lights up while there are unsaved changes, and every save shows a "Saved" notice. Leaving the editor (or closing the window) with unsaved changes asks first instead of writing silently. In split view the preview now catches up on each save, not on each keystroke (#71).

- **The explorer tree takes the arrow keys** — `↑ ↓ → ← Home End` — and is one tab stop instead of one per row. Moving only selects; Enter opens (#56). **The regex toggle in search reads "Regex"**, not a bare `.*` (#55).

- **Edit, Split and Copy are stacked in a column** in the document's corner instead of a row (#61). **Tabs are labelled with the file name**, not the document's title (#63).

- **Opening a folder walks nothing.** It listed the top level lazily and then
  did two things that were not lazy at all: it built the wiki-link index, and
  the Search tab ran a full walk before its first query. Neither is needed when
  a folder is opened. The index is built when a document on screen turns out to
  contain a wiki-link that did not resolve — once per vault, and never for a
  note that has no `[[link]]` in it. The Search tab's walk is gone outright:
  nothing consumed it, because `vault::search::search` takes the root and walks
  it itself, per query. Opening a folder is now one directory listing.

- **Clicking a row in "Links" scrolls to where the link is written**; following
  it is the button on the right. Two destinations were behind one gesture, and
  the row is the easier of the two to hit by accident — a panel listing a
  document's links is a way of navigating that document, and a stray click
  should not replace it.
- **The sidebar's bottom panel is "Links" and points outward.** It listed
  *backlinks* — the notes pointing at the one being read — and now lists what
  the open note points at: wiki-links and `https://` links together, in the
  order they appear, each with its source line. Read out of the rendered
  document rather than the vault index, which means no round trip, no waiting
  for the index, and web links included — something a vault graph has no
  opinion about. A wiki-link that resolves to nothing is struck through and
  inert. `backlinks_for` is still a command; only its caller changed.

- **Rows are one line with an open glyph**, separated by a rule.

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

- **Double-click to open** in the explorer, and one click to select — the
  gesture every file explorer uses. `Enter` opens from the keyboard.
- **Non-Markdown files are listed**, dimmed and not openable. A folder shown
  with its other files stripped out looks empty or wrong to whoever put them
  there. Dotfiles stay hidden, as ignored directories already were.
- **The split-view toggle moved to the document's top-right corner** and is
  filled when it is on. It changes how the document is shown rather than what is
  beside it, which is not what the activity bar is for.

### Fixed

- **Task-list boxes are readable in dark mode.** They were the browser's greyed-out disabled checkbox; they are now drawn with a clear border and an accent fill with a tick when checked.

- **Task-list items render at the same indentation.** Only the first item of a list was pulled left, so items written at the same level looked nested.
- **Explorer's *Open folder as Vault* works while Marklet is already open.**
  It worked only with Marklet closed. A second launch is forwarded to the
  running window rather than starting another process, and every forwarded
  path was sent as a file — so a folder was read as a markdown document and
  failed. Rust now tells the two apart and sends a folder as a folder, which
  the window adopts exactly as the picker does. A relative path is also
  resolved against the directory the second launch was started in: `marklet .`
  in a terminal used to mean wherever the first instance had happened to start.
- **Launching on a folder no longer shows an empty window.** `marklet .`, and
  Explorer's *Open folder as Vault*, set the vault and open no document. The
  welcome panel is guarded on there being no vault either, so with a vault and
  no document neither it nor any side panel rendered — the folder was open, the
  tree was one click away, and the window was blank. It now opens on the
  explorer, which is what launching on a folder was asking for.
  ([#57](https://github.com/AlessandroLiscio/Marklet/issues/57))
- **Opening a diagram fullscreen now makes it bigger.** It made it smaller. A
  Mermaid `<svg>` carries `width="100%"` and a `max-width` of its own natural
  size; in the document that resolves against the reading column and the
  diagram fills it, but the viewer's stage is sized by its content, so there
  was nothing for `width: 100%` to resolve against and the SVG fell back to its
  intrinsic size — the layout engine's own units, usually much smaller than the
  column it had just been filling. The viewer then opened at 1× of that and
  called it 100%.

  It now measures the diagram and the window once, before anything is
  transformed, and opens at the scale that fills one against the other. That
  scale is what **Reset**, `0` and a double-click return to, and what the
  readout calls 100% — an SVG's intrinsic size is not a size anyone chose, so
  it is not a useful baseline. The zoom limits moved with it: eight times in
  and four times out from what you are looking at, rather than from a number
  that happens to be 1.
- **An overlay that covers the page now takes the keyboard with it.** Three
  reports, one cause: the diagram viewer and the keyboard sheet both drew a
  backdrop over the document and then let every keystroke through to it. One
  `Escape` closed the diagram *and* threw you out of live edit behind it
  ([#48](https://github.com/AlessandroLiscio/Marklet/issues/48)); `+` zoomed
  the diagram and the document underneath at the same time, because the viewer
  never checked for a modifier and the settings panel was listening for the
  same characters with one
  ([#49](https://github.com/AlessandroLiscio/Marklet/issues/49)); and
  `Ctrl+W`, `Ctrl+Tab`, `Ctrl+O` and `Ctrl+P` all still acted on the
  application while a diagram was open fullscreen over it
  ([#50](https://github.com/AlessandroLiscio/Marklet/issues/50)).

  Both overlays now claim the keyboard on the way *down* from the window,
  before the page's own handlers — which all listen on the way back up — can
  see anything. `Tab` still moves focus and `Enter` still presses a button,
  because the browser performs those itself rather than through a listener.
  `F1` closes the sheet from inside the sheet, since the application can no
  longer hear it.
- **The floating controls no longer jump when you start editing.** Entering
  live preview hides the reading column, so the page stops being scrollable,
  its scrollbar goes away, and the viewport gets that much wider — which moved
  every `position: fixed` control anchored to the right edge outward by exactly
  a scrollbar: Edit, Split, and the settings card, all together, onto the
  editor's own scrollbar. The gutter is now reserved whether or not there is
  anything to scroll, so the width is the same in every mode. The editor draws
  no bar of its own in live preview either, which is the rule split view has
  followed since it was written: one strip at the right of the window, never
  two beside each other.
- **The snippet list says what each snippet writes.** Eighteen bare words
  answered "what can I type" and not "why would I" — `fn` and `hr` say nothing
  on their own. Each now shows the first line of what it inserts, taken from
  the template itself rather than written out again, so the sheet cannot
  describe something the snippet does not do.
- **Nine documentation claims that were not true.** The README promised reading
  position remembered per file: the IPC for it exists and has no callers, and
  what ships is a per-tab line that lasts the session. It promised a backlinks
  panel: the panel lists *outgoing* links, and backlinks remain
  [#39](https://github.com/AlessandroLiscio/Marklet/issues/39). `demo.md` told
  you to scroll to the bottom, restart, and find your place — a test step that
  fails. It described the export buttons in a corner they left this morning, and
  the exports as landing beside the file when both now ask where to go. It
  claimed the explorer tree survives a switch to Search, which unmounts it, and
  offered arrow keys the tree has never had. `--help` said `--settings` opens a
  settings window; it opens a panel.

  Every one of them was found by checking the docs against the source rather
  than the other way round. Two were written this morning and left stale by the
  change that moved the exports.
- **The installer says `Marklet` where it used to say `{product_name}`.**
  Reinstalling with the application open raises a dialog asking you to close
  it; in Italian that dialog read *"Chiudi {product_name} e riprova."* The
  placeholder is not a template — `utils.nsh` inside `tauri-bundler` does a
  literal runtime substitution of `{{product_name}}`, and Tauri's Italian
  translation writes the second occurrence of each string with one brace
  instead of two, so the substitution never matches it. English has two braces
  throughout, which is why nothing looked wrong until someone reinstalled on an
  Italian system. Marklet now ships its own corrected copy of the file; when
  upstream fixes it, ours goes away rather than becoming a permanent fork of a
  translation we do not own.
- **A PDF export is the document, not a photograph of the application.** The
  first PDF anyone opened had the activity bar printed down the left edge of
  the page, a column of empty panel background beside it, the tab strip, the
  export buttons, the status bar, and — from split view — the markdown source
  in one half and the rendered document squeezed into the other.

  `print.css` hid `.sidebar` and `.outline` but not `.chrome-layer`, the fixed
  shell that *holds* them, so hiding the panels only emptied the shell instead
  of removing it. `.tabbar`, `.doc-actions`, `.divider`, `.welcome`, `.notice`
  and the editor were never listed at all, and every one of them is
  `position: fixed`, so each landed on the page wherever the viewport had put
  it. Nothing failed and nothing could: a CSS selector that matches nothing is
  silent by design, and PDF export needs a live webview, so no machine that
  runs the tests ever renders one.

  The chrome is now hidden by its container, which also covers whatever panel
  is added next; the insets the reading column carries are zeroed, so the
  document has the whole page instead of a margin the width of a rail that is
  no longer on it; and printing from live edit brings the reading column back,
  because `editor.css` hides it there and hiding the editor too would have
  left an empty page. `npm run check:print` fails the build if any of those
  selectors stops naming a real element.

- **A file opened from the picker had no place in the vault.** `rel`, the
  vault-relative path, is filled by Rust from the vault that was open *when the
  document was rendered* — so a file opened before any folder existed kept
  `rel: null` for good. That is the identity everything vault-shaped is keyed
  on: the Links panel was handed `null` and said "No note open", and the
  explorer had nothing to highlight. Double-clicking the same file in the tree
  worked, because there the vault was already open. The document is now
  re-read once `open_vault` returns.

- **Wiki-links are no longer all struck through in the "Links" panel, and
  clicking one in the document works.** Two faults, one cause: the webview was
  reading a URL prefix that does not exist. `AssetRoot::url_prefix()` is
  `marklet://localhost/` on Linux and `http://marklet.localhost/` on Windows —
  WebView2 refuses a genuinely custom scheme — while the panel matched
  `marklet://vault/`, which is what an *arbitrary test resolver* returns in
  `render/wikilink.rs`'s unit tests. It matched on neither platform, so every
  resolved link was reported as unresolved. Nothing is matched against a prefix
  now: the renderer's own `unresolved` class says whether a link resolved, and
  the path is everything after the authority.
- **Clicking a wiki-link in the document** passed `data-target` — the name as
  written, `[[demo]]` — to `open_note`, which canonicalizes what it is given
  against the vault root. A name with no extension resolves to nothing, so
  every wiki-link click failed. It uses the href the renderer already resolved.

- **Opening a folder no longer freezes the window.** The directory listing was
  lazy, but two other things were not: opening a vault built the wiki-link
  index, and the Search tab walked the whole tree — both as *synchronous*
  Tauri commands, which run on the thread that draws the window. On a tree of
  1 976 notes that is 1 358 ms of walking plus 865 ms of parsing with the
  application unresponsive, and several times that on NTFS with a scanner in
  the path. Both now run off the main thread.
- **The index cache is actually used.** `index_vault` computed the cache
  directory and then passed `None`, so every open re-read and re-parsed every
  note: 865 ms where a warm cache costs 126 ms.
- **The sidebar says when it is busy**, and which of the two things it is
  doing — an application that looks idle while it is working is one you assume
  is broken.

- **The Links panel kept showing links that were no longer in the document.**
  It re-read the document when the `path` prop changed, and a document is also
  replaced *without* its path changing — re-rendered once the index exists so
  its wiki-links resolve, or reloaded by the watcher after a save. So the
  document was fixed and the panel still listed every wiki-link struck through,
  pointing at notes visible in the tree beside it. `setDocument` announces the
  replacement now, and the panel listens for that instead.
- **Wiki-links resolve after the vault opens.** Resolution happens in Rust when
  the HTML is built, not when a link is clicked, so a document rendered before
  there was an index had every `[[link]]` struck through for good — and *every*
  document opened at launch is in that state, because `lib.rs` renders it
  before a vault exists. Opening the explorer on a file therefore showed a
  panel of dead links pointing at notes sitting right beside it. The document
  is rendered again once the index is built, keeping the reading position, and
  only when it actually holds an unresolved link.
- **The welcome screen goes when a folder is chosen**, rather than staying
  behind the explorer until a note is opened.

- **The search match is visible in the excerpt again.** It was marked with
  `--accent-muted` as a background and nothing else, and in dark mode that
  token is `oklch(0.24 …)` — a near-black amber on a near-black panel. The
  excerpt read as one uniform block with no sign of what had matched. The match
  now takes the accent colour and a heavier weight, with the wash only giving
  the run an edge.
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

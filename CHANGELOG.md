# Changelog

All notable changes to Marklet are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every entry says what a user can now do, not which files changed. Size and cold-start deltas
against the previous release belong in the entry too — users of a tool that calls itself
lightweight care about that number, and publishing it keeps us honest.

## [Unreleased]

### Added

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
  stylesheets gzip to 2119 B against a 12 KiB gate; the full edition adds three selectable
  type pairings and four accent palettes through a dynamic import lite never fetches.
- **CLI and headless export.** `--help`, `--version`, `--benchmark`, `--install`,
  `--uninstall`, `--unbind`, plus `MD_HTML=1`, `MD_HTML_OUTPUT`, `MD_EDITOR` and `PORT`.
  Every headless path exits before Tauri starts: `--help` completes in 29 ms. Standalone
  HTML export inlines CSS and images as `data:` URIs and makes no network requests.
- Project foundation: Tauri 2 shell, Rust render core contract, Svelte 5 chrome, and the
  CI matrix that builds Windows and Linux artifacts from the first commit.

### Fixed

- Block math was emitted inside `<p>`, which browsers repair by closing the paragraph early
  — desynchronising every `data-l` after it, and with it all four features that read the
  line map.
- `<svg><script>alert(1)</script></svg>` rendered `alert(1)` as visible prose.
  `pulldown-cmark` splits inline HTML into `InlineHtml`/`Text`/`InlineHtml`, so the script
  body never reached the sanitizer. Dropped tags now swallow their contents, bounded to the
  enclosing block so an unclosed `<script>` cannot blank the rest of the document.
- **Two editions from one codebase.** Marklet Lite carries the size promise — 2.8 MiB
  installer, 1200 ms cold start, both enforced in CI. Marklet (full) is deliberately not
  bound by the lightweight rule and is where Mermaid, regex vault search, CJK detection,
  bundled typography, motion, tabs and the updater live. `docs/editions.md` has the full
  comparison; the README carries a short version.
- Size gate enforcing both ceilings, with the byte delta commented on every pull request.
  The empty shell measures **904 KiB**, so lite has about 1.45 MiB left for the features
  still to land.
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

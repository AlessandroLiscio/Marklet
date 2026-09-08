<div align="center">

# Marklet

**An open-source Markdown viewer. Reading first.**

Open a `.md` file and read it like a webpage — outline, live reload, vault search,
wiki-links, diagrams, math, PDF export. Fully offline. No account, no telemetry, no paywall.

Ships in two editions: **Marklet**, which is allowed to be beautiful, and **Marklet Lite**,
which is held to 2.8 MiB by CI.

MIT licensed · Windows 11 and Linux · every feature free

</div>

---

## Why this exists

Markdown editors are heavy because editing is heavy. But most of the time you are not
editing — you are *reading* a README, a spec, a set of notes. `mdview` proved that a
reading-first viewer can be 2 MB and start in under a second, and then put its best features
behind a licence key.

Marklet drops the paywall and splits the difference the honest way, into two products. Lite
keeps the constraint, enforced by CI rather than by good intentions: a pull request that
pushes it over 2.8 MiB fails. The full edition is free of that constraint on purpose, and
spends the room on the things a budget cannot justify.

Every feature is free in both. The source is here.

## Status

**Early development.** The architecture is settled and the foundation is in place; features
land phase by phase. Not yet usable as a daily driver.

## What it will do

**Reading** — outline panel with scroll sync, live reload when an external editor saves,
reading position remembered per file, light and dark themes with an accent generator,
adjustable font, density and column width with real reflow, aligned tables, footnotes with
backlinks, local images, task lists, YAML frontmatter, syntax-highlighted code.

**Vault** — folder tree, full-text search across thousands of notes, `[[wiki-links]]` and a
backlinks panel. This is the part `mdview` does not have.

**Rich content** — Mermaid diagrams with fullscreen zoom and pan, KaTeX math. Both bundled,
both loaded only when a document actually contains one, both working with the network off.

**Editing** — `F2` live preview in the Obsidian sense: the markdown syntax hides on lines
your cursor is not on. `F3` splits source and preview with two-way scroll sync. `F4` opens
your real editor at the cursor. Pasted images are written to disk and linked.

**Export** — PDF with proper pagination, standalone single-file HTML that opens anywhere
with zero network requests, PNG or SVG of any diagram.

## Two editions

Marklet ships as **two products from one codebase**, not one product with a stripped
variant. Lite is the constrained one and carries the size promise; the full edition is
deliberately not bound by it and is allowed to spend bytes on being good.

| | **Marklet Lite** | **Marklet** |
|---|---|---|
| Installer ceiling | **2.8 MiB** — a promise | **12 MiB** — a tripwire for accidents |
| Cold start ceiling | 1200 ms median | 1800 ms median |
| Reading core, outline, live reload, position memory, reflow | ✅ | ✅ |
| Tables, footnotes, task lists, frontmatter, local images | ✅ | ✅ |
| Syntax highlighting, KaTeX math | ✅ | ✅ |
| Editing — F2 live preview, F3 dual column, F4 external editor | ✅ | ✅ |
| Vault — tree, `[[wiki-links]]`, backlinks | ✅ | ✅ |
| **Mermaid diagrams** | ✗ | ✅ with fullscreen zoom and pan |
| **Vault search** | literal, multi-term | ✅ plus full regex |
| **Encoding** | BOM, UTF-8, UTF-16, cp1252 | ✅ plus CJK auto-detection |
| **Typography** | system fonts only | ✅ curated bundled pairings, multiple palettes |
| **Motion** | CSS state changes only | ✅ considered transitions |
| **Tabs** | ✗ | ✅ |
| **Settings** | panel in the main window | ✅ dedicated window |
| **Export** | PDF, standalone HTML, diagram SVG/PNG | ✅ plus PNG of any block |
| **Auto-update** | ✗ | ✅ |

Where a row is ✅ on both sides, the two editions run the same code. Lite is never a worse
implementation — it is the absence of a feature, or a narrower one that says so.

**Marklet is the default download.** Choose Lite deliberately: when you want something small
and fast, or your Markdown never contains a diagram.

[`docs/editions.md`](docs/editions.md) has the full table, the reasoning, and how the split
is built.

### On the numbers

Lite's 2.8 MiB is measured, not guessed: the first CI build produced a **904 KiB** empty
shell, leaving about 1.9 MiB for the features. A pull request that spends more than that
fails.

We are not yet claiming to beat `mdview`'s 2 MB — no shipped number exists. When the features
are in, the honest comparison is against Lite, and it will be published either way.

## Install

Download from [Releases](../../releases):

- **Windows 11** — `marklet-setup.exe` for the full edition, or `marklet-lite-setup.exe` for
  the small one. Per-user install, no administrator rights. `marklet-offline-setup.exe`
  embeds the WebView2 runtime for air-gapped machines.
- **Linux** — `.AppImage` or `.deb`, full edition.

Marklet adds itself to the "Open with" list for `.md` rather than silently taking over the
association. Making it the default is your call.

> **Known limitation.** The Explorer context-menu entry appears under **"Show more
> options"**, not at the top level. Windows 11's modern menu requires a packaged COM
> extension with MSIX identity, which is deferred to v2.

## Command line

```
marklet <file.md>     Open a file
marklet --install     Register the file association and context menu (HKCU only)
marklet --unbind      Remove them, leaving no residual registry keys
marklet --uninstall   Unbind and remove the context-menu verbs
marklet --settings    Open with the settings panel showing
marklet --help
```

| Variable | Effect |
|---|---|
| `MD_HTML=1` | Render to standalone HTML on stdout and exit, without opening a window |
| `MD_HTML_OUTPUT` | Write that HTML to a path instead of stdout |
| `MD_EDITOR` | Command used by `F4` (default: `code -g`) |
| `PORT` | Local HTTP port; random free port by default |

## Build from source

```bash
git clone https://github.com/AlessandroLiscio/Marklet
cd Marklet
./scripts/setup-linux.sh   # once per machine; needs sudo for system packages
./scripts/dev.sh           # dev loop
```

Windows installers come from GitHub Actions `windows-latest`. Tauri's own documentation
calls Linux-to-Windows cross-compilation a last resort, so we do not rely on it — push a
branch and take the artifact.

## Architecture

Tauri 2 with a Rust core and a Svelte 5 chrome. Markdown is parsed in Rust by
`pulldown-cmark`, whose offset iterator gives the byte range of every block — one primitive
that scroll sync, scroll restore, the outline and byte-exact block editing all read.

The rendered document is deliberately outside the Svelte tree: a 3 MB markdown file must not
go through a reactive renderer. The frontend holds no filesystem, shell or http permission;
every privileged action is a validated Rust command.

`docs/architecture.md` has the reasoning, including what was rejected and why.

## Contributing

Read `CLAUDE.md` first — it states the invariants, and most of them exist because breaking
them costs the project its reason to exist. In particular: any new dependency arrives with
its measured compressed size.

## Licence

MIT. See [LICENSE](LICENSE).

<div align="center">

# Marklet

**A lightweight Markdown viewer. Reading first.**

Open a `.md` file and read it like a webpage — outline, live reload, vault search,
wiki-links, diagrams, math, PDF export. Fully offline. No account, no telemetry, no paywall.

MIT licensed · Windows 11 and Linux · every feature free

</div>

---

## Why this exists

Markdown editors are heavy because editing is heavy. But most of the time you are not
editing — you are *reading* a README, a spec, a set of notes. `mdview` proved that a
reading-first viewer can be 2 MB and start in under a second, and then put its best features
behind a licence key.

Marklet keeps the constraint and drops the paywall. Every feature is free and the source is
here. The size budget is enforced by CI, not by good intentions: a pull request that pushes
the installer over 3.5 MiB fails.

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

## Size and speed

| | Full | Lite (no Mermaid) |
|---|---:|---:|
| Ceiling enforced in CI | 3.5 MiB | 2.8 MiB |
| Measured today (empty shell, v0.1.0) | 904 KiB | 904 KiB |

Cold start is gated at 1200 ms median on Windows 11.

Mermaid is roughly 750 KB — the single largest item in the budget, for one feature. That is
why `marklet-lite-setup.exe` exists as a first-class download rather than a curiosity.

The ceilings come from measurement, not from a guess: the first CI build produced a 904 KiB
installer, so there is about 2.6 MiB of room for everything still to be written. A pull
request that spends more than that fails.

We are not claiming to beat `mdview`'s 2 MB — no shipped number exists yet. When the
features are in, the honest comparison will be against the lite build, and it will be
published either way.

## Install

Download from [Releases](../../releases):

- **Windows 11** — `marklet-setup.exe`, or `marklet-lite-setup.exe` if you do not need
  diagrams. Per-user install, no administrator rights. `marklet-offline-setup.exe` embeds
  the WebView2 runtime for air-gapped machines.
- **Linux** — `.AppImage` or `.deb`.

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

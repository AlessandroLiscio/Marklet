# The two editions

Marklet ships as **two products from one codebase**, not as one product with a stripped
variant. They answer different questions, and the difference is deliberate on both sides.

> **Status: planned.** The edition mechanism is in place and enforced by CI. Most of the
> features below are not written yet — the table is the contract each phase builds against,
> not a description of what you can download today.

## What each one is for

**Marklet Lite** is the reason the project exists. `mdview` proved a reading-first Markdown
viewer can be 2 MB and start in under a second, and that is the only reason writing another
Markdown viewer is worthwhile — there are already good heavyweight editors. Lite carries a
hard 2.8 MiB ceiling and a 1200 ms cold-start ceiling, both enforced in CI, and every feature
argues against them. It is for opening a `.md` from Explorer and reading it, now.

**Marklet** is the full edition and is **not bound by the lightweight rule.** It is allowed
to spend bytes on being good: real typography, motion, tabs, regex search across a vault, CJK
detection, an updater. Its 12 MiB ceiling is a tripwire for accidents — a dependency pulled
in by mistake — not a promise to anyone, and it is cheap to raise because nobody was promised
it.

The mistake to avoid is treating lite as "full minus things". Lite is the constrained product
with its own integrity; full is the generous one. A feature belongs in lite only if it earns
its bytes there.

## The comparison

| | **Marklet Lite** | **Marklet** |
|---|---|---|
| **Installer ceiling** | 2.8 MiB — a promise | 12 MiB — a tripwire |
| **Cold start ceiling** | 1200 ms median | 1800 ms median |
| **Reading core** — outline with scroll sync, live reload, reading position per file, font/density/column controls with real reflow | ✅ | ✅ |
| **Markdown** — aligned tables, footnotes with backlinks, task lists, YAML frontmatter, local images | ✅ | ✅ |
| **GFM alerts** — `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]`, each with its own colour and generated label | ✅ | ✅ |
| **Syntax highlighting** | ✅ highlight.js, ~22 languages | ✅ same |
| **KaTeX math** | ✅ | ✅ |
| **Mermaid diagrams** | ✗ — 750 KB, 27% of the lite ceiling for one feature | ✅ with fullscreen zoom and pan |
| **Vault** — folder tree, `[[wiki-links]]`, backlinks panel | ✅ | ✅ |
| **Vault search** | literal, multi-term AND | ✅ **plus full regex** (`regex-search`, +1.2–1.8 MB) |
| **Encoding** | BOM, UTF-8, UTF-16, Windows-1252 | ✅ **plus CJK auto-detection** (`full-encodings`, +500 KB) |
| **Editing** — F2 live preview, F3 dual column, F4 external editor, paste-image | ✅ CodeMirror 6 | ✅ same |
| **Themes** | 2 modes plus a WCAG-AA accent generator; **system fonts only** | ✅ same generator, plus the two rows below |
| **Typography** | one system stack per role | **three selectable pairings** — `editorial` (Newsreader + Inter, the default), `literary` (Cormorant Garamond + Libre Baskerville), `technical` (JetBrains Mono + IBM Plex Sans), via `[data-typeface]` |
| **Palettes** | the default accent hue | **four extra accent hues** — teal, amber, forest, violet, via `[data-palette]`. They recolour **only the accent**; background, foreground and border are identical across all of them, deliberately — reading comfort over decoration |
| **Motion** | CSS state changes only | ✅ **considered transitions**, honouring `prefers-reduced-motion` |
| **Tabs** | ✗ — one document per window | ✅ |
| **Settings** | panel inside the main window | ✅ **dedicated window** (~40 MB RSS for a second webview) |
| **Export** | PDF, standalone HTML, diagram SVG/PNG | ✅ **plus PNG of any block** and richer print themes |
| **Auto-update** | ✗ — download manually | ✅ `tauri-plugin-updater`, ~200 KB plus a signing key |

Where a row says ✅ on both sides, the two editions run the same code. The difference is never
a worse implementation in lite; it is the absence of a feature, or a narrower one that is
honest about being narrower.

### Not in either, and not a size question

- **The Windows 11 *modern* context menu.** It needs a packaged `IExplorerCommand` COM
  extension with MSIX identity. That is work and a signing requirement, not bytes. Both
  editions use the legacy verb, which appears under "Show more options".
- **Code signing.** A recurring certificate cost. Until it happens, SmartScreen warns on
  first run for both editions.
- **macOS and Android.** Out of scope for v1 entirely.

## How the split is built

The difference lives in two places, so it takes two switches. Neither can gate the other's
weight.

| Switch | Read by | Controls |
|---|---|---|
| `MARKLET_EDITION=full\|lite` | Vite (`vite.config.ts`) | the bundle: Mermaid aliased to a stub, full-only UI modules eliminated |
| `--features full` | Cargo (`src-tauri/Cargo.toml`) | the binary: `full-encodings`, `regex-search` |

**Both default to lite.** A feature that forgets to declare its edition lands in the
constrained build and fails the gate loudly, rather than landing in the generous one and
never being noticed.

In the frontend, `__MARKLET_EDITION__` is a compile-time constant, so
`if (__MARKLET_EDITION__ === 'full')` is eliminated from the lite bundle rather than shipped
and skipped. Full-only modules are reached through `import()` so the lite build tree-shakes
them out entirely.

Everything heavy stays lazily loaded in **both** editions. Full has a looser budget, not an
excuse to put Mermaid on the boot path — a document with no diagram should not pay for the
diagram engine in either product.

## Design brief for the full edition

Lite's visual restraint is a consequence of its budget: system fonts, no webfonts except the
KaTeX subset, no motion library. Full has no such excuse, and "not size-constrained" is not
the same as "unconsidered".

The bar for full is the one the `ui-ux-pro-max`, `frontend-design` and `taste-skill` skills
set — real typographic pairings, a coherent token system, motion that carries meaning rather
than decorating. Reading comfort still wins over visual interest, because this is a tool
people stare at for an hour; the difference is that full can afford to make comfort
beautiful rather than merely adequate.

Both editions keep the accessibility floor: every semantic foreground/background pair clears
4.5:1, verified by `npm run contrast`, and `prefers-reduced-motion` is honoured.

## Which one to ship to a person

Lite, if they asked for something fast and small, or they read Markdown that never contains a
diagram. Full otherwise. When in doubt, full — it is the default download, and lite is the
deliberate choice.

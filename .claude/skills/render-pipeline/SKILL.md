---
name: render-pipeline
description: The contract between Marklet's Rust render core and everything downstream — the data-l line map, the sanitizer allowlist, heading slugs, math and mermaid placeholders, wiki-link resolution. Read before changing anything under src-tauri/src/render/**, before consuming RenderedDoc in the frontend, and before adding a markdown extension.
---

# Render pipeline

One function is the source of truth for everything the user sees:

```rust
pub fn render(bytes: &[u8], opts: RenderOpts) -> RenderedDoc
```

`RenderedDoc` carries `html`, `outline`, `line_map`, `links`, `frontmatter`, `encoding`.

## Non-negotiables

**`render/**` must not import `tauri`.** It runs headless for `MD_HTML=1`, which must
complete without creating a window or initializing WebView2. If you find yourself wanting a
Tauri type in here, the design is wrong: pass a plain value or a callback through
`RenderOpts`.

**`render/**` must compile with `--no-default-features`.** There is a test for it.

## Why pulldown-cmark, and the one primitive that matters

`pulldown-cmark` 0.13 supports natively: `ENABLE_GFM`, `ENABLE_TABLES`, `ENABLE_FOOTNOTES`,
`ENABLE_TASKLISTS`, `ENABLE_STRIKETHROUGH`, `ENABLE_MATH`, `ENABLE_WIKILINKS`,
`ENABLE_HEADING_ATTRIBUTES`, `ENABLE_YAML_STYLE_METADATA_BLOCKS`. Enable exactly those.

The reason it was chosen over `comrak` is `Parser::into_offset_iter()`: it yields the byte
range of every event. Four features fall out of that one iterator in a single pass —

- scroll sync between source and preview,
- scroll **restore** across a reflow after a font or width change,
- the outline,
- byte-exact block editing.

Anything that discards those offsets breaks all four at once.

## `data-l` is a contract

Every block-level opening tag carries `data-l="<1-based source line>"`.

```html
<p data-l="12">…</p>
<table data-l="30">…</table>
<pre data-l="41"><code class="language-rust">…</code></pre>
```

Consumers that depend on it, and will break silently if it is wrong:

| Consumer | Uses it for |
|---|---|
| `src/lib/doc.ts` | building the line map, binary-searched on scroll |
| `src/lib/outline.svelte` | scroll-spy, jump-to-heading |
| `src/lib/settings/**` | restoring position by nearest anchor after a reflow |
| `src/lib/edit/split.ts` | two-way scroll sync in F3 |
| `src/lib/edit/livepreview.ts` | mapping a clicked block back to its source bytes |

Restoring scroll uses the **nearest `data-l` anchor**, never a pixel offset. A pixel offset
does not survive a font change; that is the whole point.

## Placeholders — Rust renders none of these

Rust emits a placeholder and stops. The webview finishes the job, lazily, only when the
document actually contains one.

```html
<span class="math-inline" data-tex="E = mc^2"></span>
<div class="math-block" data-tex="\int_0^1 x\,dx"></div>
<div class="mermaid" data-src="graph TD; A--&gt;B"></div>
<pre data-l="41"><code class="language-rust">…</code></pre>
```

Mermaid source is HTML-escaped and never syntax-highlighted. Code blocks are emitted plain;
`highlight.js` decorates them after first paint through an `IntersectionObserver`, so
highlighting never delays the first frame.

## Sanitizer allowlist

Raw HTML in a markdown file is untrusted input, and the app has filesystem-capable IPC
behind it. `sanitize.rs` filters `Event::Html` and `Event::InlineHtml` against an allowlist —
not a blocklist.

- **Tags:** `br`, `img`, `details`, `summary`, `sub`, `sup`, `kbd`, `mark`
- **Attributes:** `src`, `alt`, `title`, `width`, `height`, `open`
- **Dropped:** everything else, every `on*` handler, every `javascript:` URL, and every
  `data:` URL except `data:image/*`

The fixture corpus in `tests/fixtures/` includes `javascript:` hrefs, `onerror=`,
`<svg><script>`, `srcdoc` and `<iframe>`. Adding an allowed tag means adding fixtures for it.

`ammonia` is not an option — it pulls `html5ever` for roughly +1.2 MB.

## Heading slugs

GitHub-compatible, because people paste GitHub anchors: lowercase, strip punctuation,
spaces to hyphens, deduplicate with `-1`, `-2`. The outline and the in-document anchors must
agree; a mismatch shows up as an outline entry that jumps nowhere.

## Wiki-links

`Tag::Link { link_type: WikiLink, .. }` resolves through a callback supplied in `RenderOpts`.
The render core does not know what a vault is — the callback does. When it returns `None`:

```html
<a class="wikilink unresolved" data-target="Note">Note</a>
```

The unresolved state is styled, not hidden. A wiki-link pointing at a note that does not
exist yet is a normal state in a vault, not an error.

## Encoding

BOM sniff (UTF-8, UTF-16LE, UTF-16BE) → UTF-8 validation → Windows-1252 fallback. The path
taken is reported in `RenderedDoc.encoding` so the UI can show it.

This covers virtually every real-world `.md` on a Western machine. Universal CJK detection
(`encoding_rs` + `chardetng`, about +500 KB) sits behind the `full-encodings` cargo feature,
off by default, and turns on only if users report broken files.

## Fixtures

`tests/fixtures/` holds `<name>.md` paired with `<name>.expected.html`. At least 40, covering
aligned tables, footnotes with backlinks, nested and task lists, resolved and unresolved
wiki-links, inline and block math, a mermaid fence, a cp1252 file, a UTF-16LE BOM file, and
the XSS corpus.

A change to the HTML writer that updates expected output must say, in the diff receipt, which
fixtures changed and why. Regenerating them wholesale hides regressions.

## Performance

A 5 MB markdown file renders in under 120 ms. There is a timed test. `pulldown-cmark` is a
pull parser with no AST allocation, so this is comfortable — but it stops being comfortable
the moment someone collects events into a `Vec` to make a second pass convenient.

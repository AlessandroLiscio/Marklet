/**
 * Turning what is on screen into one file that still works on a machine with
 * no network and no Marklet.
 *
 * Only reachable through `import()` from the export action — it pulls the
 * KaTeX stylesheet as text and every font that stylesheet names, which is
 * roughly 190 KB nobody who is merely *reading* should download.
 *
 * The division of labour with Rust is deliberate and worth stating once:
 *
 *   - **here**: serialize the enriched DOM, and inline the CSS and fonts,
 *     because only this side has the rendered result and the bundle's own
 *     asset URLs;
 *   - **`export_html` in `src-tauri/src/ipc.rs`**: inline local `<img>` files
 *     and write the document, because only that side can read the disk.
 *
 * Neither half can do the other's job, which is why the export is a
 * round trip rather than one call.
 */

/** What the export needs from the live page. */
export interface EnrichedDocument {
  /** The document body, already enriched, ready to be wrapped by Rust. */
  bodyHtml: string;
  /** The stylesheet that markup needs, with its fonts inlined. */
  extraCss: string;
}

/**
 * Replaces every `url(...)` in `css` with a `data:` URI of the file it points
 * at.
 *
 * A standalone HTML file that still asks the network for fonts is not
 * standalone, and a `file://` page cannot fetch them at all — the export would
 * silently lose every glyph KaTeX draws with, which is most of the maths.
 *
 * A font that cannot be fetched is left as it was rather than failing the
 * export: one missing weight is a worse-looking formula, and refusing to
 * export the document over it would be a worse outcome than that.
 */
async function inlineFonts(css: string): Promise<string> {
  const urls = [...new Set([...css.matchAll(/url\(["']?([^"')]+)["']?\)/g)].map((m) => m[1]!))]
    .filter((u) => !u.startsWith('data:'));

  const replacements = await Promise.all(
    urls.map(async (url) => {
      try {
        const response = await fetch(url);
        if (!response.ok) return null;
        const buffer = new Uint8Array(await response.arrayBuffer());
        let binary = '';
        // Chunked rather than `String.fromCharCode(...buffer)`: spreading a
        // 40 KB font blows the argument limit in both engines.
        for (let i = 0; i < buffer.length; i += 8192) {
          binary += String.fromCharCode(...buffer.subarray(i, i + 8192));
        }
        const type = url.endsWith('.woff2') ? 'font/woff2' : 'application/octet-stream';
        return [url, `data:${type};base64,${btoa(binary)}`] as const;
      } catch {
        return null;
      }
    }),
  );

  let out = css;
  for (const pair of replacements) {
    if (!pair) continue;
    out = out.split(pair[0]).join(pair[1]);
  }
  return out;
}

/**
 * The enriched document, serialized.
 *
 * `root` is the live `<article id="doc">`: KaTeX has already replaced every
 * `.math-*` placeholder with real markup and Mermaid has already replaced
 * every `.mermaid` node with an inlined `<svg>`, so serializing it is all the
 * maths and diagrams the export needs. Nothing is re-rendered here.
 *
 * The clone drops anything that only makes sense in a live window — the
 * Mermaid fullscreen affordance, and the `data-l` line map, which is an
 * editor's index into a file the reader of the export does not have.
 */
export async function serializeEnrichedDocument(root: HTMLElement): Promise<EnrichedDocument> {
  const clone = root.cloneNode(true) as HTMLElement;

  for (const node of clone.querySelectorAll('[data-marklet-ui]')) node.remove();
  for (const node of clone.querySelectorAll('[data-l]')) node.removeAttribute('data-l');
  for (const node of clone.querySelectorAll('[data-tex]')) node.removeAttribute('data-tex');
  for (const node of clone.querySelectorAll('[data-src]')) node.removeAttribute('data-src');

  const katexCss = (await import('./katex.css?inline')).default;
  return { bodyHtml: clone.innerHTML, extraCss: await inlineFonts(katexCss) };
}

/**
 * Stands in for Mermaid in the lite build.
 *
 * Mermaid is roughly 750 KB compressed — about 37% of the full installer for a
 * single feature. `MARKLET_LITE=1` aliases the real package to this file in
 * `vite.config.ts`, so the chunk is never emitted at all.
 *
 * The API surface mirrors only what `src/lib/rich/mermaid.ts` calls. Keep the
 * two in step: a stub that silently lacks a method the caller uses turns a
 * missing feature into a runtime crash.
 */

export interface RenderResult {
  svg: string;
}

/** Matches mermaid's `initialize`. Accepts anything, does nothing. */
export function initialize(_config: unknown): void {
  /* no diagrams in the lite build */
}

/**
 * Renders a placeholder instead of a diagram.
 *
 * Telling the reader the source is intact and which build to install is more
 * useful than an empty box, and far more useful than a thrown error that takes
 * the surrounding document down with it.
 */
export async function render(_id: string, source: string): Promise<RenderResult> {
  const escaped = source
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  return {
    svg: `<div class="mermaid-unavailable" role="note">
  <p>Diagrams are not included in the lite build. Install the full build to render this one.</p>
  <pre><code>${escaped}</code></pre>
</div>`,
  };
}

export default { initialize, render };

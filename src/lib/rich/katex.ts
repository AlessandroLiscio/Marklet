/**
 * KaTeX, lazy.
 *
 * Rust emits a placeholder and stops — see `.claude/skills/render-pipeline/SKILL.md`:
 *
 *   <span class="math-inline" data-tex="E = mc^2"></span>
 *   <div class="math-block" data-tex="\int_0^1 x\,dx"></div>
 *
 * `renderMathIn` is called once, only when `index.ts`'s sniff finds at least
 * one of those nodes in the document. It loads the real `katex` package (its
 * own `render(tex, el, opts)` mutates `el` in place — no string round trip)
 * plus `./katex.css`, a hand-subset copy of KaTeX's stylesheet. See that
 * file's header for which font faces were kept and why.
 *
 * `data-tex` is a normal HTML attribute; the browser has already decoded its
 * entities by the time `.dataset.tex` reads it back, so there is no manual
 * unescaping to do here.
 */
import type { EnrichResult } from './types';

export async function renderMathIn(root: HTMLElement): Promise<EnrichResult> {
  const inline = Array.from(root.querySelectorAll<HTMLElement>('.math-inline'));
  const block = Array.from(root.querySelectorAll<HTMLElement>('.math-block'));
  if (inline.length === 0 && block.length === 0) return { rendered: 0 };

  const [katex] = await Promise.all([import('katex'), import('./katex.css')]);

  let rendered = 0;
  for (const el of inline) {
    if (renderOne(katex.default, el, false)) rendered++;
  }
  for (const el of block) {
    if (renderOne(katex.default, el, true)) rendered++;
  }
  return { rendered };
}

/**
 * Renders one node in place. A malformed expression falls back to showing
 * the raw TeX source as text — `throwOnError: false` already asks KaTeX to
 * render its own inline error span, but a hostile or truncated `data-tex`
 * (hand-edited markdown, a truncated placeholder) can still throw before
 * KaTeX gets that far, so the `catch` is a second, harder floor under it.
 */
function renderOne(katex: typeof import('katex').default, el: HTMLElement, displayMode: boolean): boolean {
  const tex = el.dataset['tex'] ?? '';
  try {
    katex.render(tex, el, { throwOnError: false, displayMode, output: 'html' });
    return true;
  } catch {
    el.textContent = tex;
    return false;
  }
}

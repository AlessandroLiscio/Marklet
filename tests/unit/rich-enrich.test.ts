// @vitest-environment happy-dom
/**
 * Proves the P4 acceptance requirement from `.claude/agents/ui-engineer.md` /
 * the P4 handoff: a document with none of the three rich-render placeholders
 * (fenced code, math, mermaid) must load ZERO of `hljs` / `katex` / `mermaid`.
 *
 * `hljs.ts`, `katex.ts` and `mermaid.ts` are mocked at the module boundary —
 * this test is about the *dispatch* logic in `src/lib/rich/index.ts`
 * (`enrich()` and its `has*` sniffs), not about the real KaTeX/Mermaid/hljs
 * behaviour, which is out of scope for a unit test (real chunk loading is a
 * wdio assertion, phase P9, per the handoff).
 *
 * `@vitest-environment happy-dom` overrides the shared `environment: 'node'`
 * in `vitest.config.ts` for this file only — that config is not owned by
 * this task, so it stays untouched; `happy-dom` is a devDependency added for
 * this file's `document.createElement`/`querySelector` calls and is never
 * shipped (test-only, not part of any build output).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';

const hljsMock = vi.fn(() => ({ rendered: 1 }));
const katexMock = vi.fn(async () => ({ rendered: 1 }));
const mermaidMock = vi.fn(async () => ({ rendered: 1 }));

vi.mock('../../src/lib/rich/hljs', () => ({ highlightIn: hljsMock }));
vi.mock('../../src/lib/rich/katex', () => ({ renderMathIn: katexMock }));
vi.mock('../../src/lib/rich/mermaid', () => ({ renderMermaidIn: mermaidMock }));

import { enrich, hasCode, hasMath, hasMermaid } from '../../src/lib/rich';

function rootWith(html: string): HTMLElement {
  const div = document.createElement('div');
  div.innerHTML = html;
  return div;
}

afterEach(() => {
  hljsMock.mockClear();
  katexMock.mockClear();
  mermaidMock.mockClear();
});

describe('rich/index sniffing and dispatch', () => {
  it('loads none of the three lazy modules for a plain document', async () => {
    const root = rootWith('<p data-l="1">Just words, no code, no math, no diagram.</p>');

    expect(hasCode(root)).toBe(false);
    expect(hasMath(root)).toBe(false);
    expect(hasMermaid(root)).toBe(false);

    const summary = await enrich(root);

    expect(summary).toEqual({});
    expect(hljsMock).not.toHaveBeenCalled();
    expect(katexMock).not.toHaveBeenCalled();
    expect(mermaidMock).not.toHaveBeenCalled();
  });

  it('detects a fenced code block and loads only hljs', async () => {
    const root = rootWith('<pre data-l="1"><code class="language-rust">fn main() {}</code></pre>');
    expect(hasCode(root)).toBe(true);

    const summary = await enrich(root);

    expect(summary).toEqual({ hljs: { rendered: 1 } });
    expect(hljsMock).toHaveBeenCalledWith(root);
    expect(katexMock).not.toHaveBeenCalled();
    expect(mermaidMock).not.toHaveBeenCalled();
  });

  it('detects inline and block math and loads only katex', async () => {
    const root = rootWith(
      '<p data-l="1">Energy is <span class="math-inline" data-tex="E=mc^2"></span>.</p>' +
        '<div class="math-block" data-tex="\\int_0^1 x\\,dx"></div>',
    );
    expect(hasMath(root)).toBe(true);

    const summary = await enrich(root);

    expect(summary).toEqual({ katex: { rendered: 1 } });
    expect(katexMock).toHaveBeenCalledWith(root);
    expect(hljsMock).not.toHaveBeenCalled();
    expect(mermaidMock).not.toHaveBeenCalled();
  });

  it('detects a mermaid node and loads only mermaid', async () => {
    const root = rootWith('<div class="mermaid" data-src="graph TD; A--&gt;B"></div>');
    expect(hasMermaid(root)).toBe(true);
    // The browser has already decoded the attribute entity by the time we read it.
    expect(root.querySelector<HTMLElement>('.mermaid')?.dataset['src']).toBe('graph TD; A-->B');

    const summary = await enrich(root);

    expect(summary).toEqual({ mermaid: { rendered: 1 } });
    expect(mermaidMock).toHaveBeenCalledWith(root);
    expect(hljsMock).not.toHaveBeenCalled();
    expect(katexMock).not.toHaveBeenCalled();
  });

  it('loads all three, and only those three, for a document mixing all of them', async () => {
    const root = rootWith(
      '<pre data-l="1"><code class="language-python">x = 1</code></pre>' +
        '<div class="math-block" data-tex="x^2"></div>' +
        '<div class="mermaid" data-src="graph TD; A--&gt;B"></div>',
    );

    const summary = await enrich(root);

    expect(summary).toEqual({
      hljs: { rendered: 1 },
      katex: { rendered: 1 },
      mermaid: { rendered: 1 },
    });
    expect(hljsMock).toHaveBeenCalledTimes(1);
    expect(katexMock).toHaveBeenCalledTimes(1);
    expect(mermaidMock).toHaveBeenCalledTimes(1);
  });
});

/**
 * The formatting toolbar must never cover the text being edited (#148).
 *
 * shortcut: no test layer here computes layout, so this pins the CSS that keeps
 * the two apart; measure real rectangles when a browser test runner exists.
 */
// Not `?raw`: vitest hands CSS imports over empty.
// @ts-expect-error -- @types/node is not a dependency of this project.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const read = (path: string): string => readFileSync(new URL(`../../src/${path}`, import.meta.url), 'utf8');

/** The declarations of the first rule whose selector is exactly `selector`. */
function rule(css: string, selector: string): string {
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, '');
  for (const [, selectors, body] of bare.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (selectors!.trim() === selector) return body!;
  }
  return '';
}

describe('the formatting toolbar strip', () => {
  const editor = read('lib/edit/editor.css');
  const bar = rule(read('lib/chrome/format-bar.svelte').split('<style>')[1]!, '.format-bar');

  it('sits on the right edge, not over the bottom of the text', () => {
    expect(bar).toContain('inset-inline-end: var(--edit-edge, var(--space-2));');
    expect(bar).not.toContain('inset-inline-start');
    expect(bar).not.toContain('inset-block-end');
  });

  it('is kept free by the live editor', () => {
    expect(rule(editor, 'body.marklet-mode-live .marklet-editor .cm-scroller')).toContain(
      'padding-inline-end: var(--format-strip);',
    );
  });

  it('leaves the live editor its scrollbar, and the floating buttons clear of it', () => {
    expect(editor).not.toMatch(/marklet-mode-live[^{}]*\.cm-scroller(::-webkit-scrollbar)?\s*[,{][^}]*(scrollbar-width: none|display: none)/);
    expect(rule(editor, 'body.marklet-mode-live')).toContain('--edit-edge: calc(var(--space-2) + 15px);');
    expect(read('app.svelte')).toContain('inset-inline-end: var(--edit-edge, var(--space-3));');
  });

  it('is kept free by the split preview', () => {
    expect(rule(editor, 'body.marklet-mode-split')).toContain('padding-inline-end: var(--format-strip);');
  });

  it('folds into more columns in a short window', () => {
    expect(bar).toContain('repeat(var(--format-cols, 1), 28px)');
    expect(editor).toMatch(/max-height: 720px[\s\S]*--format-cols: 2;[\s\S]*max-height: 520px[\s\S]*--format-cols: 3;/);
  });
});

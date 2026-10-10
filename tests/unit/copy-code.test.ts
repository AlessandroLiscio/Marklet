// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { addCopyButtons, codeText } from '../../src/lib/copy-code';

function page(html: string): HTMLElement {
  const root = document.createElement('article');
  root.innerHTML = html;
  return root;
}

describe('addCopyButtons', () => {
  it('adds one button to a code block, once', () => {
    const root = page('<pre><code>let a = 1;</code></pre>');
    addCopyButtons(root);
    addCopyButtons(root);
    expect(root.querySelectorAll('.copy-code')).toHaveLength(1);
    expect(root.querySelector('.copy-code')?.hasAttribute('data-marklet-ui')).toBe(true);
  });

  it('skips diagrams and a pre with no code in it', () => {
    const root = page('<pre class="mermaid">graph TD</pre><pre>plain</pre>');
    addCopyButtons(root);
    expect(root.querySelectorAll('.copy-code')).toHaveLength(0);
  });

  it('copies the code, not the button label', () => {
    const root = page('<pre><code>one\ntwo</code></pre>');
    addCopyButtons(root);
    expect(codeText(root.querySelector('pre') as HTMLElement)).toBe('one\ntwo');
  });
});

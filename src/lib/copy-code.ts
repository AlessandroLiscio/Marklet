/**
 * A Copy button on every fenced code block.
 *
 * The rendered document is plain `innerHTML`, so the button is added to the DOM
 * after each render rather than emitted by the renderer: the render contract
 * (`data-l`, the golden fixtures, the standalone export) stays exactly as it
 * was. `data-marklet-ui` is what `rich/export.ts` strips from an export, and
 * print.css hides `.copy-code`, so neither a PDF nor an exported file carries it.
 */

/**
 * Icon only, no label. Two glyphs in one button — the clipboard and a tick —
 * and `data-state` decides which one shows, so the confirmation needs no text
 * and no layout change. `aria-label` carries the words for a screen reader.
 */
const BUTTON =
  '<button type="button" class="copy-code" data-marklet-ui aria-label="Copy code" title="Copy code">' +
  '<svg class="icon-copy" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">' +
  '<rect x="5.5" y="5.5" width="8" height="8" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.4" />' +
  '<path d="M10.5 5.5V4A1.5 1.5 0 0 0 9 2.5H4A1.5 1.5 0 0 0 2.5 4v5A1.5 1.5 0 0 0 4 10.5h1.5" fill="none" stroke="currentColor" stroke-width="1.4" />' +
  '</svg>' +
  '<svg class="icon-done" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">' +
  '<path d="M3.5 8.5l3 3 6-6.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />' +
  '</svg>' +
  '</button>';

/** Adds a button to each code block that has none. Safe to call repeatedly. */
export function addCopyButtons(root: HTMLElement): void {
  for (const pre of root.querySelectorAll<HTMLElement>('pre')) {
    // A diagram is a `pre` until Mermaid replaces it; it is not code to copy.
    if (pre.classList.contains('mermaid') || pre.querySelector(':scope > .copy-code')) continue;
    if (pre.querySelector(':scope > code') === null) continue;
    pre.insertAdjacentHTML('beforeend', BUTTON);
  }
}

/** The text a block's button copies: the code, never the button's own label. */
export function codeText(pre: HTMLElement): string {
  return pre.querySelector(':scope > code')?.textContent ?? '';
}

/**
 * Handles a click inside the document. Returns whether it was a Copy click, so
 * the caller can stop looking at it.
 */
export async function onCopyClick(event: MouseEvent): Promise<boolean> {
  const button = (event.target as HTMLElement | null)?.closest<HTMLButtonElement>('.copy-code');
  const pre = button?.parentElement;
  if (!button || !pre) return false;

  const settle = (state: 'copied' | 'failed'): void => {
    button.dataset.state = state;
    button.setAttribute('aria-label', state === 'copied' ? 'Copied' : 'Could not copy');
    button.title = state === 'copied' ? 'Copied' : 'Could not copy';
    setTimeout(() => {
      delete button.dataset.state;
      button.setAttribute('aria-label', 'Copy code');
      button.title = 'Copy code';
    }, 1500);
  };

  try {
    await navigator.clipboard.writeText(codeText(pre));
    settle('copied');
  } catch {
    settle('failed');
  }
  return true;
}

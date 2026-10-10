/**
 * A Copy button on every fenced code block.
 *
 * The rendered document is plain `innerHTML`, so the button is added to the DOM
 * after each render rather than emitted by the renderer: the render contract
 * (`data-l`, the golden fixtures, the standalone export) stays exactly as it
 * was. `data-marklet-ui` is what `rich/export.ts` strips from an export, and
 * print.css hides `.copy-code`, so neither a PDF nor an exported file carries it.
 */

const BUTTON =
  '<button type="button" class="copy-code" data-marklet-ui aria-label="Copy code" title="Copy code">Copy</button>';

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

  try {
    await navigator.clipboard.writeText(codeText(pre));
    button.textContent = 'Copied';
  } catch {
    button.textContent = 'Failed';
  }
  setTimeout(() => {
    button.textContent = 'Copy';
  }, 1500);
  return true;
}

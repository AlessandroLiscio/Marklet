/**
 * The document area: `innerHTML` on a plain `<article>`, deliberately outside
 * the Svelte tree.
 *
 * A 3 MB markdown file is tens of thousands of nodes. Passing that through a
 * reactive renderer costs memory for the component instances and time on every
 * update, to buy reactivity a static document never uses. Svelte owns the
 * chrome — sidebar, outline, settings, search — where state actually changes.
 *
 * This module also owns the **line map**, the single index that four separate
 * features read: scroll sync, scroll restore across a reflow, the outline's
 * scroll-spy, and mapping a clicked block back to its source bytes.
 */
import type { BlockSpan, OpenedDocument } from './ipc';

/** The live document, or `null` before one is open. */
let current: OpenedDocument | null = null;

/** Elements carrying `data-l`, in document order — the DOM half of the line map. */
let anchors: HTMLElement[] = [];

export function currentDocument(): OpenedDocument | null {
  return current;
}

export function lineMap(): BlockSpan[] {
  return current?.line_map ?? [];
}

/**
 * Replaces the document and rebuilds the anchor index.
 *
 * `innerHTML` rather than a parsed fragment: the HTML comes from our own
 * sanitizer, which drops every script, every `on*` handler and every unsafe
 * URL before it reaches here. `innerHTML` also does not execute `<script>` it
 * inserts, so the two defences are independent rather than stacked.
 */
export function setDocument(root: HTMLElement, doc: OpenedDocument): void {
  root.innerHTML = doc.html;
  current = doc;
  anchors = Array.from(root.querySelectorAll<HTMLElement>('[data-l]'));
}

/** The source line of the block nearest the top of the viewport. */
export function visibleLine(root: HTMLElement): number {
  const top = root.getBoundingClientRect().top;
  let best = 0;
  for (const el of anchors) {
    // The first anchor whose top edge has not yet passed the viewport top.
    if (el.getBoundingClientRect().top - top >= -1) break;
    best = lineOf(el);
  }
  return best;
}

/**
 * Scrolls so the given source line is at the top.
 *
 * **Anchored to the nearest `data-l`, never to a pixel offset.** A pixel offset
 * does not survive a font, density or column-width change — which is precisely
 * when restoring position matters most. Binary search over `anchors`, which is
 * sorted because `line_map` is.
 */
export function scrollToLine(line: number, behavior: ScrollBehavior = 'auto'): void {
  const el = anchorForLine(line);
  if (el) el.scrollIntoView({ behavior, block: 'start' });
}

/** The anchor at or immediately before `line`. */
export function anchorForLine(line: number): HTMLElement | null {
  if (anchors.length === 0) return null;

  let lo = 0;
  let hi = anchors.length - 1;
  let found = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const el = anchors[mid];
    if (el === undefined) break;
    if (lineOf(el) <= line) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return anchors[found] ?? null;
}

/** Scrolls to a heading by its GitHub-compatible slug. */
export function scrollToSlug(root: HTMLElement, slug: string): boolean {
  const el = root.querySelector<HTMLElement>(`#${CSS.escape(slug)}`);
  if (!el) return false;
  el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  return true;
}

function lineOf(el: HTMLElement): number {
  const raw = el.dataset['l'];
  const n = raw === undefined ? NaN : Number.parseInt(raw, 10);
  return Number.isNaN(n) ? 0 : n;
}

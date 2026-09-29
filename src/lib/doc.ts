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
  clearFlash();
  root.innerHTML = doc.html;
  current = doc;
  anchors = Array.from(root.querySelectorAll<HTMLElement>('[data-l]'));
  // Announced because the document is outside the Svelte tree: a panel showing
  // something *about* the document — the links in it — has no other way to
  // learn that the document under it was replaced. Re-rendering to resolve
  // wiki-links is exactly that case, and the panel read the old DOM without it.
  document.dispatchEvent(new CustomEvent(DOCUMENT_EVENT));
}

/** Raised on `document` after {@link setDocument} has replaced the markup. */
export const DOCUMENT_EVENT = 'marklet-document';

/**
 * The source line of the block nearest the top of the viewport.
 *
 * **Client coordinates: the viewport's top edge is 0.** This used to measure
 * each anchor against `#doc`'s own box, which scrolls with the document — so
 * what it computed was the anchor's offset *inside* the document, a number
 * that does not change when you scroll. The first anchor always cleared the
 * test, the loop always broke on it, and the answer was always 0.
 *
 * Everything reading a position paid for that: split view told the editor to
 * go to line 0 on every preview scroll and then dragged the preview back to
 * the top to match, so scrolling the preview appeared to do nothing and then
 * undo itself. Scroll restore across a reflow restored to the top. The
 * outline's scroll-spy highlighted the first heading forever.
 *
 * Takes no argument now. The old one was the mistake: `anchors` is module
 * state, the caller's element was never needed, and passing it is what made
 * measuring against it look reasonable.
 */
export function visibleLine(): number {
  let best = 0;
  for (const el of anchors) {
    // The last anchor whose top edge has passed the viewport's.
    if (el.getBoundingClientRect().top > 1) break;
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

/**
 * Announced on `document` just before a deliberate jump moves the preview.
 *
 * `detail.line` is the source line being jumped to. Split view listens: the
 * two columns are linked by an echo-suppressed scroll handler with a 150 ms
 * window, and a *smooth* scroll outlives that window by hundreds of
 * milliseconds. Mid-animation the link would see the preview move, push the
 * editor after it, then see the editor move and pull the preview back with an
 * `auto` scroll — which cancels the animation and parks the preview where the
 * editor still was. The jump is therefore announced rather than inferred, so
 * the link can mute itself and move the editor to the target directly.
 */
export const JUMP_EVENT = 'marklet-jump';

/** The class `content.css` animates. One element carries it at a time. */
const FLASH_CLASS = 'marklet-flash';

/** How long the highlight stays up. Long enough to find with your eye, short
 *  enough that it is gone before it becomes decoration. */
const FLASH_MS = 1600;

/** Undoes whatever the last flash did — a class, or a wrapper element. */
let undoFlash: (() => void) | null = null;

/** Removes the highlight currently showing, if any. */
export function clearFlash(): void {
  undoFlash?.();
  undoFlash = null;
}

function flash(el: HTMLElement, undo: () => void): void {
  clearFlash();
  el.classList.add(FLASH_CLASS);
  const timer = setTimeout(clearFlash, FLASH_MS);
  undoFlash = () => {
    clearTimeout(timer);
    el.classList.remove(FLASH_CLASS);
    undo();
  };
}

/**
 * Scrolls an element into view and highlights it.
 *
 * `block: 'center'` rather than `'start'`: this is "look at this", not "read
 * on from here", and a thing pinned to the top edge of the window is harder to
 * find than one in the middle. The jump is announced so split view mutes its
 * scroll link — see {@link JUMP_EVENT}.
 */
export function revealElement(el: HTMLElement, line: number): void {
  if (line > 0) {
    document.dispatchEvent(new CustomEvent(JUMP_EVENT, { detail: { line } }));
  }
  el.scrollIntoView({ behavior: 'smooth', block: 'center' });
  flash(el, () => {});
}

/**
 * Scrolls to a source line and highlights the block that holds it.
 *
 * The fallback for "go here" when there is nothing finer to point at.
 */
export function revealLine(line: number): boolean {
  const el = anchorForLine(line);
  if (el === null) return false;
  revealElement(el, line);
  return true;
}

/**
 * Scrolls to a source line and highlights the first occurrence of `text`
 * inside it — a search hit, pointed at rather than merely scrolled to.
 *
 * The match is wrapped in a `<mark>` and unwrapped again when the highlight
 * expires, with `normalize()` putting the split text nodes back together, so
 * the document is byte-identical afterwards. The alternative, the CSS Custom
 * Highlight API, needs no mutation at all but is not old enough to rely on in
 * WebKitGTK, which is the development platform.
 *
 * Falls back to {@link revealLine} when the text is not found — the line moved,
 * the match spans an element boundary, or the query was a regex whose source
 * text is not what the document says.
 */
export function revealMatch(line: number, text: string): boolean {
  const block = anchorForLine(line);
  const needle = text.trim().toLowerCase();
  if (block === null || needle === '') return revealLine(line);

  const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    const value = node.nodeValue ?? '';
    const at = value.toLowerCase().indexOf(needle);
    if (at === -1) continue;

    const target = node as Text;
    const rest = target.splitText(at);
    rest.splitText(needle.length);

    const mark = document.createElement('mark');
    const parent = rest.parentNode;
    if (parent === null) return revealLine(line);
    parent.replaceChild(mark, rest);
    mark.append(rest);

    revealElement(mark, line);
    // Re-wrap the undo so it also unwraps the <mark>; `revealElement` only
    // knows how to take a class off.
    const takeClassOff = undoFlash;
    undoFlash = () => {
      takeClassOff?.();
      const owner = mark.parentNode;
      if (owner === null) return;
      owner.replaceChild(rest, mark);
      owner.normalize();
    };
    return true;
  }

  return revealLine(line);
}

/** Scrolls to a heading by its GitHub-compatible slug. */
export function scrollToSlug(root: HTMLElement, slug: string): boolean {
  const el = root.querySelector<HTMLElement>(`#${CSS.escape(slug)}`);
  if (!el) return false;

  // The heading carries `data-l` itself under the `data-l` contract, but the
  // slug's element is whatever the renderer gave the id to, so the nearest
  // one at or above it is what is asked for. Line 0 means nothing was found,
  // and announcing it would send the editor to the top of the file.
  const anchor = el.closest<HTMLElement>('[data-l]');
  const line = anchor === null ? 0 : lineOf(anchor);
  if (line > 0) {
    document.dispatchEvent(new CustomEvent(JUMP_EVENT, { detail: { line } }));
  }

  el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  return true;
}

function lineOf(el: HTMLElement): number {
  const raw = el.dataset['l'];
  const n = raw === undefined ? NaN : Number.parseInt(raw, 10);
  return Number.isNaN(n) ? 0 : n;
}

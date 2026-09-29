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

/**
 * The vault-relative path out of a resolved wiki-link's href.
 *
 * **Never matched against a fixed prefix.** The href is built from
 * `AssetRoot::url_prefix()`, which is `marklet://localhost/` on Linux and
 * `http://marklet.localhost/` on Windows — WebView2 refuses a genuinely custom
 * scheme, so Tauri serves one over http there. This function matched
 * `marklet://vault/` for one release, which is neither: it is the string an
 * *arbitrary test resolver* returns in `render/wikilink.rs`'s unit tests, and
 * it was read as if it were the format. Every resolved wiki-link failed the
 * test and the Links panel struck all of them through, while the document
 * beside it showed them live.
 *
 * So: everything after the authority, percent-decoded. The authority is not
 * inspected at all — what says a link resolved is the renderer's own
 * `unresolved` class, not the shape of the URL.
 */
export function relOfAssetHref(href: string): string | null {
  const scheme = href.indexOf('://');
  if (scheme === -1) return null;
  const slash = href.indexOf('/', scheme + 3);
  if (slash === -1) return null;

  let rel = href.slice(slash + 1);
  const cut = rel.search(/[?#]/);
  if (cut !== -1) rel = rel.slice(0, cut);
  if (rel === '') return null;

  try {
    return decodeURIComponent(rel);
  } catch {
    // A stray `%` that is not an escape. The raw form is still better than
    // calling the link unresolved.
    return rel;
  }
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

/** Takes the current highlight off. One element carries it at a time. */
let undoFlash: (() => void) | null = null;

/** Removes the highlight currently showing, if any. */
export function clearFlash(): void {
  undoFlash?.();
  undoFlash = null;
}

function flash(el: HTMLElement): void {
  clearFlash();
  el.classList.add(FLASH_CLASS);
  const timer = setTimeout(clearFlash, FLASH_MS);
  undoFlash = () => {
    clearTimeout(timer);
    el.classList.remove(FLASH_CLASS);
    // `classList.remove` leaves `class=""` behind on an element that had no
    // class of its own. Harmless to render, but it means the markup is not
    // what the renderer produced any more — and this module's rule is that a
    // highlight leaves nothing behind.
    if (el.classList.length === 0) el.removeAttribute('class');
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
  flash(el);
}

/**
 * Scrolls to a source line and highlights the **whole block** that holds it.
 *
 * The block, not the matched words inside it. Marking only the search term
 * highlighted three characters in the middle of a paragraph and left the
 * reader to work out what they were part of; the row in the search panel
 * showed a passage, and the passage is what the jump should land on. It also
 * means nothing has to be wrapped in a `<mark>` and unwrapped again — the
 * document the renderer owns is never touched.
 */
export function revealLine(line: number): boolean {
  const el = anchorForLine(line);
  if (el === null) return false;
  revealElement(el, line);
  return true;
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

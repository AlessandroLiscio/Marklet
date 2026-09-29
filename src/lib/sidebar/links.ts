/**
 * The links a document contains, read back out of the rendered document.
 *
 * **From the DOM, not from the index.** The index knows wiki-links, because
 * that is what a vault graph is made of; it does not know `https://` ones, and
 * a reader asking "what does this note point at" means both. The rendered
 * document already holds every link, already resolved, already carrying the
 * `data-l` of the block it sits in — so the answer is one `querySelectorAll`
 * with no IPC, and it cannot disagree with what is on screen.
 *
 * Pure, and therefore tested: `tests/unit/sidebar-links.test.ts`.
 */

/** What a link points at, which decides both the icon and what a click does. */
export type LinkKind = 'note' | 'unresolved' | 'web';

export interface DocLink {
  kind: LinkKind;
  /** The link's text, as written. */
  label: string;
  /**
   * A note's **vault-relative path**, a web link's URL, or — for an unresolved
   * wiki-link — the name that answered to nothing.
   *
   * The relative path rather than the wiki name, because that is what
   * `open_note` and `open_in_new_window` take. It is already in the href: the
   * renderer resolved the name when it built the document.
   */
  target: string;
  /** The source line of the block the link sits in; 0 when there is none. */
  line: number;
}

/** What `wikilink::open_tag` writes for a link it resolved. */
const VAULT_PREFIX = 'marklet://vault/';

function lineOf(el: Element): number {
  const raw = el.closest<HTMLElement>('[data-l]')?.dataset['l'];
  const n = raw === undefined ? NaN : Number.parseInt(raw, 10);
  return Number.isNaN(n) ? 0 : n;
}

/** A link, paired with the element it was read from — what the panel jumps to. */
export interface LinkRow {
  link: DocLink;
  el: HTMLAnchorElement;
}

/**
 * Every link in `root`, in document order, each with its element.
 *
 * The element is what makes "show me where this link is written" possible:
 * the panel scrolls to it and flashes it, rather than scrolling to the block
 * around it and leaving the reader to find which of its links was meant.
 * {@link collectLinks} is the same pass without the elements, which is the
 * part worth testing on its own.
 */
export function collectLinkRows(root: HTMLElement): LinkRow[] {
  const rows: LinkRow[] = [];

  for (const el of root.querySelectorAll<HTMLAnchorElement>('a')) {
    const href = el.getAttribute('href') ?? '';
    const label = (el.textContent ?? '').trim();
    const line = lineOf(el);

    if (el.classList.contains('wikilink')) {
      if (el.classList.contains('unresolved') || !href.startsWith(VAULT_PREFIX)) {
        const target = el.dataset['target'] ?? label;
        rows.push({ link: { kind: 'unresolved', label: label || target, target, line }, el });
      } else {
        const target = decodeURIComponent(href.slice(VAULT_PREFIX.length));
        rows.push({ link: { kind: 'note', label: label || target, target, line }, el });
      }
      continue;
    }

    if (/^https?:\/\//i.test(href)) {
      rows.push({ link: { kind: 'web', label: label || href, target: href, line }, el });
    }
  }

  return rows;
}

/**
 * Every link in `root`, in document order.
 *
 * Occurrences are **not** collapsed: a note linked three times is three rows,
 * each with its own line, the same way three search hits in one file are three
 * rows. "Every link in this document" is the question being answered.
 *
 * In-page anchors, `mailto:` and relative file links are left out. Nothing in
 * the application opens the last two, and a row that does nothing when clicked
 * is worse than one that is not there; an anchor is not a link to somewhere
 * else.
 */
export function collectLinks(root: HTMLElement): DocLink[] {
  return collectLinkRows(root).map((row) => row.link);
}

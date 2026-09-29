// @vitest-environment happy-dom
/**
 * `visibleLine()` — the one number four features read.
 *
 * It was wrong in a way nothing surfaced. It measured each anchor against
 * `#doc`'s own bounding box, and `#doc` scrolls with the document, so the
 * subtraction produced the anchor's offset *inside* the document — a constant.
 * The first anchor always cleared the test, the loop always broke on it, and
 * the answer was always 0, at every scroll position.
 *
 * What that cost: split view told the editor to go to line 0 on every preview
 * scroll and then pulled the preview up to match, so scrolling the preview
 * looked like it did nothing and then undid itself; scroll restore across a
 * reflow restored to the top; the outline's scroll-spy highlighted the first
 * heading forever.
 *
 * `happy-dom` gives no layout — every rect is zero — so the rects are stubbed
 * per element. That is the whole input to this function, so stubbing them is
 * the test rather than a shortcut around it.
 */
import { beforeEach, describe, expect, it } from 'vitest';

import { setDocument, visibleLine } from '../../src/lib/doc';
import type { OpenedDocument } from '../../src/lib/ipc';

/** Positions the anchors as if the document were scrolled by `scrolled` px. */
function scrollTo(root: HTMLElement, tops: number[], scrolled: number): void {
  const anchors = Array.from(root.querySelectorAll<HTMLElement>('[data-l]'));
  root.getBoundingClientRect = () => ({ top: -scrolled }) as DOMRect;
  anchors.forEach((el, i) => {
    const top = (tops[i] ?? 0) - scrolled;
    el.getBoundingClientRect = () => ({ top }) as DOMRect;
  });
}

const HTML = `
  <h1 data-l="1">One</h1>
  <p data-l="4">…</p>
  <h2 data-l="10">Two</h2>
  <p data-l="14">…</p>
  <h2 data-l="20">Three</h2>
`;

/** Each anchor's offset from the top of the document, in order.
 *  The first is 32 rather than 0 because `#doc` has `--space-8` of padding
 *  above its first child — which is why the top of a document answers 0. */
const TOPS = [32, 232, 632, 832, 1432];

let root: HTMLElement;

beforeEach(() => {
  root = document.createElement('article');
  setDocument(root, { html: HTML, line_map: [], outline: [] } as unknown as OpenedDocument);
});

describe('visibleLine', () => {
  it('is 0 at the top of the document', () => {
    scrollTo(root, TOPS, 0);
    expect(visibleLine()).toBe(0);
  });

  it('answers the last block whose top edge has passed the viewport top', () => {
    scrollTo(root, TOPS, 650);
    expect(visibleLine()).toBe(10);
  });

  it('changes as the document scrolls — the property the old form lacked', () => {
    // Measured against `#doc`'s own box, all five of these returned 0.
    const seen = [0, 250, 650, 900, 1500].map((scrolled) => {
      scrollTo(root, TOPS, scrolled);
      return visibleLine();
    });
    expect(seen).toEqual([0, 4, 10, 14, 20]);
  });

  it('answers the block exactly at the top edge, not the one before it', () => {
    scrollTo(root, TOPS, 632);
    expect(visibleLine()).toBe(10);
  });

  it('is 0 for a document with no anchors', () => {
    const empty = document.createElement('article');
    setDocument(empty, { html: '<p>no data-l</p>', line_map: [], outline: [] } as unknown as OpenedDocument);
    expect(visibleLine()).toBe(0);
  });
});

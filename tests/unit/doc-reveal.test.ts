// @vitest-environment happy-dom
/**
 * "Show me where" — the half of a jump that scrolling does not do.
 *
 * A viewport that has moved does not say which of the forty things now on
 * screen was the one asked for. So the Links panel and a search hit both point
 * at something: a link element, or the matched run of text inside a line.
 *
 * The search case mutates the document — it wraps the match in a `<mark>` —
 * and that mutation has to be exactly reversible, because the document is
 * `innerHTML` the renderer owns and nothing downstream may see a stray
 * element. That reversal is most of what is asserted here.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  clearFlash,
  revealElement,
  revealLine,
  revealMatch,
  setDocument,
} from '../../src/lib/doc';
import type { OpenedDocument } from '../../src/lib/ipc';

const HTML =
  '<p data-l="4">The vault root note. Links back here.</p>' +
  '<p data-l="9">A second paragraph with <em>emphasis</em> in it.</p>';

let root: HTMLElement;

beforeEach(() => {
  vi.useFakeTimers();
  // happy-dom has no layout, so this is a no-op it does not define.
  Element.prototype.scrollIntoView = vi.fn();
  root = document.createElement('article');
  root.id = 'doc';
  document.body.replaceChildren(root);
  setDocument(root, { html: HTML, line_map: [], outline: [] } as unknown as OpenedDocument);
});

describe('revealElement', () => {
  it('flashes the element and scrolls it into view', () => {
    const el = root.querySelector<HTMLElement>('em')!;
    revealElement(el, 9);
    expect(el.classList.contains('marklet-flash')).toBe(true);
    expect(el.scrollIntoView).toHaveBeenCalled();
  });

  it('takes the highlight off again on its own', () => {
    const el = root.querySelector<HTMLElement>('em')!;
    revealElement(el, 9);
    vi.advanceTimersByTime(2000);
    expect(el.classList.contains('marklet-flash')).toBe(false);
  });

  it('only ever highlights one thing at a time', () => {
    // Two left up at once would be two answers to one question.
    const first = root.querySelector<HTMLElement>('[data-l="4"]')!;
    const second = root.querySelector<HTMLElement>('em')!;
    revealElement(first, 4);
    revealElement(second, 9);
    expect(first.classList.contains('marklet-flash')).toBe(false);
    expect(second.classList.contains('marklet-flash')).toBe(true);
  });

  it('announces the jump, so split view mutes its scroll link', () => {
    // Without this the link sees the preview move, pulls the editor after it,
    // and answers its own answer mid-animation.
    const heard: number[] = [];
    const listener = (e: Event) => heard.push((e as CustomEvent<{ line: number }>).detail.line);
    document.addEventListener('marklet-jump', listener);
    revealElement(root.querySelector<HTMLElement>('em')!, 9);
    document.removeEventListener('marklet-jump', listener);
    expect(heard).toEqual([9]);
  });
});

describe('revealMatch', () => {
  it('wraps the matched run in a mark, case-insensitively', () => {
    expect(revealMatch(4, 'LINKS BACK')).toBe(true);
    const mark = root.querySelector('mark.marklet-flash');
    expect(mark?.textContent).toBe('Links back');
  });

  it('puts the document back exactly as it was when the flash expires', () => {
    // The document is `innerHTML` the renderer owns. A leftover `<mark>`, or
    // a paragraph left split into three text nodes, is damage.
    revealMatch(4, 'links back');
    vi.advanceTimersByTime(2000);
    expect(root.innerHTML).toBe(HTML);
    expect(root.querySelector<HTMLElement>('[data-l="4"]')!.childNodes.length).toBe(1);
  });

  it('falls back to the whole block when the text is not in it', () => {
    // A regex query, a line that moved, or a match spanning two elements.
    expect(revealMatch(4, 'nothing like this')).toBe(true);
    expect(root.querySelector('mark')).toBe(null);
    expect(root.querySelector<HTMLElement>('[data-l="4"]')!.classList.contains('marklet-flash')).toBe(
      true,
    );
  });

  it('falls back to the block for an empty term rather than marking nothing', () => {
    expect(revealMatch(9, '   ')).toBe(true);
    expect(root.querySelector('mark')).toBe(null);
  });
});

describe('revealLine', () => {
  it('flashes the block that holds the line', () => {
    expect(revealLine(9)).toBe(true);
    expect(root.querySelector<HTMLElement>('[data-l="9"]')!.classList.contains('marklet-flash')).toBe(
      true,
    );
  });

  it('answers false for a document with no anchors at all', () => {
    const empty = document.createElement('article');
    setDocument(empty, { html: '<p>none</p>', line_map: [], outline: [] } as unknown as OpenedDocument);
    expect(revealLine(3)).toBe(false);
  });
});

describe('setDocument', () => {
  it('announces the replacement, so panels reading the document re-read it', () => {
    // The Links panel showed links collected from markup that no longer
    // existed after the document was re-rendered to resolve its wiki-links.
    let heard = 0;
    const listener = () => (heard += 1);
    document.addEventListener('marklet-document', listener);
    setDocument(root, { html: '<p data-l="1">New</p>', line_map: [], outline: [] } as unknown as OpenedDocument);
    document.removeEventListener('marklet-document', listener);
    expect(heard).toBe(1);
  });

  it('drops a flash that belonged to the document being replaced', () => {
    const el = root.querySelector<HTMLElement>('em')!;
    revealElement(el, 9);
    setDocument(root, { html: HTML, line_map: [], outline: [] } as unknown as OpenedDocument);
    // The element is gone with the old markup; what matters is that the next
    // reveal is not cancelled by the previous one's timer firing late.
    clearFlash();
    expect(root.querySelector('.marklet-flash')).toBe(null);
  });
});

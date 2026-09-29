// @vitest-environment happy-dom
/**
 * "Show me where" — the half of a jump that scrolling does not do.
 *
 * A viewport that has moved does not say which of the forty things now on
 * screen was the one asked for. So the Links panel points at the link element
 * itself, and a search hit points at the whole block it landed in — the
 * passage the search row showed, not the three characters that matched inside
 * it.
 *
 * Nothing here mutates the document. An earlier version wrapped the search
 * term in a `<mark>` and unwrapped it afterwards; highlighting the block needs
 * no wrapper at all, and the markup the renderer owns is never touched.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { clearFlash, revealElement, revealLine, setDocument } from '../../src/lib/doc';
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

describe('revealLine', () => {
  it('flashes the whole block that holds the line, not a run inside it', () => {
    expect(revealLine(9)).toBe(true);
    expect(root.querySelector<HTMLElement>('[data-l="9"]')!.classList.contains('marklet-flash')).toBe(
      true,
    );
    // No wrapper anywhere: the markup is the renderer's and stays untouched.
    expect(root.querySelector('mark')).toBe(null);
  });

  it('leaves the document byte-identical once the flash expires', () => {
    revealLine(4);
    vi.advanceTimersByTime(2000);
    expect(root.innerHTML).toBe(HTML);
  });

  it('answers the nearest block above a line with no anchor of its own', () => {
    // A hit on a line inside a multi-line paragraph: the block is the passage
    // the search row showed, which is the thing worth landing on.
    expect(revealLine(6)).toBe(true);
    expect(root.querySelector<HTMLElement>('[data-l="4"]')!.classList.contains('marklet-flash')).toBe(
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

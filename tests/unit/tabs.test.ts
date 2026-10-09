/**
 * The tab strip's model.
 *
 * Pure list arithmetic, and the part of tabs worth testing: which tab is
 * showing after an open, a close, or a close of something that was not
 * showing. Every one of those is an off-by-one waiting to happen, and each
 * gets noticed by the reader as "it jumped somewhere else".
 */
import { describe, expect, it } from 'vitest';

import {
  activateTab,
  activeTab,
  closeTab,
  CLOSED_CAP,
  cycle,
  EMPTY,
  openTab,
  popClosed,
  rememberLine,
  tabLabel,
  type Tab,
  type TabState,
} from '../../src/lib/tabs';

function tab(path: string, title = path): Tab {
  return { path, title, line: 0 };
}

/** A strip of three with the middle one showing. */
function three(): TabState {
  let s = openTab(EMPTY, tab('a'));
  s = openTab(s, tab('b'));
  s = openTab(s, tab('c'));
  return activateTab(s, 1);
}

describe('openTab', () => {
  it('opens into an empty strip and shows it', () => {
    const s = openTab(EMPTY, tab('a'));
    expect(s.tabs.map((t) => t.path)).toEqual(['a']);
    expect(activeTab(s)?.path).toBe('a');
  });

  it('inserts after the active tab, not at the end', () => {
    // A tab opened from a link belongs next to what linked to it.
    const s = openTab(three(), tab('new'));
    expect(s.tabs.map((t) => t.path)).toEqual(['a', 'b', 'new', 'c']);
    expect(activeTab(s)?.path).toBe('new');
  });

  it('never opens a second tab for the same file', () => {
    // Otherwise a wiki-link followed twice fills the strip with one note.
    const s = openTab(three(), tab('a'));
    expect(s.tabs.map((t) => t.path)).toEqual(['a', 'b', 'c']);
    expect(activeTab(s)?.path).toBe('a');
  });

  it('reveals an already-open file without disturbing where the reader was in it', () => {
    let s = activateTab(three(), 0);
    s = rememberLine(s, 42);
    s = openTab(s, tab('b'));
    s = openTab(s, tab('a'));
    expect(activeTab(s)?.line).toBe(42);
  });

  it('honours an explicit line for an already-open file', () => {
    // A search hit in a note that happens to be open still goes to its line.
    let s = rememberLine(activateTab(three(), 0), 42);
    s = openTab(s, { ...tab('a'), line: 9 });
    expect(activeTab(s)?.line).toBe(9);
  });

});

describe('closeTab', () => {
  it('hands over to the tab on the right', () => {
    const s = closeTab(three(), 1);
    expect(s.tabs.map((t) => t.path)).toEqual(['a', 'c']);
    expect(activeTab(s)?.path).toBe('c');
  });

  it('hands over to the left when there is no right', () => {
    // Closing the last of a group should not jump you back to the first.
    const s = closeTab(activateTab(three(), 2), 2);
    expect(activeTab(s)?.path).toBe('b');
  });

  it('closing an inactive tab does not change what is showing', () => {
    const s = closeTab(three(), 0);
    expect(activeTab(s)?.path).toBe('b');
  });

  it('closing the only tab empties the strip, but not the history', () => {
    const s = closeTab(openTab(EMPTY, tab('a')), 0);
    expect(s.tabs).toEqual([]);
    expect(s.active).toBe(-1);
    expect(s.closed.map((t) => t.path)).toEqual(['a']);
  });

  it('ignores an index that is not there', () => {
    const s = three();
    expect(closeTab(s, 9)).toBe(s);
    expect(closeTab(s, -1)).toBe(s);
  });
});

describe('cycle', () => {
  it('wraps in both directions', () => {
    expect(activeTab(cycle(activateTab(three(), 2), 1))?.path).toBe('a');
    expect(activeTab(cycle(activateTab(three(), 0), -1))?.path).toBe('c');
  });

  it('does nothing on an empty strip', () => {
    expect(cycle(EMPTY, 1)).toEqual(EMPTY);
  });
});

describe('tabLabel', () => {
  it('uses the file name even when there is a title', () => {
    expect(tabLabel({ path: '/a/b.md', title: 'Runbook', line: 0 })).toBe('b.md');
  });

  it('falls back to the file name, on either separator', () => {
    expect(tabLabel({ path: '/a/b.md', title: '', line: 0 })).toBe('b.md');
    expect(tabLabel({ path: 'C:\\a\\b.md', title: '', line: 0 })).toBe('b.md');
  });
});

describe('the closed-tab history', () => {
  it('reopens the most recently closed tab', () => {
    const after = closeTab(three(), 1);
    const back = popClosed(after);
    expect(back?.tab.path).toBe('b');
  });

  it('keeps the history when the last tab is closed', () => {
    // The whole point of reopening is that it works after you have closed
    // everything; wiping the history there would be the one case it is for.
    let s = closeTab(three(), 2);
    s = closeTab(s, 1);
    s = closeTab(s, 0);
    expect(s.tabs).toEqual([]);
    expect(s.closed.map((t) => t.path)).toEqual(['c', 'b', 'a']);
  });

  it('remembers where you were in a tab you closed', () => {
    const s = closeTab(rememberLine(three(), 88), 1);
    expect(popClosed(s)?.tab.line).toBe(88);
  });

  it('takes the tab off the history so a second press goes further back', () => {
    let s = closeTab(three(), 2);
    s = closeTab(s, 1);

    const first = popClosed(s);
    expect(first?.tab.path).toBe('b');
    const second = popClosed(first!.state);
    expect(second?.tab.path).toBe('c');
    expect(popClosed(second!.state)).toBe(null);
  });

  it('answers null when nothing has been closed', () => {
    expect(popClosed(three())).toBe(null);
  });

  it('forgets the oldest beyond the cap', () => {
    let s = EMPTY;
    for (let i = 0; i < CLOSED_CAP + 3; i += 1) s = openTab(s, tab(`n${i}`));
    for (let i = 0; i < CLOSED_CAP + 3; i += 1) s = closeTab(s, 0);

    expect(s.closed.length).toBe(CLOSED_CAP);
    expect(s.closed[0]?.path).toBe('n3');
  });
});

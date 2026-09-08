/**
 * The tree model and its virtualization arithmetic.
 *
 * This is the part of the sidebar worth unit-testing: it is pure, and it is
 * where a mistake shows up as a tree that drifts as you scroll rather than as
 * an error. Rendering correctness lives in wdio; the Rust side of the vault is
 * `cargo test`.
 */
import { describe, expect, it } from 'vitest';

import {
  ancestorsOf,
  displayName,
  OVERSCAN,
  ROW_HEIGHT,
  toggle,
  visibleRows,
  windowOf,
} from '../../src/lib/sidebar/tree';
import type { Entry } from '../../src/lib/ipc';

/** A scan entry, in the depth-first order `scan::walk` guarantees. */
function entry(path: string, dir = false): Entry {
  const name = path.slice(path.lastIndexOf('/') + 1);
  return { path, name, dir, depth: path.split('/').length - 1, size: 0, mtime_ms: 0 };
}

const vault: Entry[] = [
  entry('index.md'),
  entry('journal', true),
  entry('journal/2026', true),
  entry('journal/2026/01.md'),
  entry('journal/2026/02.md'),
  entry('projects', true),
  entry('projects/marklet.md'),
];

const paths = (rows: { path: string }[]): string[] => rows.map((r) => r.path);

describe('visibleRows', () => {
  it('collapses every folder by default', () => {
    // A vault opened fully expanded presents five thousand rows and hides its
    // own shape. The top level is the useful first view.
    expect(paths(visibleRows(vault, new Set()))).toEqual(['index.md', 'journal', 'projects']);
  });

  it('reveals only the children of an expanded folder', () => {
    expect(paths(visibleRows(vault, new Set(['journal'])))).toEqual([
      'index.md',
      'journal',
      'journal/2026',
      'projects',
    ]);
  });

  it('nests, so a grandchild needs both ancestors open', () => {
    expect(paths(visibleRows(vault, new Set(['journal/2026'])))).toEqual([
      'index.md',
      'journal',
      'projects',
    ]);
    expect(paths(visibleRows(vault, new Set(['journal', 'journal/2026'])))).toEqual([
      'index.md',
      'journal',
      'journal/2026',
      'journal/2026/01.md',
      'journal/2026/02.md',
      'projects',
    ]);
  });

  it('marks a folder expandable only when the scan gave it children', () => {
    const withEmpty = [...vault, entry('empty', true)];
    const rows = visibleRows(withEmpty, new Set());
    expect(rows.find((r) => r.path === 'journal')?.expandable).toBe(true);
    expect(rows.find((r) => r.path === 'empty')?.expandable).toBe(false);
  });

  it('does not confuse a sibling whose name is a prefix of a folder', () => {
    // `journal-old` starts with `journal` but is not inside it. Skipping by a
    // bare prefix rather than `prefix + "/"` would swallow it.
    const tricky = [entry('journal', true), entry('journal/a.md'), entry('journal-old.md')];
    expect(paths(visibleRows(tricky, new Set()))).toEqual(['journal', 'journal-old.md']);
  });

  it('survives a partial scan, which is what streaming means', () => {
    // The first batch may end mid-folder. It must still render.
    expect(paths(visibleRows(vault.slice(0, 3), new Set(['journal'])))).toEqual([
      'index.md',
      'journal',
      'journal/2026',
    ]);
    expect(visibleRows([], new Set())).toEqual([]);
  });
});

describe('windowOf', () => {
  it('renders a window, not the whole tree', () => {
    // The point of the exercise: 5 000 rows, a 600 px viewport, and fewer than
    // forty rows in the DOM.
    const w = windowOf(5000, 0, 600);
    expect(w.end - w.start).toBeLessThan(40);
    expect(w.padBottom).toBe((5000 - w.end) * ROW_HEIGHT);
  });

  it('keeps total height constant as it scrolls', () => {
    const total = 5000 * ROW_HEIGHT;
    for (const scrollTop of [0, 1, 999, 40_000, total]) {
      const w = windowOf(5000, scrollTop, 600);
      expect(w.padTop + (w.end - w.start) * ROW_HEIGHT + w.padBottom).toBe(total);
    }
  });

  it('overscans in both directions so a fast scroll is not blank', () => {
    const w = windowOf(5000, 100 * ROW_HEIGHT, 600);
    expect(w.start).toBe(100 - OVERSCAN);
  });

  it('does not scroll past the end', () => {
    const w = windowOf(10, 10_000, 600);
    expect(w.end).toBe(10);
    expect(w.padBottom).toBe(0);
  });

  it('is empty for an empty tree rather than negative', () => {
    expect(windowOf(0, 0, 600)).toEqual({ start: 0, end: 0, padTop: 0, padBottom: 0 });
    expect(windowOf(5, -50, -10).start).toBe(0);
  });
});

describe('ancestorsOf', () => {
  it('lists every folder on the way to a note', () => {
    expect(ancestorsOf('journal/2026/03/note.md')).toEqual([
      'journal',
      'journal/2026',
      'journal/2026/03',
    ]);
    expect(ancestorsOf('top.md')).toEqual([]);
  });
});

describe('toggle', () => {
  it('adds, removes, and never mutates the set it was given', () => {
    const before = new Set(['a']);
    const opened = toggle(before, 'b');
    expect([...opened].sort()).toEqual(['a', 'b']);
    expect([...toggle(opened, 'a')]).toEqual(['b']);
    // The original is untouched: expanded folders are plain data, so a stale
    // reference can never mutate the live set out from under a render.
    expect([...before]).toEqual(['a']);
  });
});

describe('displayName', () => {
  it('drops a markdown extension and keeps everything else', () => {
    expect(displayName('Note.md', false)).toBe('Note');
    expect(displayName('Note.markdown', false)).toBe('Note');
    expect(displayName('archive.tar.md', false)).toBe('archive.tar');
    expect(displayName('journal', true)).toBe('journal');
    expect(displayName('.hidden', false)).toBe('.hidden');
  });
});

import { describe, expect, it } from 'vitest';
import { nearestHeading } from '../../src/lib/outline';
import type { Heading } from '../../src/lib/ipc';

function heading(line: number, slug: string, level = 1): Heading {
  return { level, text: slug, slug, line };
}

describe('nearestHeading', () => {
  it('returns null for an empty outline', () => {
    expect(nearestHeading([], 10)).toBeNull();
  });

  it('returns null when the line is before every heading', () => {
    const headings = [heading(5, 'a'), heading(10, 'b')];
    expect(nearestHeading(headings, 1)).toBeNull();
  });

  it('picks the last heading at or before the given line', () => {
    const headings = [heading(1, 'a'), heading(10, 'b'), heading(20, 'c')];
    expect(nearestHeading(headings, 15)?.slug).toBe('b');
  });

  it('matches exactly on a heading boundary', () => {
    const headings = [heading(1, 'a'), heading(10, 'b'), heading(20, 'c')];
    expect(nearestHeading(headings, 10)?.slug).toBe('b');
  });

  it('picks the last heading when the line is past every heading', () => {
    const headings = [heading(1, 'a'), heading(10, 'b'), heading(20, 'c')];
    expect(nearestHeading(headings, 1000)?.slug).toBe('c');
  });

  it('handles a single-heading outline', () => {
    const headings = [heading(5, 'only')];
    expect(nearestHeading(headings, 0)).toBeNull();
    expect(nearestHeading(headings, 5)?.slug).toBe('only');
    expect(nearestHeading(headings, 999)?.slug).toBe('only');
  });

  it('agrees with a linear scan across a larger outline', () => {
    // Cross-check the binary search against the obvious O(n) definition —
    // "the last heading whose line is <= the target" — over a range of
    // targets, so an off-by-one in the binary search bounds cannot hide
    // behind a small hand-picked fixture.
    const headings = Array.from({ length: 50 }, (_, i) => heading(i * 3, `h${i}`));
    function linear(line: number): Heading | null {
      let best: Heading | null = null;
      for (const h of headings) {
        if (h.line <= line) best = h;
      }
      return best;
    }
    for (let line = -5; line <= 160; line += 1) {
      expect(nearestHeading(headings, line)?.slug ?? null).toBe(linear(line)?.slug ?? null);
    }
  });
});

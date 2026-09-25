/**
 * The byte-exactness proof.
 *
 * `src/lib/edit/splice.ts` is where the "no serializer" promise is actually
 * kept: it reduces a whole-buffer change to the ONE byte range that differs,
 * so everything outside that range is copied through by Rust untouched. The
 * headline test below edits a single cell of a hand-aligned table and diffs
 * line by line — the same assertion `src-tauri/src/ipc.rs`'s
 * `editing_one_table_cell_leaves_every_other_line_byte_identical` makes on the
 * far side of the boundary. Both halves have to hold for the property to be
 * real; proving it only in Rust would leave the offsets themselves untested.
 */
import { describe, expect, it } from 'vitest';

import { computeSplice, utf8Length } from '../../src/lib/edit/splice';

/** Applies a splice the way Rust does, over bytes, so the test proves offsets. */
function applySplice(before: string, splice: { start: number; end: number; replacement: string }) {
  const bytes = new TextEncoder().encode(before);
  const out = new Uint8Array(
    bytes.length - (splice.end - splice.start) + new TextEncoder().encode(splice.replacement).length,
  );
  const replacement = new TextEncoder().encode(splice.replacement);
  out.set(bytes.subarray(0, splice.start), 0);
  out.set(replacement, splice.start);
  out.set(bytes.subarray(splice.end), splice.start + replacement.length);
  return new TextDecoder().decode(out);
}

const ALIGNED = `# Report

| Component  | Installer | Loaded when     |
|:-----------|----------:|:---------------:|
| mermaid    |    750 KB | has a diagram   |
| katex      |    200 KB | has math        |
| codemirror |    180 KB | F2 / F3 / Ctrl+E|

Trailing paragraph, untouched.
`;

describe('utf8Length', () => {
  it('counts ASCII as one byte each', () => {
    expect(utf8Length('hello')).toBe(5);
  });

  it('counts two-, three- and four-byte code points', () => {
    expect(utf8Length('é')).toBe(2);
    expect(utf8Length('€')).toBe(3);
    expect(utf8Length('😀')).toBe(4);
  });

  it('agrees with TextEncoder on mixed text', () => {
    const mixed = 'a é € 😀 — «quoted» ✓ 日本語';
    expect(utf8Length(mixed)).toBe(new TextEncoder().encode(mixed).length);
  });
});

describe('computeSplice', () => {
  it('is null when nothing changed', () => {
    expect(computeSplice('same', 'same')).toBeNull();
  });

  it('reduces an insertion to a zero-width range', () => {
    const splice = computeSplice('ab', 'aXb');
    expect(splice).toEqual({ start: 1, end: 1, replacement: 'X' });
  });

  it('reduces a deletion to an empty replacement', () => {
    expect(computeSplice('abc', 'ac')).toEqual({ start: 1, end: 2, replacement: '' });
  });

  it('reports byte offsets, not string indices', () => {
    // "é" is one JavaScript character and two UTF-8 bytes. A splice that
    // reported string indices would be off by one for every accented
    // character before the cursor — silently, and only in some documents.
    const splice = computeSplice('café x', 'café y');
    expect(splice).toEqual({ start: 6, end: 7, replacement: 'y' });
    expect(applySplice('café x', splice!)).toBe('café y');
  });

  it('never splits a surrogate pair', () => {
    const before = 'a😀b';
    const after = 'a😀c';
    const splice = computeSplice(before, after)!;
    expect(splice.start).toBe(5); // 'a' + four bytes of the emoji
    expect(applySplice(before, splice)).toBe(after);

    // And the other direction: changing the emoji itself must replace the
    // whole four-byte code point, not half of it.
    const swapped = computeSplice('a😀b', 'a🎉b')!;
    expect(swapped.start).toBe(1);
    expect(swapped.end).toBe(5);
    expect(applySplice('a😀b', swapped)).toBe('a🎉b');
  });

  /**
   * The acceptance criterion, from the frontend's side: editing one cell of a
   * hand-aligned table leaves every other line byte-identical. Proved by
   * diffing the result, not by reading it.
   */
  it('editing one table cell leaves every other line byte-identical', () => {
    const edited = ALIGNED.replace('|    200 KB |', '|     63 KB |');
    const splice = computeSplice(ALIGNED, edited)!;

    // One narrow range, inside the katex row — not the whole table, and
    // certainly not the whole file.
    expect(splice.end - splice.start).toBeLessThan(20);

    const result = applySplice(ALIGNED, splice);
    const before = ALIGNED.split('\n');
    const after = result.split('\n');

    expect(after).toHaveLength(before.length);
    const changed = before.map((line, i) => (line === after[i] ? null : i)).filter((i) => i !== null);
    expect(changed).toEqual([5]);
    expect(after[5]).toBe('| katex      |     63 KB | has math        |');
    expect(result).toBe(edited);
  });

  it('a multi-line rewrite is still one range', () => {
    const before = 'alpha\nbravo\ncharlie\ndelta\n';
    const after = 'alpha\nBRAVO\nCHARLIE\ndelta\n';
    const splice = computeSplice(before, after)!;

    expect(splice.start).toBe(6);
    expect(applySplice(before, splice)).toBe(after);
    // The unchanged first and last lines are outside the range entirely.
    expect(before.slice(0, splice.start)).toBe('alpha\n');
  });

  it('handles a replacement of the entire document', () => {
    const splice = computeSplice('old', 'new')!;
    expect(splice).toEqual({ start: 0, end: 3, replacement: 'new' });
  });

  it('handles growing from empty and shrinking to empty', () => {
    expect(computeSplice('', 'hello')).toEqual({ start: 0, end: 0, replacement: 'hello' });
    expect(computeSplice('hello', '')).toEqual({ start: 0, end: 5, replacement: '' });
  });
});

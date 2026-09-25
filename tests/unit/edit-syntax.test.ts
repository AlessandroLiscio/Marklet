/**
 * The live-preview rule set, tested without CodeMirror.
 *
 * `src/lib/edit/syntax.ts` is deliberately plain data and plain functions over
 * `@lezer/markdown` node names, so the decisions — what is hidden, what is
 * replaced, what is left exactly as typed — can be asserted here while the
 * ~180 KB of CodeMirror stays unloaded. The plumbing that carries the
 * decisions out lives in `livepreview.ts` and is exercised in the app.
 */
import { describe, expect, it } from 'vitest';

import {
  BULLET,
  TASK_CHECKED,
  TASK_UNCHECKED,
  actionFor,
  appliesWhileRevealed,
  revealedLines,
} from '../../src/lib/edit/syntax';

describe('actionFor', () => {
  it('hides the markers that carry no information', () => {
    for (const node of [
      'EmphasisMark',
      'StrikethroughMark',
      'CodeMark',
      'CodeInfo',
      'HeaderMark',
      'QuoteMark',
      'LinkMark',
      'URL',
      'LinkTitle',
      'HorizontalRule',
    ]) {
      expect(actionFor(node, 'Paragraph', ''), node).toEqual({ kind: 'hide' });
    }
  });

  it('keeps the text of what those markers described, and says what it means', () => {
    expect(actionFor('StrongEmphasis', 'Paragraph', '')).toEqual({
      kind: 'mark',
      cls: 'cm-md-strong',
    });
    expect(actionFor('Emphasis', 'Paragraph', '')).toEqual({ kind: 'mark', cls: 'cm-md-em' });
    expect(actionFor('InlineCode', 'Paragraph', '')).toEqual({ kind: 'mark', cls: 'cm-md-code' });
    expect(actionFor('Link', 'Paragraph', '')).toEqual({ kind: 'mark', cls: 'cm-md-link' });
    expect(actionFor('Image', 'Paragraph', '')).toEqual({ kind: 'mark', cls: 'cm-md-image' });
  });

  it('turns a bullet into a bullet and leaves an ordered list its number', () => {
    expect(actionFor('ListMark', 'ListItem', '-')).toEqual({
      kind: 'replace',
      text: BULLET,
      cls: 'cm-md-bullet',
    });
    expect(actionFor('ListMark', 'ListItem', '*')).toMatchObject({ kind: 'replace' });

    // `3.` says which item this is. A bullet does not, so the number stays.
    expect(actionFor('ListMark', 'ListItem', '3.')).toEqual({
      kind: 'mark',
      cls: 'cm-md-number',
    });
  });

  it('renders a task marker as a checkbox in the right state', () => {
    expect(actionFor('TaskMarker', 'ListItem', '[ ]')).toMatchObject({
      kind: 'replace',
      text: TASK_UNCHECKED,
    });
    expect(actionFor('TaskMarker', 'ListItem', '[x]')).toMatchObject({
      kind: 'replace',
      text: TASK_CHECKED,
    });
    expect(actionFor('TaskMarker', 'ListItem', '[X]')).toMatchObject({ text: TASK_CHECKED });
  });

  /**
   * The refusals, and the reason they are refusals. A table's pipes and
   * padding ARE the alignment somebody tuned by hand; hiding them would move
   * every column in the editor to a position the file does not have, in the
   * one construct this project promises to round-trip byte for byte.
   */
  it('leaves tables and reference definitions exactly as typed', () => {
    for (const node of ['Table', 'TableRow', 'TableCell', 'TableDelimiter', 'LinkReference']) {
      expect(actionFor(node, 'Document', ''), node).toBeNull();
    }
  });

  it('leaves anything nested inside an undecorated container alone', () => {
    // Emphasis inside a table cell would otherwise lose its asterisks and
    // shorten the cell, breaking the alignment of every row below it.
    expect(actionFor('EmphasisMark', 'TableCell', '*')).toBeNull();
    expect(actionFor('CodeMark', 'TableCell', '`')).toBeNull();
  });

  it('leaves raw HTML, comments and escapes alone', () => {
    expect(actionFor('HTMLTag', 'Paragraph', '')).toBeNull();
    expect(actionFor('Comment', 'Document', '')).toBeNull();
    expect(actionFor('Escape', 'Paragraph', '')).toBeNull();
  });

  it('has no opinion about a node it does not know', () => {
    expect(actionFor('SomeFutureNode', 'Paragraph', '')).toBeNull();
  });
});

describe('revealedLines', () => {
  const lineAt = (pos: number) => Math.floor(pos / 10) + 1;

  it('reveals the line a cursor is on', () => {
    expect([...revealedLines([{ from: 25, to: 25 }], lineAt)]).toEqual([3]);
  });

  it('reveals every line a selection touches', () => {
    expect([...revealedLines([{ from: 5, to: 34 }], lineAt)]).toEqual([1, 2, 3, 4]);
  });

  it('reveals every line of every cursor in a multi-cursor selection', () => {
    const lines = revealedLines(
      [
        { from: 5, to: 5 },
        { from: 45, to: 45 },
      ],
      lineAt,
    );
    expect([...lines].sort((a, b) => a - b)).toEqual([1, 5]);
  });

  it('does not care which end of a range is first', () => {
    expect([...revealedLines([{ from: 34, to: 5 }], lineAt)]).toEqual([1, 2, 3, 4]);
  });

  it('reveals nothing when there is no selection at all', () => {
    expect(revealedLines([], lineAt).size).toBe(0);
  });
});

describe('appliesWhileRevealed', () => {
  it('keeps styling but stops removing characters', () => {
    // Bold text stays bold with its asterisks showing. Losing the formatting
    // as well as gaining the markers would make every cursor move flash the
    // whole line.
    expect(appliesWhileRevealed({ kind: 'mark', cls: 'cm-md-strong' })).toBe(true);
    expect(appliesWhileRevealed({ kind: 'hide' })).toBe(false);
    expect(appliesWhileRevealed({ kind: 'replace', text: BULLET, cls: 'x' })).toBe(false);
  });
});

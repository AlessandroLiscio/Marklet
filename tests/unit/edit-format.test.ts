import { describe, expect, it } from 'vitest';
import { applyFormat, FORMATS, type Format } from '../../src/lib/edit/format';

const by = (key: string): Format => FORMATS.find((f) => f.key === key) as Format;

/** Applies the edits the way CodeMirror would: all offsets are of the original text. */
function run(text: string, from: number, to: number, key: string): { text: string; sel: string } {
  const done = applyFormat(text, from, to, by(key));
  let out = text;
  for (const c of [...done.changes].sort((a, b) => b.from - a.from)) {
    out = out.slice(0, c.from) + c.insert + out.slice(c.to);
  }
  return { text: out, sel: out.slice(done.anchor, done.head) };
}

describe('applyFormat', () => {
  it('wraps the selection and keeps it selected', () => {
    const r = run('make this loud', 5, 9, 'b');
    expect(r.text).toBe('make **this** loud');
    expect(r.sel).toBe('this');
  });

  it('puts the cursor between an empty pair', () => {
    const r = run('ab', 1, 1, 'i');
    expect(r.text).toBe('a__b');
    expect(r.sel).toBe('');
  });

  it('takes bold off again, whether the markers are outside or inside the selection', () => {
    expect(run('a **b** c', 4, 5, 'b').text).toBe('a b c');
    expect(run('a **b** c', 2, 7, 'b').text).toBe('a b c');
  });

  it('does not read bold as italic, so Ctrl+I on bold text adds italic', () => {
    expect(run('**x**', 2, 3, 'i').text).toBe('**_x_**');
  });

  it('does strikethrough and inline code', () => {
    expect(run('old', 0, 3, 'x').text).toBe('~~old~~');
    expect(run('let a', 4, 5, '`').text).toBe('let `a`');
  });

  it('makes a link and selects the address', () => {
    const r = run('see docs here', 4, 8, 'k');
    expect(r.text).toBe('see [docs](url) here');
    expect(r.sel).toBe('url');
  });

  it('gives an empty selection a link label', () => {
    expect(run('', 0, 0, 'k').text).toBe('[link](url)');
  });
});

import {
  applyBlock,
  applyImage,
  applyTable,
  inlineFormat,
  tableMarkdown,
  type FormatKind,
} from '../../src/lib/edit/format';

/** Applies a block edit to `text` and returns the new text. */
function block(text: string, from: number, to: number, kind: FormatKind): string {
  const done = applyBlock(text, from, to, kind);
  let out = text;
  for (const c of [...done.changes].sort((a, b) => b.from - a.from)) {
    out = out.slice(0, c.from) + c.insert + out.slice(c.to);
  }
  return out;
}

describe('block formats', () => {
  it('toggles a heading and replaces another level', () => {
    expect(block('title', 0, 0, 'h1')).toBe('# title');
    expect(block('# title', 0, 0, 'h1')).toBe('title');
    expect(block('# title', 0, 0, 'h2')).toBe('## title');
  });

  it('applies a list kind to every touched line, skipping blank ones', () => {
    expect(block('a\n\nb', 0, 4, 'bullet')).toBe('- a\n\n- b');
    expect(block('- a\n- b', 0, 7, 'bullet')).toBe('a\nb');
    expect(block('a\nb', 0, 3, 'numbered')).toBe('1. a\n2. b');
    expect(block('1. a\n2. b', 0, 9, 'numbered')).toBe('a\nb');
    expect(block('- a', 0, 0, 'task')).toBe('- [ ] a');
  });

  it('treats a checked task as a task when toggling the format off', () => {
    expect(block('- [x] done\n- [ ] open', 0, 21, 'task')).toBe('done\nopen');
    expect(block('- [X] done', 0, 0, 'task')).toBe('done');
  });

  it('quotes and unquotes', () => {
    expect(block('a\nb', 0, 3, 'quote')).toBe('> a\n> b');
    expect(block('> a\n> b', 0, 7, 'quote')).toBe('a\nb');
  });

  it('wraps lines in a fence and unwraps them', () => {
    expect(block('x = 1', 0, 5, 'codeblock')).toBe('```\nx = 1\n```');
    expect(block('```\nx = 1\n```', 0, 13, 'codeblock')).toBe('x = 1');
  });

  it('inserts a table and a rule after the current line', () => {
    expect(block('intro', 0, 0, 'table')).toContain('intro\n\n| Column | Column | Column |\n| --- |');
    expect(block('intro', 5, 5, 'rule')).toBe('intro\n\n---\n');
  });

  it('inserts an image around the selection with the address selected', () => {
    const done = applyBlock('cat', 0, 3, 'image');
    expect(done.changes[0]!.insert).toBe('![cat](url)');
    expect('![cat](url)'.slice(done.anchor, done.head)).toBe('url');
  });

  it('maps the inline buttons to the keyboard formats', () => {
    expect(inlineFormat('bold')?.open).toBe('**');
    expect(inlineFormat('link')?.link).toBe(true);
    expect(inlineFormat('table')).toBeUndefined();
  });
});

describe('block formats on an empty row', () => {
  /** Applies the edit and returns the text with `|` where the caret ends up. */
  function caret(text: string, at: number, kind: FormatKind): string {
    const done = applyBlock(text, at, at, kind);
    expect(done.anchor).toBe(done.head);
    let out = text;
    for (const c of [...done.changes].sort((a, b) => b.from - a.from)) {
      out = out.slice(0, c.from) + c.insert + out.slice(c.to);
    }
    return `${out.slice(0, done.head)}|${out.slice(done.head)}`;
  }

  it('writes the marker and leaves the caret after it', () => {
    expect(caret('', 0, 'h1')).toBe('# |');
    expect(caret('', 0, 'h3')).toBe('### |');
    expect(caret('', 0, 'bullet')).toBe('- |');
    expect(caret('', 0, 'numbered')).toBe('1. |');
    expect(caret('', 0, 'task')).toBe('- [ ] |');
    expect(caret('', 0, 'quote')).toBe('> |');
  });

  it('works on an empty line between paragraphs, and leaves the others alone', () => {
    expect(caret('a\n\nb', 2, 'h2')).toBe('a\n## |\nb');
  });

  it('toggles off again, and swaps one heading level for another', () => {
    expect(block('# ', 2, 2, 'h1')).toBe('');
    expect(block('# ', 2, 2, 'h2')).toBe('## ');
    expect(block('- ', 2, 2, 'bullet')).toBe('');
  });
});

describe('table insertion', () => {
  it('builds the size asked for, header row included', () => {
    expect(tableMarkdown(2, 3)).toBe('| Column | Column |\n| --- | --- |\n|  |  |\n|  |  |\n');
    expect(tableMarkdown(1, 1)).toBe('| Column |\n| --- |\n');
  });

  it('keeps the old three-column table as the default of the bare button', () => {
    expect(block('intro', 0, 0, 'table')).toBe(`intro\n\n${tableMarkdown(3, 2)}`);
  });

  it('holds the size inside the limits', () => {
    expect(tableMarkdown(0, 0)).toBe(tableMarkdown(1, 1));
    expect(tableMarkdown(99, 2).split('\n')[0]!.split('|').length).toBe(20 + 2);
    expect(tableMarkdown(2, 999).split('\n').length).toBe(50 + 2); // plus the rule line and the trailing break
  });

  it('goes after the current line with the first header cell selected', () => {
    const text = 'one\ntwo';
    const done = applyTable(text, 1, 2, 2);
    const change = done.changes[0]!;
    const out = text.slice(0, change.from) + change.insert + text.slice(change.to);
    expect(out.startsWith('one\n\n| Column')).toBe(true);
    expect(out.slice(done.anchor, done.head)).toBe('Column');
    expect(out.endsWith('\ntwo')).toBe(true);
  });
});

describe('image insertion', () => {
  it('uses the file name as the description and leaves the caret after it', () => {
    const done = applyImage('', 0, 0, 'assets/trip-photo.png');
    expect(done.changes[0]!.insert).toBe('![trip-photo](assets/trip-photo.png)');
    expect(done.anchor).toBe(done.head);
    expect(done.head).toBe('![trip-photo](assets/trip-photo.png)'.length);
  });

  it('prefers the selected text as the description', () => {
    expect(applyImage('my cat', 3, 6, 'cat.jpg').changes[0]).toEqual({
      from: 3,
      to: 6,
      insert: '![cat](cat.jpg)',
    });
  });

  it('wraps an address with a space in angle brackets', () => {
    expect(applyImage('', 0, 0, '../my pics/a b.png').changes[0]!.insert).toBe(
      '![a b](<../my pics/a b.png>)',
    );
  });
});

describe('numbering on an empty row', () => {
  it('carries on the count of the item above, and starts at 1 otherwise', () => {
    expect(block('1. a\n2. b\n', 10, 10, 'numbered')).toBe('1. a\n2. b\n3. ');
    expect(block('text\n', 5, 5, 'numbered')).toBe('text\n1. ');
  });
});

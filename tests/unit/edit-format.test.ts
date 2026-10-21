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

import { applyBlock, inlineFormat, type FormatKind } from '../../src/lib/edit/format';

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

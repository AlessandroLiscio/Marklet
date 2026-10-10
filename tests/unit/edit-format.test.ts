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

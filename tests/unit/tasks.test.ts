import { describe, expect, it } from 'vitest';
import { toggleTask } from '../../src/lib/tasks';

function apply(source: string, line: number): string | null {
  const t = toggleTask(source, line);
  if (t === null) return null;
  const bytes = new TextEncoder().encode(source);
  const out = [
    ...bytes.slice(0, t.start),
    ...new TextEncoder().encode(t.replacement),
    ...bytes.slice(t.end),
  ];
  return new TextDecoder().decode(new Uint8Array(out));
}

describe('toggleTask', () => {
  it('checks an unchecked box and unchecks a checked one', () => {
    expect(apply('- [ ] a\n- [x] b\n', 1)).toBe('- [x] a\n- [x] b\n');
    expect(apply('- [ ] a\n- [x] b\n', 2)).toBe('- [ ] a\n- [ ] b\n');
    expect(apply('- [X] a\n', 1)).toBe('- [ ] a\n');
  });

  it('handles indentation, tabs, other markers and ordered lists', () => {
    expect(apply('\t- [ ] a\n', 1)).toBe('\t- [x] a\n');
    expect(apply('  * [ ] a\n', 1)).toBe('  * [x] a\n');
    expect(apply('1. [ ] a\n', 1)).toBe('1. [x] a\n');
  });

  it('counts bytes, not characters, before the box', () => {
    expect(apply('# héllo — ✓\n\n- [ ] ünï\n', 3)).toBe('# héllo — ✓\n\n- [x] ünï\n');
  });

  it('leaves CRLF files alone', () => {
    expect(apply('- [ ] a\r\n- [ ] b\r\n', 2)).toBe('- [ ] a\r\n- [x] b\r\n');
  });

  it('returns null when the line is not a task', () => {
    expect(toggleTask('- plain\n', 1)).toBeNull();
    expect(toggleTask('- [ ] a\n', 5)).toBeNull();
    expect(toggleTask('text [ ] not a task\n', 1)).toBeNull();
  });
});

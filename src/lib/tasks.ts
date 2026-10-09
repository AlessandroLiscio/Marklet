/**
 * Flipping a task-list checkbox in the markdown source.
 *
 * Pure: the source text and the 1-based line in, a byte range and its
 * replacement out — the shape `splice_range` takes. Nothing here knows about
 * the DOM or the file.
 */
import { utf8Length } from './edit/splice';

export interface TaskToggle {
  /** Byte offsets of the three characters `[ ]` / `[x]`. */
  start: number;
  end: number;
  replacement: string;
}

/** A list marker, then the box: `- [ ]`, `* [x]`, `1. [X]`, any indent. */
const TASK = /^(\s*(?:[-*+]|\d{1,9}[.)])\s+)\[([ xX])\]/;

/**
 * The edit that flips the checkbox on `line`, or `null` if that line holds
 * none — the document changed under the click, or the line is not a task.
 */
export function toggleTask(source: string, line: number): TaskToggle | null {
  const lines = source.split('\n');
  const text = lines[line - 1];
  if (text === undefined) return null;

  const match = TASK.exec(text);
  if (match === null) return null;

  const before = lines.slice(0, line - 1).join('\n');
  const lineStart = line === 1 ? 0 : utf8Length(before) + 1;
  const start = lineStart + utf8Length(match[1] ?? '');
  return { start, end: start + 3, replacement: match[2] === ' ' ? '[x]' : '[ ]' };
}

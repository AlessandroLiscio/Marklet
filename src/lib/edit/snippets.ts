/**
 * Tab expansion: type a word, press Tab, get the markdown.
 *
 * Pure string arithmetic over the buffer — no CodeMirror types anywhere — so
 * the whole table is unit tested and `session.ts` only has to bind Tab to it.
 *
 * Every expansion below is markdown a person would otherwise type by hand, and
 * every one of them is *plain text going into a plain text document*. There is
 * no snippet engine, no tab stops, no placeholder traversal: one insertion,
 * one cursor position. That keeps this honest with the rest of the editor,
 * where the document is never in any representation but markdown.
 */

/** A ready-to-apply edit: replace `[from, to)` with `insert`, put the cursor at `cursor`. */
export interface Expansion {
  /** String index where the typed word starts. */
  from: number;
  /** String index just past the typed word. */
  to: number;
  insert: string;
  /** Absolute string index for the caret afterwards. */
  cursor: number;
}

/**
 * `$0` marks where the caret lands. It is stripped from the inserted text, so
 * a snippet that wants a literal `$0` cannot have one — an acceptable trade
 * for a table of fifteen entries that never needs it.
 */
const CARET = '$0';

/**
 * The table. Kept small on purpose: a snippet nobody remembers is a keystroke
 * nobody saves, and Tab still has to indent when no word matches.
 *
 * The GFM alert bodies match what the render core actually understands — see
 * `.claude/skills/render-pipeline/SKILL.md` and the `alerts` fixture — so an
 * expansion always previews as something, never as a plain blockquote that
 * looks like a bug.
 */
export const SNIPPETS: Record<string, string> = {
  table: '| Column | Column |\n|--------|--------|\n| $0     |        |',
  code: '```$0\n\n```',
  link: '[$0](url)',
  img: '![$0](path.png)',
  wiki: '[[$0]]',
  todo: '- [ ] $0',
  done: '- [x] $0',
  quote: '> $0',
  note: '> [!NOTE]\n> $0',
  tip: '> [!TIP]\n> $0',
  important: '> [!IMPORTANT]\n> $0',
  warning: '> [!WARNING]\n> $0',
  caution: '> [!CAUTION]\n> $0',
  hr: '---\n\n$0',
  fn: '[^1]$0\n\n[^1]: ',
  math: '$$\n$0\n$$',
  mermaid: '```mermaid\ngraph TD;\n  $0\n```',
  details: '<details>\n<summary>$0</summary>\n\n\n</details>',
};

/** The word immediately before `pos`, or `''`. Letters and digits only. */
export function wordBefore(text: string, pos: number): string {
  let start = pos;
  while (start > 0) {
    const ch = text.charCodeAt(start - 1);
    const isWord =
      (ch >= 48 && ch <= 57) || (ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122);
    if (!isWord) break;
    start -= 1;
  }
  return text.slice(start, pos);
}

/**
 * The expansion for whatever word sits before `pos`, or `null` when there
 * isn't one.
 *
 * `null` is the common case and the caller must treat it as "Tab means indent"
 * — swallowing Tab whenever the cursor happens to follow a word would break
 * indenting a nested list, which is the thing Tab is for the rest of the time.
 */
export function expandSnippet(text: string, pos: number): Expansion | null {
  const word = wordBefore(text, pos);
  if (word.length === 0) return null;

  const body = SNIPPETS[word.toLowerCase()];
  if (body === undefined) return null;

  const from = pos - word.length;
  const caretAt = body.indexOf(CARET);
  const insert = caretAt === -1 ? body : body.slice(0, caretAt) + body.slice(caretAt + CARET.length);

  return {
    from,
    to: pos,
    insert,
    cursor: from + (caretAt === -1 ? insert.length : caretAt),
  };
}

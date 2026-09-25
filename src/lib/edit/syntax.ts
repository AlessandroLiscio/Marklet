/**
 * What live preview hides, and — just as important — what it refuses to hide.
 *
 * This module is the *decision*; `livepreview.ts` is the CodeMirror plumbing
 * that carries it out. Splitting them is not tidiness: everything here is
 * plain data and plain functions over node names, so the rule set can be unit
 * tested without loading a single byte of CodeMirror, which is the point of
 * the whole lazy-chunk arrangement.
 *
 * The behaviour being described is Obsidian's Live Preview, and Obsidian does
 * **not** use ProseMirror for it. It is CodeMirror 6 with view decorations
 * that hide markdown syntax on every line the cursor is not on. That single
 * distinction is why Marklet can have this feature at ~180 KB with no
 * serializer, instead of ~400 KB with one — see
 * `.claude/skills/size-budget/SKILL.md`, where ProseMirror, Tiptap and
 * Milkdown are rejected on the serializer rather than on the size.
 *
 * Node names below are `@lezer/markdown`'s, with the GFM extensions
 * `@codemirror/lang-markdown` enables through `markdownLanguage`.
 */

/** What live preview does with one syntax node. */
export type SyntaxAction =
  /** Remove the text entirely while the line is not being edited. */
  | { kind: 'hide' }
  /** Swap the text for a short stand-in — a bullet, a checkbox. */
  | { kind: 'replace'; text: string; cls: string }
  /** Keep the text, add a class so it reads as what it means. */
  | { kind: 'mark'; cls: string };

/**
 * Markers that vanish outright. Every one of them is pure syntax: removing it
 * loses nothing a reader wants, and the styling that replaces it is applied to
 * the parent node by {@link MARKED}.
 *
 * `CodeMark` covers both inline backticks and the ``` of a fence; `LinkMark`
 * covers `[`, `]`, `(`, `)` and an image's leading `![`.
 */
const HIDDEN = new Set([
  'EmphasisMark', // * _ around emphasis and strong
  'StrikethroughMark', // ~~
  'CodeMark', // ` and ```
  'CodeInfo', // the language after an opening fence
  'HeaderMark', // leading # of an ATX heading, ==== of a setext one
  'QuoteMark', // >
  'LinkMark', // [ ] ( ) and ![
  'URL', // the destination of a link or image
  'LinkTitle', // the "title" after a URL
  'HorizontalRule', // --- , replaced by a ruled line
]);

/** Nodes whose text stays but gains the meaning its markers used to carry. */
const MARKED: Record<string, string> = {
  Emphasis: 'cm-md-em',
  StrongEmphasis: 'cm-md-strong',
  Strikethrough: 'cm-md-strike',
  InlineCode: 'cm-md-code',
  Link: 'cm-md-link',
  Image: 'cm-md-image',
  Autolink: 'cm-md-link',
};

/**
 * Block containers that colour a whole line. Handled per visible line rather
 * than per node, because a blockquote or a fenced block spans many lines and a
 * line decoration has to sit at each line's start.
 */
export const LINE_CLASSES: Record<string, string> = {
  ATXHeading1: 'cm-md-h1',
  ATXHeading2: 'cm-md-h2',
  ATXHeading3: 'cm-md-h3',
  ATXHeading4: 'cm-md-h4',
  ATXHeading5: 'cm-md-h5',
  ATXHeading6: 'cm-md-h6',
  SetextHeading1: 'cm-md-h1',
  SetextHeading2: 'cm-md-h2',
  Blockquote: 'cm-md-quote',
  FencedCode: 'cm-md-fenced',
  CodeBlock: 'cm-md-fenced',
  HorizontalRule: 'cm-md-rule',
  Table: 'cm-md-table',
};

/**
 * Markdown that is deliberately left exactly as the author typed it, and why.
 *
 * **Tables.** The pipes and the padding *are* the alignment somebody tuned by
 * hand. Hiding them would move every column in the editor to a position the
 * file does not have, so what you edit would stop looking like what is saved —
 * in the one construct where that matters most, and in the one this project
 * promises to round-trip byte for byte.
 *
 * **Reference link definitions** (`[label]: https://…`), for the same reason:
 * a reference block is a hand-maintained table of its own.
 *
 * **Raw HTML, comments, and escapes.** All three mean "I want these exact
 * characters"; a preview that hides them is hiding the author's intent.
 *
 * **Frontmatter.** Not markdown at all. It is metadata, and it is the thing
 * people most often open the editor specifically to change.
 */
export const UNDECORATED = new Set([
  'Table',
  'TableHeader',
  'TableRow',
  'TableCell',
  'TableDelimiter',
  'LinkReference',
  'HTMLBlock',
  'HTMLTag',
  'Comment',
  'CommentBlock',
  'Escape',
  'Entity',
  'FrontMatter',
  'ProcessingInstructionBlock',
]);

/** `[ ]` / `[x]` become a checkbox glyph rather than disappearing. */
export const TASK_UNCHECKED = '☐';
export const TASK_CHECKED = '☑';
/** The stand-in for a `-`/`*`/`+` bullet. */
export const BULLET = '•';

/**
 * The action for one node, or `null` to leave it alone.
 *
 * `parent` decides two cases the node name alone cannot:
 *  - a `ListMark` inside an ordered list is the *number*, which carries
 *    information and is kept; inside a bullet list it is punctuation and
 *    becomes a bullet,
 *  - anything inside a table (or another {@link UNDECORATED} container) is
 *    left verbatim no matter what its own name says.
 */
export function actionFor(node: string, parent: string | null, text: string): SyntaxAction | null {
  if (UNDECORATED.has(node)) return null;
  if (parent !== null && UNDECORATED.has(parent)) return null;

  if (node === 'TaskMarker') {
    const checked = /\[[xX]\]/.test(text);
    return {
      kind: 'replace',
      text: checked ? TASK_CHECKED : TASK_UNCHECKED,
      cls: checked ? 'cm-md-task cm-md-task-done' : 'cm-md-task',
    };
  }

  if (node === 'ListMark') {
    // `1.` and `1)` say which item this is. A bullet does not.
    if (/^\d/.test(text)) return { kind: 'mark', cls: 'cm-md-number' };
    return { kind: 'replace', text: BULLET, cls: 'cm-md-bullet' };
  }

  if (HIDDEN.has(node)) return { kind: 'hide' };

  const cls = MARKED[node];
  return cls === undefined ? null : { kind: 'mark', cls };
}

/**
 * Which lines show their raw markdown right now: every line touched by a
 * cursor or overlapped by a selection.
 *
 * This *is* the feature. A line the cursor enters reveals its source
 * immediately and hides it again the moment the cursor leaves, so the syntax
 * is always available exactly where you are typing and never anywhere else.
 * Line numbers are 1-based, matching CodeMirror and matching `data-l`.
 */
export function revealedLines(
  ranges: readonly { from: number; to: number }[],
  lineAt: (pos: number) => number,
): Set<number> {
  const lines = new Set<number>();
  for (const range of ranges) {
    const first = lineAt(Math.min(range.from, range.to));
    const last = lineAt(Math.max(range.from, range.to));
    for (let line = first; line <= last; line += 1) lines.add(line);
  }
  return lines;
}

/**
 * Whether an action still applies on a line the cursor is on.
 *
 * Styling survives the reveal — bold text stays bold with its asterisks
 * showing — because losing the formatting as well as gaining the markers would
 * make every cursor move flash the whole line. Only the two actions that
 * *remove* characters step aside.
 */
export function appliesWhileRevealed(action: SyntaxAction): boolean {
  return action.kind === 'mark';
}

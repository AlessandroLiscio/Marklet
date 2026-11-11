/**
 * Word-style formatting keys for the editor: `Ctrl+B` bold, `Ctrl+I` italic and
 * so on, applied to the selection.
 *
 * Pure text arithmetic. The editor hands in the document, the selection and the
 * marker, and gets back the edits to make — which is also what makes the rules
 * testable without a DOM. Nothing here knows about CodeMirror.
 *
 * Every format is a toggle: pressing it on text that is already wrapped removes
 * the markers instead of adding a second pair.
 */
import { imageMarkdown } from './paste';

export interface Format {
  /** The key, as `KeyboardEvent.key` spells it. */
  key: string;
  shift?: boolean;
  /** Written before and after the text. */
  open: string;
  close: string;
  /** A link: the text goes in brackets and an address in the parentheses. */
  link?: boolean;
}

/**
 * Italic is `_`, not `*`: with `*`, a bold word (`**x**`) looks italic to a
 * toggle that only counts asterisks, and Ctrl+I would quietly un-bold it.
 */
export const FORMATS: Format[] = [
  { key: 'b', open: '**', close: '**' },
  { key: 'i', open: '_', close: '_' },
  { key: 'x', shift: true, open: '~~', close: '~~' },
  { key: '`', open: '`', close: '`' },
  { key: 'k', open: '[', close: '](url)', link: true },
];

export interface Edit {
  from: number;
  to: number;
  insert: string;
}

export interface Formatted {
  changes: Edit[];
  /** The selection afterwards, in the new document's offsets. */
  anchor: number;
  head: number;
}

/** What the placeholder address says, selected so typing replaces it. */
const URL_PLACEHOLDER = 'url';

/** The edits that apply (or remove) `format` over `[from, to)` of `text`. */
export function applyFormat(text: string, from: number, to: number, format: Format): Formatted {
  const { open, close } = format;

  if (format.link) {
    const label = text.slice(from, to) || 'link';
    const at = from + open.length + label.length + 2; // past `](`
    return {
      changes: [{ from, to, insert: `${open}${label}](${URL_PLACEHOLDER})` }],
      anchor: at,
      head: at + URL_PLACEHOLDER.length,
    };
  }

  // Already wrapped, markers just outside the selection: take them off.
  if (
    from >= open.length &&
    text.slice(from - open.length, from) === open &&
    text.slice(to, to + close.length) === close
  ) {
    return {
      changes: [
        { from: from - open.length, to: from, insert: '' },
        { from: to, to: to + close.length, insert: '' },
      ],
      anchor: from - open.length,
      head: to - open.length,
    };
  }

  // Already wrapped, markers inside the selection: the same, from the other side.
  const picked = text.slice(from, to);
  if (
    picked.length >= open.length + close.length &&
    picked.startsWith(open) &&
    picked.endsWith(close)
  ) {
    return {
      changes: [{ from, to, insert: picked.slice(open.length, picked.length - close.length) }],
      anchor: from,
      head: to - open.length - close.length,
    };
  }

  return {
    changes: [
      { from, to: from, insert: open },
      { from: to, to, insert: close },
    ],
    anchor: from + open.length,
    head: to + open.length,
  };
}

/**
 * What a toolbar button asks for: an inline format (the same ones the Word-style
 * keys apply) or a block shape.
 */
export type FormatKind =
  | 'bold'
  | 'italic'
  | 'strike'
  | 'code'
  | 'link'
  | 'h1'
  | 'h2'
  | 'h3'
  | 'bullet'
  | 'numbered'
  | 'task'
  | 'quote'
  | 'codeblock'
  | 'table'
  | 'rule'
  | 'image';

/** The inline kinds, mapped onto the formats the keyboard already uses. */
const INLINE: Partial<Record<FormatKind, string>> = {
  bold: 'b',
  italic: 'i',
  strike: 'x',
  code: '`',
  link: 'k',
};

/** The `Format` an inline kind stands for, or `undefined` for a block kind. */
export function inlineFormat(kind: FormatKind): Format | undefined {
  const key = INLINE[kind];
  return key === undefined ? undefined : FORMATS.find((f) => f.key === key);
}

/** The most columns and rows (header included) the table picker offers. */
export const TABLE_MAX = { cols: 20, rows: 50 };

/** What the toolbar button inserts when it is not asked for a size. */
const TABLE_DEFAULT = { cols: 3, rows: 2 };

const clampInt = (n: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Math.round(Number.isFinite(n) ? n : min)));

/**
 * A ready table of `cols` columns and `rows` rows, the header row among them:
 * the header, its rule and `rows - 1` empty rows.
 */
export function tableMarkdown(cols: number, rows: number): string {
  const c = clampInt(cols, 1, TABLE_MAX.cols);
  const r = clampInt(rows, 1, TABLE_MAX.rows);
  const line = (cell: string): string => `|${Array.from({ length: c }, () => ` ${cell} `).join('|')}|\n`;
  return line('Column') + line('---') + line('').repeat(r - 1);
}

/**
 * The edits that put a `cols` × `rows` table after the line the selection ends
 * on, with the first header cell selected so typing names it.
 */
export function applyTable(text: string, to: number, cols: number, rows: number): Formatted {
  const i = text.indexOf('\n', to);
  const at = i === -1 ? text.length : i;
  const lead = '\n\n';
  const caret = at + lead.length + 2;
  return {
    changes: [{ from: at, to: at, insert: lead + tableMarkdown(cols, rows) }],
    anchor: caret,
    head: caret + 'Column'.length,
  };
}

/**
 * The edits for an image whose address the file picker returned: the selection
 * (or the file's name) becomes the description, and the caret lands after it.
 */
export function applyImage(text: string, from: number, to: number, path: string): Formatted {
  const name = path.slice(path.lastIndexOf('/') + 1);
  const alt = text.slice(from, to) || name.replace(/\.[^.]*$/, '');
  const insert = imageMarkdown(path, alt);
  const caret = from + insert.length;
  return { changes: [{ from, to, insert }], anchor: caret, head: caret };
}

/** Line prefixes a block kind toggles. The numbered one is matched by pattern. */
const PREFIX: Partial<Record<FormatKind, string>> = {
  h1: '# ',
  h2: '## ',
  h3: '### ',
  bullet: '- ',
  task: '- [ ] ',
  quote: '> ',
};

const NUMBERED = /^(\d+)\. /;
const TASK = /^- \[[ xX]\] /;
const ANY_HEADING = /^#{1,6} /;
const ANY_LIST = /^(- \[[ xX]\] |[-*+] |\d+\. )/;

/**
 * The edits for a block button over `[from, to)`.
 *
 * Line kinds (headings, lists, quote) work on every line the selection touches
 * and toggle: pressing it on lines that already have the prefix takes it off.
 * A heading replaces another heading level rather than stacking, and a list
 * kind replaces another list kind. On an empty row the marker is written alone,
 * ready to type after. Insertion kinds (table, rule, image, code block) add a
 * structure at the selection; `image` is the bare placeholder, used when no file
 * picker is wired: {@link applyImage} is the real thing.
 */
export function applyBlock(text: string, from: number, to: number, kind: FormatKind): Formatted {
  const lineStart = (pos: number) => text.lastIndexOf('\n', pos - 1) + 1;
  const lineEnd = (pos: number) => {
    const i = text.indexOf('\n', pos);
    return i === -1 ? text.length : i;
  };

  if (kind === 'table') return applyTable(text, to, TABLE_DEFAULT.cols, TABLE_DEFAULT.rows);

  if (kind === 'rule') {
    // On a fresh line: after the current one, with a blank line before.
    const at = lineEnd(to);
    const insert = '\n\n---\n';
    const caret = at + insert.length;
    return { changes: [{ from: at, to: at, insert }], anchor: caret, head: caret };
  }

  if (kind === 'image') {
    const alt = text.slice(from, to) || 'image';
    const at = from + 2 + alt.length + 2; // past `![alt](`
    return {
      changes: [{ from, to, insert: `![${alt}](url)` }],
      anchor: at,
      head: at + 3,
    };
  }

  if (kind === 'codeblock') {
    const start = lineStart(from);
    const end = lineEnd(to);
    const body = text.slice(start, end);
    const fenced = body.startsWith('```') && body.trimEnd().endsWith('```') && body.includes('\n');
    if (fenced) {
      const inner = body.split('\n').slice(1, -1).join('\n');
      return { changes: [{ from: start, to: end, insert: inner }], anchor: start, head: start + inner.length };
    }
    const insert = `\`\`\`\n${body}\n\`\`\``;
    return { changes: [{ from: start, to: end, insert }], anchor: start + 4, head: start + 4 + body.length };
  }

  const start = lineStart(from);
  // A selection that ends at the start of a line does not touch that line.
  const end = lineEnd(to > from && text[to - 1] === '\n' ? to - 1 : to);
  const lines = text.slice(start, end).split('\n');
  const prefix = PREFIX[kind];

  // Nothing but empty rows: there is no text to prefix, so write the marker
  // itself and leave the caret after it, the way an empty bold pair does.
  if (lines.every((line) => line.trim() === '')) {
    // A numbered row under a numbered item carries on the count.
    const above = kind === 'numbered' && start > 0 ? NUMBERED.exec(text.slice(lineStart(start - 1), start - 1)) : null;
    const marker = kind === 'numbered' ? `${above ? Number(above[1]) + 1 : 1}. ` : (prefix as string);
    const caret = start + marker.length;
    return { changes: [{ from: start, to: lineEnd(start), insert: marker }], anchor: caret, head: caret };
  }

  // A task counts as one whether it is checked or not: toggling the format off
  // must not turn a finished task back into an open one, or the reverse.
  const pattern = kind === 'numbered' ? NUMBERED : kind === 'task' ? TASK : null;
  const has = (line: string): boolean =>
    pattern ? pattern.test(line) : line.startsWith(prefix as string);
  const strip = (line: string): string =>
    line.replace(kind === 'h1' || kind === 'h2' || kind === 'h3' ? ANY_HEADING : ANY_LIST, '');
  const off = lines.filter((l) => l.trim() !== '').every(has);

  let n = 0;
  const next = lines.map((line) => {
    if (line.trim() === '') return line;
    if (off) return pattern ? line.replace(pattern, '') : line.slice((prefix as string).length);
    const base = kind === 'quote' ? line : strip(line);
    n += 1;
    return `${kind === 'numbered' ? `${n}. ` : prefix}${base}`;
  });

  const insert = next.join('\n');
  return { changes: [{ from: start, to: end, insert }], anchor: start, head: start + insert.length };
}

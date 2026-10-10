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

/**
 * Turning "the buffer used to say X and now says Y" into **one** byte range.
 *
 * This is the module the whole editing architecture rests on. Marklet has no
 * markdown serializer — the document is markdown text from the moment Rust
 * reads it to the moment Rust writes it back — and the way that promise is
 * kept is that a save never rewrites the file. It replaces exactly the bytes
 * that changed and copies everything else through untouched, so a hand-aligned
 * table five hundred lines away from the cursor is byte-identical afterwards
 * because nothing ever looked at it.
 *
 * `splice_range` on the Rust side takes **byte** offsets, not character
 * offsets, per `.claude/skills/tauri-ipc/SKILL.md`: the file is bytes, `data-l`
 * is lines, and a JavaScript string index is neither. Converting here, once,
 * is what keeps that boundary honest.
 */

/** One replacement, in file bytes, ready for `splice_range`. */
export interface Splice {
  /** Inclusive byte offset where the replacement starts. */
  start: number;
  /** Exclusive byte offset where the replacement ends. `start === end` is an insertion. */
  end: number;
  /** The text to put there. Empty is a deletion. */
  replacement: string;
}

/**
 * UTF-8 byte length of a JavaScript (UTF-16) string.
 *
 * A hand-rolled scan rather than `new TextEncoder().encode(s).length`, which
 * would allocate a copy of the string on every save — and on the largest
 * documents this app is built for, the prefix being measured *is* most of the
 * file. The arithmetic is the standard UTF-8 width table; a surrogate pair is
 * one 4-byte code point, and a lone surrogate is counted as 3 because that is
 * the width of the U+FFFD an encoder would substitute for it.
 */
export function utf8Length(text: string): number {
  let bytes = 0;
  for (let i = 0; i < text.length; i += 1) {
    const code = text.charCodeAt(i);
    if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
      const low = text.charCodeAt(i + 1);
      if (low >= 0xdc00 && low <= 0xdfff) {
        bytes += 4;
        i += 1;
      } else {
        bytes += 3;
      }
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

function isHighSurrogate(code: number): boolean {
  return code >= 0xd800 && code <= 0xdbff;
}

function isLowSurrogate(code: number): boolean {
  return code >= 0xdc00 && code <= 0xdfff;
}

/**
 * The single minimal splice that turns `before` into `after`, or `null` when
 * they are already equal.
 *
 * Longest common prefix, longest common suffix, everything between them is the
 * range. That is deliberately not a real diff: a real diff would produce
 * several smaller ranges for a multi-cursor edit, and each of them would need
 * its own round trip, its own conflict check and its own offset rebasing
 * against the ones before it. One range per save is slightly more bytes on the
 * wire and dramatically less to reason about — and it is still byte-exact
 * everywhere outside it, which is the only property that actually matters.
 *
 * Both boundaries are pulled back off a surrogate pair before they are
 * measured, so a splice can never be handed to Rust at a position that is not
 * a character boundary. Rust refuses that case as `Invalid` rather than
 * writing a corrupt file; this makes sure it never has to.
 */
export function computeSplice(before: string, after: string): Splice | null {
  if (before === after) return null;

  const max = Math.min(before.length, after.length);

  let prefix = 0;
  while (prefix < max && before.charCodeAt(prefix) === after.charCodeAt(prefix)) prefix += 1;
  // Never end the common prefix between the two halves of a surrogate pair.
  if (prefix > 0 && isHighSurrogate(before.charCodeAt(prefix - 1))) prefix -= 1;

  let suffix = 0;
  while (
    suffix < max - prefix &&
    before.charCodeAt(before.length - 1 - suffix) === after.charCodeAt(after.length - 1 - suffix)
  ) {
    suffix += 1;
  }
  if (suffix > 0 && isLowSurrogate(before.charCodeAt(before.length - suffix))) suffix -= 1;

  const startBytes = utf8Length(before.slice(0, prefix));
  const endBytes = startBytes + utf8Length(before.slice(prefix, before.length - suffix));

  return {
    start: startBytes,
    end: endBytes,
    replacement: after.slice(prefix, after.length - suffix),
  };
}

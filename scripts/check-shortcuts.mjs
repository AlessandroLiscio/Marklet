#!/usr/bin/env node
/**
 * Keeps the keyboard sheet and the keyboard itself in agreement.
 *
 * `src/lib/shortcuts.ts` is what the sheet renders. A sheet that names a key
 * nothing binds is worse than no sheet — silence leaves you looking, a wrong
 * answer stops you. And a key bound but unlisted is the defect the sheet was
 * written to end: an audit found 34 capabilities with no on-screen control at
 * all, and the cheapest way for that to happen again is for the next binding
 * to be added and nobody to remember the list.
 *
 * Two directions, both mechanical:
 *
 *   forward  every listed shortcut with a `where` file names a key that file
 *            actually compares against
 *   reverse  every key compared against anywhere in `src/` appears on the list
 *
 * What it does NOT prove, and cannot cheaply: that the handler still does what
 * the sheet says it does. `where` locates the comparison, not the behaviour.
 * Entries whose trigger is not a key at all — a drag, a wheel, a double-click,
 * or one of CodeMirror's own defaults, which Marklet installs as a whole
 * keymap rather than key by key — carry `where: null` and are skipped by the
 * forward pass. There is nothing in `src/` for it to find, and pretending
 * otherwise would be the false confidence this gate exists to remove.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'src');
const list = join(src, 'lib/shortcuts.ts');

const source = readFileSync(list, 'utf8');

/** `const APP = 'src/app.svelte';` — the `where` values are these, not literals. */
const files = new Map(
  [...source.matchAll(/^const ([A-Z_]+) = '([^']+)';$/gm)].map(([, name, path]) => [name, path]),
);

/** One entry per `{ keys: '…', …, where: X }`, in file order. */
const entries = [...source.matchAll(/\{ keys: '([^']+)',[^}]*?where: ([A-Z_]+|null)[,\s}]/g)].map(
  ([, keys, where]) => ({ keys, where: where === 'null' ? null : files.get(where) }),
);

if (entries.length < 30) {
  console.error(`${relative(root, list)}: found only ${entries.length} shortcuts.`);
  console.error('The list cannot have shrunk that far — the parser has drifted from the file.');
  process.exit(1);
}

/**
 * The `event.key` values a displayed chord could produce.
 *
 * Only the last segment of a chord is a key; `Ctrl` and `Shift` are modifiers
 * and are read from their own flags. A single letter is given in both cases
 * because some handlers compare `event.key` and others lowercase it first.
 */
const NAMED = {
  Esc: ['Escape'],
  // The space bar reports itself as a single space. A sheet that printed that
  // would show an empty key, so the sheet writes the word and the map carries
  // the translation — the one place the two spellings are allowed to differ.
  Space: [' '],
  '←': ['ArrowLeft'],
  '→': ['ArrowRight'],
  '↑': ['ArrowUp'],
  '↓': ['ArrowDown'],
  '=': ['=', '+'],
  '-': ['-', '_'],
};

function keysOf(display) {
  const out = [];
  for (const alternative of display.split(', ')) {
    const last = alternative.split('+').pop().trim();
    if (last in NAMED) out.push(...NAMED[last]);
    else if (last.length === 1) out.push(last.toLowerCase(), last.toUpperCase());
    else out.push(last);
  }
  return out;
}

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (/\.(ts|svelte)$/.test(path)) yield path;
  }
}

const problems = [];

// ------------------------------------------------------------------ forward
for (const { keys, where } of entries) {
  if (where === undefined) {
    problems.push(`${keys}: its \`where\` is not one of the file constants at the top of the list`);
    continue;
  }
  if (where === null) continue;

  let text;
  try {
    text = readFileSync(join(root, where), 'utf8');
  } catch {
    problems.push(`${keys}: \`where\` points at ${where}, which does not exist`);
    continue;
  }

  const found = keysOf(keys).some((key) => text.includes(`'${key}'`));
  if (!found) {
    problems.push(`${keys}: ${where} no longer compares against ${keysOf(keys).join(' or ')}`);
  }
}

// ------------------------------------------------------------------ reverse
/** Every literal compared against an `event.key`, however the handler spells it. */
const COMPARISON = /(?:\.key|\bkey)(?:\.toLowerCase\(\))?\s*[!=]==\s*'([^']*)'/g;

const bound = new Map();
for (const file of walk(src)) {
  if (file === list) continue;
  const text = readFileSync(file, 'utf8');
  for (const [, key] of text.matchAll(COMPARISON)) {
    if (!bound.has(key)) bound.set(key, relative(root, file));
  }
}

const listed = new Set(entries.flatMap(({ keys }) => keysOf(keys)));

for (const [key, file] of bound) {
  if (listed.has(key)) continue;
  problems.push(`'${key}' is handled in ${file} and is on no list — the sheet will not mention it`);
}

if (problems.length > 0) {
  console.error(`${relative(root, list)} and the handlers disagree:`);
  for (const problem of problems) console.error(`  ${problem}`);
  console.error('');
  console.error('Add the key to the list, or correct its `where`, or set `where: null` if the');
  console.error('trigger is a gesture or one of CodeMirror’s own keymap entries.');
  process.exit(1);
}

console.log(
  `${entries.length} shortcuts listed; ${bound.size} keys handled in src/. Both agree.`,
);

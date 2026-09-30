#!/usr/bin/env node
/**
 * Asserts that every chrome selector in `src/styles/print.css` still names an
 * element that exists.
 *
 * `print.css` hid `.sidebar` and `.outline` but not `.chrome-layer`, the fixed
 * shell that holds them, so hiding the panels only emptied the shell instead of
 * removing it: the first PDF anyone opened had the application's left rail
 * printed down the edge of the page and a column of empty panel background
 * beside the text. Nothing failed, and nothing could — a CSS selector that
 * matches nothing is silent by design, and PDF export needs a live webview, so
 * nothing on a developer machine ever renders one.
 *
 * Only the selectors between the fences are checked. The rest of the stylesheet
 * targets the *rendered document* — `.wikilink`, `.alert-caution`,
 * `.footnote-backref` — whose class names are built by `format!` in
 * `src-tauri/src/render/html.rs` and cannot be looked for as literals. The
 * chrome is written by hand in Svelte, where they can.
 *
 * A script rather than a vitest case because Vitest stubs every CSS import to
 * the empty string, `?raw` included, so the file cannot be read from inside a
 * test without turning CSS processing on for the whole suite.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'src');
const stylesheet = join(src, 'styles/print.css');

const START = '/* chrome-selectors:start */';
const END = '/* chrome-selectors:end */';

const css = readFileSync(stylesheet, 'utf8');
const from = css.indexOf(START);
const to = css.indexOf(END);

if (from === -1 || to <= from) {
  console.error(`${relative(root, stylesheet)}: the ${START} / ${END} fences are missing.`);
  console.error('They mark the block this gate reads; without them nothing is checked.');
  process.exit(1);
}

const selectors = [...css.slice(from, to).matchAll(/^\s*([.#][\w-]+)\s*,?\s*$/gm)].map((m) => m[1]);

if (selectors.length < 5) {
  console.error(`${relative(root, stylesheet)}: only ${selectors.length} selectors between the`);
  console.error('fences. That is fewer than the chrome has pieces — the fences have drifted.');
  process.exit(1);
}

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (/\.(ts|js|svelte)$/.test(path)) yield path;
  }
}

/**
 * Whole class tokens, never substrings.
 *
 * The obvious implementation asks whether the name appears inside a `class="…"`
 * with `\b` around it, which cannot tell `.status` apart from an unrelated
 * `class="status-item"` — `\b` counts a hyphen as a word boundary. A dead
 * selector whose name is the prefix of a live one would pass, which is most of
 * what this gate is for. A class attribute is a space-separated list, so it is
 * read as one.
 */
const classes = new Set();
const ids = new Set();

for (const file of walk(src)) {
  const text = readFileSync(file, 'utf8');
  for (const match of text.matchAll(/class=["']([^"']*)["']/g)) {
    for (const token of match[1].split(/\s+/)) if (token !== '') classes.add(token);
  }
  // `class:on={…}` puts a name on an element without ever writing it inside a
  // `class` attribute.
  for (const match of text.matchAll(/class:([\w-]+)/g)) classes.add(match[1]);
  for (const match of text.matchAll(/id=["']([\w-]+)["']/g)) ids.add(match[1]);
}

const dead = selectors.filter((s) => !(s.startsWith('#') ? ids : classes).has(s.slice(1)));

if (dead.length > 0) {
  console.error(`${relative(root, stylesheet)}: ${dead.length} chrome selector(s) name nothing:`);
  for (const selector of dead) console.error(`  ${selector}`);
  console.error('');
  console.error('Each one silently hides nothing, which on paper means it prints.');
  console.error('Rename it to what the element carries now, or drop it.');
  process.exit(1);
}

console.log(`${selectors.length} chrome selectors all name elements that exist.`);

#!/usr/bin/env node
/**
 * Proves every semantic foreground/background pair clears WCAG AA (4.5:1) in
 * both light and dark. Exits non-zero when one does not.
 *
 * Phase P1b expands this alongside the real token layer. The pair list below is
 * the contract: adding a semantic colour means adding its pair here, otherwise
 * the gate silently stops covering it.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const css = readFileSync(join(root, 'src/styles/tokens.css'), 'utf8');

/** Pairs that must clear 4.5:1. Foreground token, background token, label. */
const PAIRS = [
  ['--fg', '--bg', 'body text'],
  ['--fg-muted', '--bg', 'muted text'],
  ['--fg', '--bg-subtle', 'text on subtle surface'],
  ['--fg-muted', '--bg-subtle', 'muted text on subtle surface'],
  ['--accent', '--bg', 'links'],
];

/** Blocks to check: label, and the CSS scope its overrides live in. */
const SCOPES = [
  ['light', /:root\s*\{([\s\S]*?)\n\}/],
  ['dark', /:root\[data-theme="dark"\]\s*\{([\s\S]*?)\n\}/],
];

function parseVars(block) {
  const out = new Map();
  for (const m of block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    out.set(m[1], m[2].trim());
  }
  return out;
}

/** Resolves a token through any number of `var(--other)` indirections. */
function resolve(name, ...scopes) {
  let value;
  for (const scope of scopes) {
    if (scope.has(name)) value = scope.get(name);
  }
  if (value === undefined) return undefined;
  const ref = value.match(/^var\((--[\w-]+)\)$/);
  return ref ? resolve(ref[1], ...scopes) : value;
}

function toRgb(hex) {
  const h = hex.replace('#', '').trim();
  const full = h.length === 3 ? [...h].map((c) => c + c).join('') : h;
  if (!/^[0-9a-f]{6}$/i.test(full)) return null;
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16));
}

function luminance([r, g, b]) {
  const f = (c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const base = parseVars(css.match(SCOPES[0][1])?.[1] ?? '');
let failures = 0;

for (const [label, re] of SCOPES) {
  const scoped = parseVars(css.match(re)?.[1] ?? '');
  console.log(`\n${label}`);
  for (const [fgName, bgName, what] of PAIRS) {
    const fg = toRgb(resolve(fgName, base, scoped) ?? '');
    const bg = toRgb(resolve(bgName, base, scoped) ?? '');
    if (!fg || !bg) {
      console.log(`  ?     ${what} (${fgName} on ${bgName}) — unresolved`);
      failures++;
      continue;
    }
    const ratio = contrast(fg, bg);
    const ok = ratio >= 4.5;
    if (!ok) failures++;
    console.log(`  ${ok ? 'ok' : 'FAIL'}  ${ratio.toFixed(2)}:1  ${what} (${fgName} on ${bgName})`);
  }
}

if (failures > 0) {
  console.error(`\n${failures} pair(s) below 4.5:1.`);
  process.exit(1);
}
console.log('\nAll pairs clear WCAG AA.');

#!/usr/bin/env node
/**
 * Proves every semantic foreground/background pair clears WCAG AA (4.5:1) in
 * both light and dark, in every accent hue Marklet ships — the default plus
 * every `[data-palette]` full adds. Exits non-zero when one does not.
 *
 * Token values are `#rrggbb`/`#rgb` hex, `var(--other)` (arbitrarily
 * nested), or `oklch(L C H)` where H may itself be `var(--accent-hue)` — the
 * accent generator in tokens.css. All three are resolved and converted to
 * sRGB before the WCAG formula runs.
 *
 * The pair list below is the contract: adding a semantic colour means adding
 * its pair here, otherwise the gate silently stops covering it.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const tokensCss = readFileSync(join(root, 'src/styles/tokens.css'), 'utf8');
const palettesCss = readFileSync(join(root, 'src/styles/full/palettes.css'), 'utf8');

/** Pairs that must clear 4.5:1. Foreground token, background token, label. */
const PAIRS = [
  ['--fg', '--bg', 'body text'],
  ['--fg-muted', '--bg', 'muted text'],
  ['--fg', '--bg-subtle', 'text on subtle surface'],
  ['--fg-muted', '--bg-subtle', 'muted text on subtle surface'],
  ['--accent', '--bg', 'links'],
  ['--accent-hover', '--bg', 'links (hover)'],
  ['--on-accent', '--accent', 'text on solid accent (buttons, badges)'],
  ['--fg', '--accent-muted', 'selected text (::selection)'],
  ['--alert-note', '--bg', 'alert label: note'],
  ['--alert-tip', '--bg', 'alert label: tip'],
  ['--alert-important', '--bg', 'alert label: important'],
  ['--alert-warning', '--bg', 'alert label: warning'],
  ['--alert-caution', '--bg', 'alert label: caution'],
];

/** Blocks to check: label, and the CSS scope its overrides live in. */
const SCOPES = [
  ['light', /:root\s*\{([\s\S]*?)\n\}/],
  ['dark', /:root\[data-theme="dark"\]\s*\{([\s\S]*?)\n\}/],
];

/** `[data-palette="x"]` presets full adds on top of the default hue. */
const PALETTE_RE = /\[data-palette="([\w-]+)"\]\s*\{([\s\S]*?)\n\}/g;

function parseVars(block) {
  const out = new Map();
  for (const m of block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    out.set(m[1], m[2].trim());
  }
  return out;
}

/** Resolves a token through any number of `var(--other)` indirections, then
 * through a trailing hex or oklch() literal. Returns a raw value string
 * (hex or oklch(...)), or undefined if the chain bottoms out unresolved. */
function resolveValue(name, ...scopes) {
  let value;
  for (const scope of scopes) {
    if (scope.has(name)) value = scope.get(name);
  }
  if (value === undefined) return undefined;
  const ref = value.match(/^var\((--[\w-]+)\)$/);
  return ref ? resolveValue(ref[1], ...scopes) : value;
}

/** Resolves a bare numeric-or-var() argument, e.g. an oklch() hue channel. */
function resolveScalar(raw, ...scopes) {
  const ref = raw.trim().match(/^var\((--[\w-]+)\)$/);
  if (!ref) return Number.parseFloat(raw);
  const resolved = resolveValue(ref[1], ...scopes);
  return resolved === undefined ? Number.NaN : Number.parseFloat(resolved);
}

function hexToRgb(hex) {
  const h = hex.replace('#', '').trim();
  const full = h.length === 3 ? [...h].map((c) => c + c).join('') : h;
  if (!/^[0-9a-f]{6}$/i.test(full)) return null;
  return [0, 2, 4].map((i) => Number.parseInt(full.slice(i, i + 2), 16));
}

/** oklch(L C H) -> sRGB [0,255]^3. Björn Ottosson's OKLab matrices. */
function oklchToRgb(L, C, Hdeg) {
  const h = (Hdeg * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);

  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;

  const l = l_ ** 3;
  const m = m_ ** 3;
  const s = s_ ** 3;

  const rl = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  const gl = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  const bl = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;

  const toSrgb = (c) => {
    const cl = Math.max(0, Math.min(1, c));
    return cl <= 0.0031308 ? 12.92 * cl : 1.055 * cl ** (1 / 2.4) - 0.055;
  };
  return [rl, gl, bl].map((c) => Math.round(Math.max(0, Math.min(1, toSrgb(c))) * 255));
}

/** Resolves a fully-chained token to an [r,g,b] triple, or null. */
function toRgb(name, ...scopes) {
  const value = resolveValue(name, ...scopes);
  if (value === undefined) return null;

  const oklch = value.match(/^oklch\(\s*(.+)\s*\)$/);
  if (oklch) {
    const parts = oklch[1].trim().split(/\s+/);
    if (parts.length !== 3) return null;
    const [L, C, H] = parts.map((p) => resolveScalar(p, ...scopes));
    if ([L, C, H].some(Number.isNaN)) return null;
    return oklchToRgb(L, C, H);
  }

  return hexToRgb(value);
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

/** Extracts `[data-palette="x"] { --accent-hue: N; ... }` presets. */
function parsePalettes(css) {
  const out = new Map();
  for (const m of css.matchAll(PALETTE_RE)) {
    out.set(m[1], parseVars(m[2]));
  }
  return out;
}

const base = parseVars(tokensCss.match(SCOPES[0][1])?.[1] ?? '');
const palettes = parsePalettes(palettesCss);

let failures = 0;

function runScopes(label, extraScopeName, overrides) {
  for (const [themeLabel, re] of SCOPES) {
    const scoped = parseVars(tokensCss.match(re)?.[1] ?? '');
    const scopes = overrides
      ? [base, scoped, overrides]
      : [base, scoped];
    console.log(`\n${label} / ${themeLabel}`);
    for (const [fgName, bgName, what] of PAIRS) {
      const fg = toRgb(fgName, ...scopes);
      const bg = toRgb(bgName, ...scopes);
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
}

// Default palette (no [data-palette] override — both editions ship this one).
runScopes('default', null, null);

// Full-only palettes: each just overrides --accent-hue, so re-run the same
// pairs with that single var swapped in ahead of the theme scope.
for (const [name, vars] of palettes) {
  runScopes(`palette=${name} (full only)`, name, vars);
}

if (failures > 0) {
  console.error(`\n${failures} pair(s) below 4.5:1.`);
  process.exit(1);
}
console.log('\nAll pairs clear WCAG AA, in every palette.');

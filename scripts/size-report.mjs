#!/usr/bin/env node
/**
 * The scale. Reports what Marklet currently costs a user, in the units the NSIS
 * installer actually charges (LZMA, approximated by `xz -9`).
 *
 * Run it before claiming a change is free. See
 * .claude/skills/size-budget/SKILL.md for the ceilings and for what to do when
 * one is breached — raising it is not on the list.
 */
import { execSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const CHUNK_BUDGETS = {
  mermaid: 750 * 1024,
  katex: 200 * 1024,
  codemirror: 200 * 1024,
  hljs: 40 * 1024,
};
/**
 * The BOOT stylesheet — `dist/assets/index-*.css`, the one `index.html` links,
 * gzipped — not `src/styles/**`.
 *
 * It used to be the source, and that was wrong in a way that quietly punished
 * the thing this repository asks for everywhere else. The source gzips to
 * 11.4 KiB against a 12 KiB ceiling; what actually ships is 3.9 KiB. The
 * difference is comments, which Vite strips and which the old measurement
 * charged in full — so explaining a rule in a stylesheet moved you toward a
 * budget failure, and the fix for a "breach" would have been to delete the
 * explanations.
 *
 * Source was chosen originally to avoid counting KaTeX's stylesheet, which is
 * third-party, lazy, and has its own chunk. Naming the entry file solves that
 * directly: the lazy stylesheets are separate outputs and are not in it.
 *
 * 6 KiB against a measured 3.9 leaves room to grow and still fails on
 * something real. The old ceiling would now be 3x the measurement, and this
 * file's own header says what a gate with that much headroom is worth.
 */
const STYLES_GZIP_MAX = 6144;

const xz = (path) => Number(execSync(`xz -9c ${JSON.stringify(path)} | wc -c`).toString().trim());
const kib = (n) => `${(n / 1024).toFixed(1)} KiB`;

let failures = 0;

console.log('Chunks (xz -9, as the installer charges them)\n');
const assets = join(root, 'dist/assets');
if (!existsSync(assets)) {
  console.log('  dist/ not built — run `npm run build` first.');
} else {
  for (const file of readdirSync(assets).filter((f) => f.endsWith('.js')).sort()) {
    const size = xz(join(assets, file));
    const budget = Object.entries(CHUNK_BUDGETS).find(([name]) => file.startsWith(name))?.[1];
    const over = budget !== undefined && size > budget;
    if (over) failures++;
    const note = budget === undefined ? '' : ` (budget ${kib(budget)})${over ? '  OVER' : ''}`;
    console.log(`  ${kib(size).padStart(10)}  ${file}${note}`);
  }
}

console.log('\nBoot stylesheet (gzip -9)\n');
// The entry stylesheet is whatever index.html links; every other .css in
// dist/assets is a lazy chunk with its own budget above, or none.
const html = existsSync(join(root, 'dist/index.html'))
  ? readFileSync(join(root, 'dist/index.html'), 'utf8')
  : '';
const entryCss = /href="[^"]*\/(index-[^"/]+\.css)"/.exec(html)?.[1];

if (!entryCss) {
  console.log('  not built — run `npm run build` first.');
  failures++;
} else {
  const cssGzip = Number(
    execSync(`gzip -9c ${JSON.stringify(join(root, 'dist/assets', entryCss))} | wc -c`)
      .toString()
      .trim(),
  );
  const cssOver = cssGzip > STYLES_GZIP_MAX;
  if (cssOver) failures++;
  console.log(
    `  ${kib(cssGzip).padStart(10)}  ${entryCss} (budget ${kib(STYLES_GZIP_MAX)})${cssOver ? '  OVER' : ''}`,
  );
}

console.log('\nBinary\n');
const exe = join(root, 'src-tauri/target/release/marklet');
if (existsSync(exe)) {
  console.log(`  ${kib(statSync(exe).size).padStart(10)}  marklet (stripped)`);
} else {
  console.log('  not built — run `cargo build --release --manifest-path src-tauri/Cargo.toml`.');
}

if (failures > 0) {
  console.error(`\n${failures} item(s) over budget.`);
  process.exit(1);
}

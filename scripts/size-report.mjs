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
const STYLES_GZIP_MAX = 12288;

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

console.log('\nStylesheets (gzip -9)\n');
const cssFiles = execSync(`find ${JSON.stringify(join(root, 'src/styles'))} -name '*.css'`)
  .toString()
  .trim()
  .split('\n')
  .filter(Boolean);
const cssBytes = cssFiles.map((f) => readFileSync(f)).reduce((a, b) => Buffer.concat([a, b]), Buffer.alloc(0));
const cssGzip = Number(
  execSync('gzip -9 | wc -c', { input: cssBytes }).toString().trim(),
);
const cssOver = cssGzip > STYLES_GZIP_MAX;
if (cssOver) failures++;
console.log(`  ${kib(cssGzip).padStart(10)}  src/styles/** (budget ${kib(STYLES_GZIP_MAX)})${cssOver ? '  OVER' : ''}`);

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

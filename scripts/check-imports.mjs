#!/usr/bin/env node
/**
 * Asserts that nothing heavy is reachable through a *static* import.
 *
 * Mermaid, KaTeX, highlight.js and CodeMirror are the four largest items in the
 * budget, and all four are supposed to arrive through `import()` gated on
 * document content or on entering edit mode. A single static `import` anywhere
 * in the reachable graph pulls the chunk onto the boot path, and the symptom is
 * not an error — it is a cold start that got 200 ms slower and an installer
 * that grew, noticed weeks later by the size gate if at all.
 *
 * Cheaper to assert here than to diagnose there.
 *
 * See .claude/skills/size-budget/SKILL.md.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(root, 'src');

/** Packages that must never appear in a static import. */
const LAZY_ONLY = [
  { name: 'mermaid', why: '~750 KB — load only when the document matches /```mermaid/' },
  { name: 'katex', why: '~200 KB — load only when the document contains math' },
  { name: 'highlight.js', why: '~28 KB — load after first paint, via IntersectionObserver' },
  { name: '@codemirror/', why: '~180 KB — load only on F2 / F3 / Ctrl+E' },
  { name: '@lezer/', why: 'pulled in by CodeMirror; same rule' },
];

/**
 * Matches static forms only:
 *   import x from 'pkg'      import 'pkg'      export ... from 'pkg'
 * and deliberately not `import('pkg')`, which is the whole point.
 */
const STATIC_IMPORT =
  /(?:^|\n)\s*(?:import|export)\s+(?:[^;'"]*?\sfrom\s+)?['"]([^'"]+)['"]/g;

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (/\.(ts|js|svelte)$/.test(path)) yield path;
  }
}

const violations = [];

for (const file of walk(src)) {
  const text = readFileSync(file, 'utf8');
  for (const match of text.matchAll(STATIC_IMPORT)) {
    const spec = match[1];
    const hit = LAZY_ONLY.find((p) => spec === p.name || spec.startsWith(p.name));
    if (!hit) continue;
    const line = text.slice(0, match.index).split('\n').length;
    violations.push({ file: relative(root, file), line, spec, why: hit.why });
  }
}

if (violations.length > 0) {
  console.error('Static import of a lazy-only package:\n');
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}  imports '${v.spec}'`);
    console.error(`    ${v.why}\n`);
  }
  console.error("Use `await import('<pkg>')` behind a content or mode check instead.");
  process.exit(1);
}

console.log(`No static imports of lazy-only packages (${LAZY_ONLY.length} watched).`);

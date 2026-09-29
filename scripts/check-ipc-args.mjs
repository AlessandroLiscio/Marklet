#!/usr/bin/env node
/**
 * Every `invoke()` payload key must be a parameter the Rust command declares.
 *
 * This exists because the failure it catches is invisible on both sides. Rust
 * compiles: the command is fine. TypeScript compiles: `invoke` takes
 * `Record<string, unknown>`. `svelte-check` is happy, `clippy` is happy, and
 * the call is rejected at runtime, inside Tauri, before it reaches the command
 * — as a rejected promise that a `void invoke(...)` swallows entirely.
 *
 * Two commands shipped that way:
 *
 *   open_note(rel)      called with { path }  -> the explorer's double-click
 *                                               and every wiki-link did
 *                                               nothing at all
 *   backlinks_for(note) called with { path }  -> the backlinks panel was
 *                                               permanently empty
 *
 * Both were found by reading, not by any check, which is the argument for
 * this file.
 *
 * Tauri injects some parameters rather than taking them from the payload —
 * `AppHandle`, `Window`/`WebviewWindow`, and anything `State<'_, T>` — so
 * those are excluded by *type*, not by name.
 *
 * Case: Tauri v2 exposes a snake_case Rust parameter to JavaScript as
 * camelCase unless the command declares `rename_all = "snake_case"`. Both
 * spellings are accepted here; what is rejected is a key that matches no
 * parameter under either.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const RUST = join(root, 'src-tauri/src/ipc.rs');
const TS = join(root, 'src/lib/ipc.ts');

/** Parameter types Tauri fills in itself; never sent in the payload. */
const INJECTED = /^(tauri::)?(AppHandle|Window|WebviewWindow|State\s*<|tauri::State\s*<)/;

const camel = (name) => name.replace(/_([a-z])/g, (_, c) => c.toUpperCase());

/** `#[tauri::command] pub fn name(a: A, b: B) -> …` → { name: Set<param> }. */
function rustCommands(source) {
  const commands = new Map();
  const re = /#\[tauri::command[^\]]*\]\s*pub\s+(?:async\s+)?fn\s+(\w+)\s*\(([\s\S]*?)\)\s*(?:->|\{)/g;

  for (const [, name, args] of source.matchAll(re)) {
    const params = new Set();
    // Split on top-level commas only: `State<'_, VaultState>` contains one.
    let depth = 0;
    let current = '';
    const pieces = [];
    for (const ch of args) {
      if (ch === '<' || ch === '(' || ch === '[') depth += 1;
      else if (ch === '>' || ch === ')' || ch === ']') depth -= 1;
      if (ch === ',' && depth === 0) {
        pieces.push(current);
        current = '';
      } else {
        current += ch;
      }
    }
    pieces.push(current);

    for (const piece of pieces) {
      const arg = piece.trim().replace(/\/\/.*$/gm, '').trim();
      if (arg === '') continue;
      const at = arg.indexOf(':');
      if (at === -1) continue;
      const param = arg.slice(0, at).trim();
      const type = arg.slice(at + 1).trim();
      if (INJECTED.test(type)) continue;
      params.add(param);
    }
    commands.set(name, params);
  }
  return commands;
}

/** `invoke<T>('name', { a, b: c })` → [{ command, keys, line }]. */
function invocations(source) {
  const calls = [];
  const re = /invoke\s*(?:<[^>]*>)?\s*\(\s*'([a-z_]+)'\s*(,)?/g;

  for (const match of source.matchAll(re)) {
    const [, command, hasArgs] = match;
    const line = source.slice(0, match.index).split('\n').length;
    if (!hasArgs) {
      calls.push({ command, keys: [], line });
      continue;
    }

    // Take the balanced `{ … }` that follows, then read its top-level keys.
    const from = source.indexOf('{', match.index + match[0].length);
    if (from === -1) continue;
    let depth = 0;
    let to = from;
    for (; to < source.length; to += 1) {
      const ch = source[to];
      if (ch === '{') depth += 1;
      else if (ch === '}') {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    const body = source.slice(from + 1, to);

    const keys = [];
    depth = 0;
    let head = '';
    for (const ch of body) {
      if (ch === '{' || ch === '(' || ch === '[') depth += 1;
      else if (ch === '}' || ch === ')' || ch === ']') depth -= 1;
      if (ch === ',' && depth === 0) {
        keys.push(head);
        head = '';
      } else {
        head += ch;
      }
    }
    keys.push(head);

    calls.push({
      command,
      line,
      keys: keys
        .map((k) => k.replace(/\/\/.*$/gm, '').trim())
        .filter((k) => k !== '')
        .map((k) => (k.includes(':') ? k.slice(0, k.indexOf(':')) : k).trim())
        .filter((k) => /^[A-Za-z_]\w*$/.test(k)),
    });
  }
  return calls;
}

const commands = rustCommands(readFileSync(RUST, 'utf8'));
const calls = invocations(readFileSync(TS, 'utf8'));

const problems = [];
for (const { command, keys, line } of calls) {
  const params = commands.get(command);
  if (params === undefined) {
    problems.push(`ipc.ts:${line}  invoke('${command}') — no such #[tauri::command]`);
    continue;
  }
  const accepted = new Set([...params, ...[...params].map(camel)]);
  for (const key of keys) {
    if (!accepted.has(key)) {
      const expected = [...params].join(', ') || '(none)';
      problems.push(
        `ipc.ts:${line}  invoke('${command}', { ${key} }) — the command takes: ${expected}`,
      );
    }
  }
}

if (problems.length > 0) {
  console.error('IPC argument names do not match the commands they call:\n');
  for (const problem of problems) console.error(`  ${problem}`);
  console.error(
    '\nTauri rejects the call before it reaches Rust, as a rejected promise.' +
      '\nNothing else in the build catches this.',
  );
  process.exit(1);
}

console.log(`${calls.length} invoke() call sites match ${commands.size} commands.`);

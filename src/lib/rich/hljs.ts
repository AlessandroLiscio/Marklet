/**
 * highlight.js, lazy, and a custom subset — not the full build.
 *
 * Rust emits fenced code plain and stops (`.claude/skills/render-pipeline/SKILL.md`):
 *
 *   <pre data-l="41"><code class="language-rust">…</code></pre>
 *
 * `highlightIn` is called once, only when `index.ts`'s sniff finds at least
 * one such node. It does not highlight anything itself — it wires an
 * `IntersectionObserver` over every matching block and lets the browser
 * decide when each one is worth paying for. That is what makes this
 * "post-paint": `IntersectionObserver` callbacks fire on a later frame than
 * the one that painted the raw text, and a block below the fold is never
 * highlighted until it scrolls near the viewport — so a long document with
 * fifty code blocks does not do fifty synchronous highlight passes the
 * instant it opens.
 *
 * `highlight.js/lib/core` plus 22 individually-imported language modules
 * (not `highlight.js` itself, which is the ~700 KB batteries-included
 * build). Every import below is a dynamic `import()`, including the core —
 * a *static* `import ... from 'highlight.js/...'` anywhere in `src/` fails
 * `npm run check:imports` even inside a file that is itself only ever
 * reached lazily, because that check is a flat text scan, not a reachability
 * graph. `vite.config.ts`'s `manualChunks` groups every one of these (core +
 * all 22 languages) into the single `hljs` output chunk by module id
 * (`node_modules/highlight.js`), regardless of how many separate `import()`
 * call sites there are — measure the result with `npm run size`, don't
 * guess it from the source.
 */
import type { EnrichResult } from './types';
import './hljs.css';

/**
 * The 22 languages this build actually understands, chosen for a technical
 * reader's notes: mainstream application languages, shell/config/data
 * formats, and diff/markdown for docs-about-docs. Each hljs language module
 * registers its own aliases (`js` -> javascript, `py` -> python, `yml` ->
 * yaml, `sh`/`zsh` -> bash, `html`/`xml` both -> xml, ...) automatically —
 * `registerLanguage` reads them off the definition, nothing extra to do here.
 */
const LANGUAGE_LOADERS: Record<string, () => Promise<{ default: unknown }>> = {
  javascript: () => import('highlight.js/lib/languages/javascript'),
  typescript: () => import('highlight.js/lib/languages/typescript'),
  python: () => import('highlight.js/lib/languages/python'),
  rust: () => import('highlight.js/lib/languages/rust'),
  go: () => import('highlight.js/lib/languages/go'),
  java: () => import('highlight.js/lib/languages/java'),
  c: () => import('highlight.js/lib/languages/c'),
  cpp: () => import('highlight.js/lib/languages/cpp'),
  csharp: () => import('highlight.js/lib/languages/csharp'),
  ruby: () => import('highlight.js/lib/languages/ruby'),
  php: () => import('highlight.js/lib/languages/php'),
  swift: () => import('highlight.js/lib/languages/swift'),
  kotlin: () => import('highlight.js/lib/languages/kotlin'),
  bash: () => import('highlight.js/lib/languages/bash'),
  powershell: () => import('highlight.js/lib/languages/powershell'),
  json: () => import('highlight.js/lib/languages/json'),
  yaml: () => import('highlight.js/lib/languages/yaml'),
  sql: () => import('highlight.js/lib/languages/sql'),
  xml: () => import('highlight.js/lib/languages/xml'),
  css: () => import('highlight.js/lib/languages/css'),
  diff: () => import('highlight.js/lib/languages/diff'),
  markdown: () => import('highlight.js/lib/languages/markdown'),
};

/** The subset of the `hljs.HLJSApi` surface this module actually calls. */
interface HljsCore {
  registerLanguage(name: string, def: unknown): void;
  highlightElement(el: HTMLElement): void;
}

let corePromise: Promise<HljsCore> | null = null;

/** Loads the core plus every subset language, once, memoized. */
function loadCore(): Promise<HljsCore> {
  corePromise ??= (async () => {
    const core = ((await import('highlight.js/lib/core')) as { default: HljsCore }).default;
    const entries = Object.entries(LANGUAGE_LOADERS);
    const modules = await Promise.all(entries.map(([, load]) => load()));
    entries.forEach(([name], i) => core.registerLanguage(name, modules[i]!.default));
    return core;
  })();
  return corePromise;
}

/**
 * Watches every fenced code block under `root` and highlights each one the
 * first time it nears the viewport. Resolves once watching has started, not
 * once every block is highlighted — that work is spread over time.
 */
export function highlightIn(root: HTMLElement): EnrichResult {
  const blocks = Array.from(root.querySelectorAll<HTMLElement>('pre > code[class*="language-"]'));
  if (blocks.length === 0) return { rendered: 0 };

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const el = entry.target as HTMLElement;
        observer.unobserve(el);
        void loadCore().then((core) => {
          try {
            core.highlightElement(el);
          } catch {
            // Language not in the subset, or malformed source — the block
            // stays plain text, which is still fully readable.
          }
        });
      }
    },
    { rootMargin: '200px' },
  );

  for (const el of blocks) observer.observe(el);
  return { rendered: blocks.length };
}

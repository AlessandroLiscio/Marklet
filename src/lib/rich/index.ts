/**
 * The one entry point the main thread calls after `setDocument()`.
 *
 * Sniffs the freshly-inserted document for the three placeholder shapes Rust
 * emits (`.claude/skills/render-pipeline/SKILL.md`) and fires only the
 * `import()`s that document actually needs — never more than one dynamic
 * import per feature, and every one of `hljs` / `katex` / `mermaid` is
 * reached only from here, never from a static import anywhere in `src/`
 * (`npm run check:imports` enforces that in both editions). A document with
 * none of the three shapes below calls zero of them: `enrich()` resolves
 * having done nothing, and the network waterfall shows nothing for any of
 * the three chunks.
 */
import type { EnrichResult } from './types';

/** What each of the three lazy passes found, keyed by which one ran. */
export interface EnrichSummary {
  hljs?: EnrichResult;
  katex?: EnrichResult;
  mermaid?: EnrichResult;
}

export function hasCode(root: ParentNode): boolean {
  return root.querySelector('pre > code[class*="language-"]') !== null;
}

export function hasMath(root: ParentNode): boolean {
  return root.querySelector('.math-inline, .math-block') !== null;
}

export function hasMermaid(root: ParentNode): boolean {
  return root.querySelector('.mermaid') !== null;
}

/**
 * Sniffs `root` and dispatches to whichever of the three lazy renderers the
 * document needs. Safe to call on every `setDocument()`, including a plain
 * document with none of the three — that path does one synchronous
 * `querySelector` pass each (cheap) and awaits nothing.
 */
export async function enrich(root: HTMLElement): Promise<EnrichSummary> {
  const summary: EnrichSummary = {};
  const jobs: Promise<void>[] = [];

  if (hasCode(root)) {
    jobs.push(
      import('./hljs').then((mod) => {
        summary.hljs = mod.highlightIn(root);
      }),
    );
  }

  if (hasMath(root)) {
    jobs.push(
      import('./katex').then(async (mod) => {
        summary.katex = await mod.renderMathIn(root);
      }),
    );
  }

  if (hasMermaid(root)) {
    jobs.push(
      import('./mermaid').then(async (mod) => {
        summary.mermaid = await mod.renderMermaidIn(root);
      }),
    );
  }

  await Promise.all(jobs);
  return summary;
}

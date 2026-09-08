/**
 * Shared shapes for the three lazy renderers (`hljs.ts`, `katex.ts`,
 * `mermaid.ts`) and their `index.ts` dispatcher — kept tiny and dependency-free
 * so importing this file never pulls in any of the heavy packages themselves.
 */

/** What one enrichment pass did, for the caller and for tests. */
export interface EnrichResult {
  /** How many matching nodes were rendered (or, for `hljs`, scheduled). */
  rendered: number;
}

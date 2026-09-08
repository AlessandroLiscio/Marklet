/**
 * The floating table of contents' pure logic, split out from
 * `outline.svelte` so it is unit-testable without a component harness —
 * `vitest.config.ts` runs plain `.ts` under `tests/unit/**`, not `.svelte`
 * files, mirroring how `doc.ts` itself is tested.
 *
 * The one thing worth getting right here is scroll-spy without re-querying
 * the DOM on every scroll event: `doc.ts`'s `visibleLine()` already does the
 * DOM work (walking the `data-l` anchors it built once, at document-load
 * time), so this module only has to binary-search the much smaller
 * `outline: Heading[]` array that `visibleLine()`'s answer is checked against.
 */
import type { Heading } from './ipc';

/**
 * The last heading at or before `line` — "nearest `data-l` anchor at or
 * before", the same rule `doc.ts`'s `anchorForLine` applies to the full line
 * map, kept consistent here so the outline's highlighted entry always agrees
 * with where a font/density/measure change would restore scroll to.
 *
 * `headings` must be sorted by `line` ascending, which `RenderedDoc.outline`
 * always is — headings appear in the document in source order.
 */
export function nearestHeading(headings: readonly Heading[], line: number): Heading | null {
  let lo = 0;
  let hi = headings.length - 1;
  let found = -1;

  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const h = headings[mid];
    if (h === undefined) break;
    if (h.line <= line) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }

  return found === -1 ? null : (headings[found] ?? null);
}

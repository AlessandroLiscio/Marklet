/**
 * The arithmetic behind the diagram viewer's zoom, with no DOM in it.
 *
 * Split out because the event plumbing around it has been wrong twice — wheel
 * zoom was once dead in a build where dragging worked — and because the part
 * that can be tested should be, rather than being described as tested. A
 * comment in `mermaid.ts` claimed a `zoom.test.ts` covered this for months
 * before the file existed.
 *
 * `tests/unit/zoom.test.ts`.
 */

/** A rectangle, from `getBoundingClientRect` or from a test. */
export interface Box {
  width: number;
  height: number;
}

/** How much of the overlay a fitted diagram fills, leaving room for the controls. */
export const FIT_MARGIN = 0.9;

/** How far either side of the fitted size the viewer will zoom. */
export const ZOOM_OUT_LIMIT = 0.25;
export const ZOOM_IN_LIMIT = 8;

/**
 * The scale at which `content` fills `view`, whichever axis runs out first.
 *
 * **Why the viewer does not simply open at 1.** A Mermaid `<svg>` carries a
 * `max-width` of its own natural width and `width="100%"`, and in the document
 * that resolves against the reading column, so the diagram fills it. The
 * overlay's stage is sized by its content instead, so `width: 100%` has
 * nothing to resolve against and the SVG falls back to its intrinsic size —
 * which for a flowchart is the layout engine's own units and is usually much
 * smaller than the column it had just been filling. Opening fullscreen made
 * diagrams *smaller*, which is the opposite of what fullscreen is for.
 *
 * Scaling up is allowed: a small diagram in a large window is exactly the case
 * this exists to fix. A degenerate box — zero width or height, which is what
 * an SVG that has not been laid out yet reports — gives 1 rather than
 * `Infinity`, so a diagram that cannot be measured is shown at its own size
 * instead of vanishing.
 */
export function fitScale(content: Box, view: Box, margin = FIT_MARGIN): number {
  if (content.width <= 0 || content.height <= 0) return 1;
  if (view.width <= 0 || view.height <= 0) return 1;
  return Math.min((view.width * margin) / content.width, (view.height * margin) / content.height);
}

/**
 * Keeps a scale within reach of the fitted one.
 *
 * The limits are multiples of `fit` rather than absolute, because `fit` is
 * what the reader sees as 100%: a diagram that opened at 3.2× should still
 * zoom eight times in from *there*, and one that opened at 0.4× should not be
 * nearly at its limit before the reader has touched anything.
 */
export function clampScale(scale: number, fit: number): number {
  return Math.min(fit * ZOOM_IN_LIMIT, Math.max(fit * ZOOM_OUT_LIMIT, scale));
}

/** What the readout shows: the fitted size is 100%, whatever that cost in pixels. */
export function zoomPercent(scale: number, fit: number): number {
  return Math.round((scale / fit) * 100);
}

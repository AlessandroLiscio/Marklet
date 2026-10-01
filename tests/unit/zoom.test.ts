/**
 * Opening a diagram fullscreen has to make it bigger.
 *
 * It did the opposite. A Mermaid `<svg>` carries `width="100%"` and a
 * `max-width` of its own natural width; in the document that resolves against
 * the reading column and the diagram fills it, but the viewer's stage is sized
 * by its content, so `width: 100%` had nothing to resolve against and the SVG
 * fell back to its intrinsic size — smaller than the column it had just been
 * filling. Reported as "si apre a full screen, ma è renderizzata molto piccola".
 *
 * These are the sums behind the fix. `mermaid.ts` has claimed since it was
 * written that a `zoom.test.ts` covered them; this is that file, finally.
 */
import { describe, expect, it } from 'vitest';
import {
  clampScale,
  fitScale,
  zoomPercent,
  FIT_MARGIN,
  ZOOM_IN_LIMIT,
  ZOOM_OUT_LIMIT,
} from '../../src/lib/rich/zoom';

describe('fitScale', () => {
  it('grows a small diagram to fill the window', () => {
    // The reported case: a diagram far smaller than the overlay it opened in.
    expect(fitScale({ width: 200, height: 100 }, { width: 2000, height: 1000 })).toBeCloseTo(9);
  });

  it('shrinks one that is larger than the window', () => {
    expect(fitScale({ width: 4000, height: 1000 }, { width: 1000, height: 1000 })).toBeCloseTo(0.225);
  });

  it('fits the axis that runs out first, so nothing is cut off', () => {
    // Wide and short in a tall window: width decides, and the result must not
    // be the height's more generous answer.
    const wide = fitScale({ width: 1000, height: 100 }, { width: 1000, height: 1000 });
    expect(wide).toBeCloseTo(FIT_MARGIN);
  });

  it('leaves a margin, so the diagram never touches the controls', () => {
    const square = fitScale({ width: 100, height: 100 }, { width: 100, height: 100 });
    expect(square).toBeLessThan(1);
    expect(square).toBeCloseTo(FIT_MARGIN);
  });

  it('answers 1 for a box it cannot measure', () => {
    // An SVG that has not been laid out reports zero, and dividing by it would
    // open the viewer at Infinity — a diagram that vanishes instead of one
    // that is merely the wrong size.
    expect(fitScale({ width: 0, height: 0 }, { width: 800, height: 600 })).toBe(1);
    expect(fitScale({ width: 800, height: 600 }, { width: 0, height: 0 })).toBe(1);
  });
});

describe('clampScale', () => {
  it('measures its limits from the fitted size, not from 1', () => {
    // A diagram that opened at 4x must still have eight times' worth of zoom
    // left in it; absolute limits would have left it almost at the ceiling.
    expect(clampScale(100, 4)).toBe(4 * ZOOM_IN_LIMIT);
    expect(clampScale(0.001, 4)).toBe(4 * ZOOM_OUT_LIMIT);
  });

  it('leaves a scale inside the limits alone', () => {
    expect(clampScale(2, 1)).toBe(2);
  });
});

describe('zoomPercent', () => {
  it('calls the fitted size 100%, whatever it cost in pixels', () => {
    // What the reader sees as "the whole diagram" is the only honest baseline:
    // an SVG's intrinsic size is the layout engine's units, not a size anyone
    // asked for.
    expect(zoomPercent(9, 9)).toBe(100);
    expect(zoomPercent(18, 9)).toBe(200);
    expect(zoomPercent(4.5, 9)).toBe(50);
  });
});

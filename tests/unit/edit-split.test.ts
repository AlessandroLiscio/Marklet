// @vitest-environment happy-dom
/**
 * The two-way scroll link between the split columns, and the one case it got
 * wrong: a deliberate jump.
 *
 * Clicking a heading in the Contents panel scrolls the preview **smoothly**,
 * and a smooth scroll in Chromium runs for several hundred milliseconds. The
 * link's echo suppression is a 150 ms window sized for the settle after one
 * programmatic call, so it expired mid-animation; the link then saw the editor
 * move, pulled the preview back with an instant scroll, cancelled the
 * animation, and left the preview at the line the editor was still on. The
 * heading was reachable from the outline and not from the outline in split
 * view, which is how it was reported.
 *
 * `happy-dom` for `document`/`window` only — everything measured here is the
 * arbitration logic, not layout.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const scrollToLine = vi.fn<(line: number, behavior?: ScrollBehavior) => void>();
const visibleLine = vi.fn<() => number>(() => 0);

vi.mock('../../src/lib/doc', () => ({
  JUMP_EVENT: 'marklet-jump',
  // Spread, not a fixed arity: the wrapper exists because `vi.mock`'s factory
  // is hoisted above the `const`s it closes over, and forwarding a named
  // `behavior` would record an extra `undefined` argument on every call.
  scrollToLine: (...args: [number, ScrollBehavior?]) => scrollToLine(...args),
  visibleLine: () => visibleLine(),
}));

import { ECHO_MS, JUMP_MS, linkScroll } from '../../src/lib/edit/split';
import type { Session } from '../../src/lib/edit/session';

/** Only the two methods `linkScroll` reaches for. */
function fakeSession(): Session & { goToLine: ReturnType<typeof vi.fn> } {
  const goToLine = vi.fn<(line: number) => void>();
  return { goToLine, topLine: () => 0 } as unknown as Session & {
    goToLine: ReturnType<typeof vi.fn>;
  };
}

function jump(line: number): void {
  document.dispatchEvent(new CustomEvent('marklet-jump', { detail: { line } }));
}

let link: ReturnType<typeof linkScroll>;
let session: ReturnType<typeof fakeSession>;

beforeEach(() => {
  vi.useFakeTimers();
  scrollToLine.mockClear();
  visibleLine.mockClear();
  session = fakeSession();
  link = linkScroll();
  link.attach(session);
});

afterEach(() => {
  link.destroy();
  vi.useRealTimers();
});

describe('a deliberate jump', () => {
  it('moves the editor to the target line itself', () => {
    jump(42);
    expect(session.goToLine).toHaveBeenCalledWith(42);
  });

  it('ignores a line of 0, which means no anchor was found', () => {
    // `scrollToSlug` guards this too. Both ends check, because sending the
    // editor to the top of the file is a worse failure than doing nothing.
    jump(0);
    expect(session.goToLine).not.toHaveBeenCalled();
  });

  it('does not pull the preview back while the animation is still running', () => {
    jump(42);
    session.goToLine.mockClear();

    // The editor scrolls in response, several times, across a span longer than
    // the ordinary echo window — this is what used to cancel the animation.
    vi.advanceTimersByTime(ECHO_MS + 50);
    link.onEditorScroll(7);
    vi.advanceTimersByTime(ECHO_MS + 50);
    link.onEditorScroll(9);

    expect(scrollToLine).not.toHaveBeenCalled();
  });

  it('also ignores the preview scrolling under it, so the editor is not dragged along', () => {
    jump(42);
    session.goToLine.mockClear();

    // Every frame of the smooth animation is a scroll event at a line that is
    // not the destination yet.
    visibleLine.mockReturnValue(11);
    vi.advanceTimersByTime(ECHO_MS + 50);
    window.dispatchEvent(new Event('scroll'));

    expect(session.goToLine).not.toHaveBeenCalled();
  });

  it('hands the link back once the animation has had time to finish', () => {
    jump(42);
    session.goToLine.mockClear();

    vi.advanceTimersByTime(JUMP_MS + 1);
    visibleLine.mockReturnValue(50);
    window.dispatchEvent(new Event('scroll'));

    expect(session.goToLine).toHaveBeenCalledWith(50);
  });
});

describe('the ordinary two-way link', () => {
  it('drives the editor from the preview', () => {
    visibleLine.mockReturnValue(12);
    window.dispatchEvent(new Event('scroll'));
    expect(session.goToLine).toHaveBeenCalledWith(12);
  });

  it('drives the preview from the editor', () => {
    link.onEditorScroll(31);
    expect(scrollToLine).toHaveBeenCalledWith(31);
  });

  it('suppresses the answer to its own answer', () => {
    // Without this the two sides answer each other and the columns creep down
    // the document together.
    visibleLine.mockReturnValue(12);
    window.dispatchEvent(new Event('scroll'));
    link.onEditorScroll(12);
    expect(scrollToLine).not.toHaveBeenCalled();
  });

  it('stops listening after destroy', () => {
    link.destroy();
    jump(42);
    visibleLine.mockReturnValue(12);
    window.dispatchEvent(new Event('scroll'));
    expect(session.goToLine).not.toHaveBeenCalled();
  });
});

/**
 * F3 — two columns, kept on the same line.
 *
 * The sync runs through the **`data-l` line map** and nothing else. Not a
 * scroll percentage, not a pixel ratio: a 40-line code block is one line of
 * source and twenty centimetres of preview, so any proportional scheme drifts
 * exactly where a reader notices — and `data-l` is already the contract four
 * other features read (`.claude/skills/render-pipeline/SKILL.md`), so this
 * costs one lookup rather than a second index.
 *
 * The preview column is the ordinary document: `#doc` in the page, scrolled by
 * the window. The editor is a fixed overlay beside it. Nothing is moved in the
 * DOM — `setDocument()` keeps its anchor list, `app.svelte` keeps finding
 * `#doc` by id, and leaving split mode is removing one class.
 */
import { JUMP_EVENT, scrollToLine, visibleLine } from '../doc';
import type { Session } from './session';

/**
 * How long one side ignores scroll events after being scrolled by the other.
 *
 * Smooth scrolling and momentum both keep firing `scroll` well after the
 * programmatic call returns, and without a window each side would answer the
 * other's answer — the columns lock together and creep down the document.
 * Long enough to outlast the settle, short enough that a deliberate scroll
 * immediately afterwards is still honoured.
 */
export const ECHO_MS = 150;

/**
 * How long the link stays muted after a deliberate jump.
 *
 * `scrollToSlug` scrolls smoothly, and a smooth scroll in Chromium runs for
 * several hundred milliseconds — longer for a longer distance. {@link ECHO_MS}
 * is sized for the settle after one programmatic call and expires in the
 * middle of an animation, at which point the link starts arbitrating between
 * two positions that are both still moving. This window covers the animation
 * instead. A real scroll during it is ignored, which is the cost; the
 * alternative is the preview snapping back to where the editor was.
 */
export const JUMP_MS = 700;

export interface ScrollLink {
  /**
   * Hand to `createSession`'s `onScroll`. Safe to pass before the session
   * exists — it does nothing until {@link ScrollLink.attach}.
   */
  onEditorScroll(line: number): void;
  /** Completes the link once the session has been constructed. */
  attach(session: Session): void;
  /** Re-aligns the preview to the editor, e.g. after a re-render. */
  resync(): void;
  destroy(): void;
}

function stamp(): number {
  return typeof performance === 'undefined' ? Date.now() : performance.now();
}

/**
 * Ties the editor's scroll position to the preview's, both ways.
 *
 * The preview is the ordinary document and the *window* is what scrolls it,
 * which is why the listener goes on the window. It took the `<article id="doc">`
 * as an argument until `visibleLine` stopped needing one — measuring a position
 * against an element that scrolls with the document is what made it wrong.
 *
 * Built before the session and completed with `attach`, because the session
 * has to be constructed with its `onScroll` already in place: one of the two
 * must learn about the other late, and a setter is the smaller knot.
 */
export function linkScroll(): ScrollLink {
  let session: Session | null = null;
  let quietUntil = 0;
  let alive = true;

  function onWindowScroll(): void {
    if (!alive || session === null) return;
    const at = stamp();
    if (at < quietUntil) return;
    quietUntil = at + ECHO_MS;
    session.goToLine(visibleLine());
  }

  /**
   * Somebody jumped the preview on purpose — the outline, for now.
   *
   * The editor is moved to the same line here rather than being allowed to
   * follow the preview, because following means reading a position that is
   * still animating.
   */
  function onJump(event: Event): void {
    const line = (event as CustomEvent<{ line?: number }>).detail?.line;
    if (!alive || session === null || line === undefined || line <= 0) return;
    quietUntil = stamp() + JUMP_MS;
    session.goToLine(line);
  }

  window.addEventListener('scroll', onWindowScroll, { passive: true });
  document.addEventListener(JUMP_EVENT, onJump);

  return {
    onEditorScroll(line) {
      if (!alive || session === null) return;
      const at = stamp();
      if (at < quietUntil) return;
      quietUntil = at + ECHO_MS;
      scrollToLine(line);
    },
    attach(next) {
      session = next;
    },
    resync() {
      if (!alive || session === null) return;
      // After a re-render the preview's anchors are new elements at new
      // offsets, so the editor is the side that still knows where the reader
      // was — it is the one that was never replaced.
      quietUntil = 0;
      scrollToLine(session.topLine());
      quietUntil = stamp() + ECHO_MS;
    },
    destroy() {
      alive = false;
      session = null;
      window.removeEventListener('scroll', onWindowScroll);
      document.removeEventListener(JUMP_EVENT, onJump);
    },
  };
}

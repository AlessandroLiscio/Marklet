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
import { scrollToLine, visibleLine } from '../doc';
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
 * `docRoot` is the `<article id="doc">` the render pipeline fills; the window
 * is what scrolls it, which is why the listener goes on the window rather than
 * on the element.
 *
 * Built before the session and completed with `attach`, because the session
 * has to be constructed with its `onScroll` already in place: one of the two
 * must learn about the other late, and a setter is the smaller knot.
 */
export function linkScroll(docRoot: HTMLElement): ScrollLink {
  let session: Session | null = null;
  let quietUntil = 0;
  let alive = true;

  function onWindowScroll(): void {
    if (!alive || session === null) return;
    const at = stamp();
    if (at < quietUntil) return;
    quietUntil = at + ECHO_MS;
    session.goToLine(visibleLine(docRoot));
  }

  window.addEventListener('scroll', onWindowScroll, { passive: true });

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
    },
  };
}

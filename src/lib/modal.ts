/**
 * Keyboard isolation for an overlay that has claimed the window.
 *
 * **The defect this exists to end.** The Mermaid viewer and the keyboard sheet
 * are both drawn as modals — a backdrop, the page unreachable behind them —
 * and neither behaved as one. Their listeners sat on `document` and let
 * everything through, while the application's own listeners sit on `window`.
 * So one `Escape` closed the viewer *and* threw you out of live edit behind
 * it; `+` zoomed the diagram and the document underneath at the same time; and
 * `Ctrl+W`, `Ctrl+Tab`, `Ctrl+O` and `Ctrl+P` all still acted on the
 * application while a diagram covered it. Three separate reports, one cause:
 * modal in appearance only.
 *
 * **Why the capture phase on `window` is the whole fix.** A keydown starts by
 * descending from `window` to whatever has focus, and only then bubbles back.
 * A listener registered here runs before anything else can, and calling
 * `stopPropagation` from it ends the event there: it never reaches the page's
 * own handlers, which all listen on the way back up. The overlay's own keys
 * are then dispatched by hand, which is what makes this isolation rather than
 * deafness.
 *
 * What still works, and must: a default action is not propagation. `Tab` still
 * moves focus and `Enter` still presses the focused button, because the
 * browser performs those itself rather than through a listener. Overlays that
 * contain a text field would need more than this — neither of ours does, and
 * the day one does, this comment is the warning.
 */

/**
 * Takes every keystroke until the returned function is called.
 *
 * `handle` sees each event and decides what the overlay does with it;
 * everything it ignores is simply swallowed, which is the correct behaviour
 * for a modal and the reason this is not a list of keys to block.
 */
export function captureKeys(handle: (event: KeyboardEvent) => void): () => void {
  const onKey = (event: KeyboardEvent): void => {
    event.stopPropagation();
    handle(event);
  };

  window.addEventListener('keydown', onKey, true);
  return () => window.removeEventListener('keydown', onKey, true);
}

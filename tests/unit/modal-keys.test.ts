// @vitest-environment happy-dom
/**
 * An overlay that looks modal has to behave like one.
 *
 * Three separate defects came from the same cause — the diagram viewer and the
 * keyboard sheet both drew a backdrop over the page and then let every key
 * through to it. One `Escape` closed the viewer and left live edit behind it;
 * `+` zoomed the diagram and the document underneath together; `Ctrl+W` closed
 * a tab from behind a fullscreen overlay.
 *
 * What makes `captureKeys` the fix is the phase, which is exactly the kind of
 * thing that gets "simplified" later by someone who reads it as an ordinary
 * listener. These tests fail if it moves.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { captureKeys } from '../../src/lib/modal';

const releases: Array<() => void> = [];

function open(handle: (event: KeyboardEvent) => void): void {
  releases.push(captureKeys(handle));
}

afterEach(() => {
  while (releases.length > 0) releases.pop()?.();
});

function press(key: string, target: EventTarget = window): void {
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
}

describe('captureKeys', () => {
  it('hands the overlay every key, not a chosen few', () => {
    const seen: string[] = [];
    open((event) => seen.push(event.key));

    for (const key of ['Escape', '+', 'w', 'F1']) press(key);

    expect(seen).toEqual(['Escape', '+', 'w', 'F1']);
  });

  it('keeps the page behind from seeing anything', () => {
    // The application listens on `window` on the way back up, which is where
    // `Ctrl+W`, `Ctrl+Tab` and the export chords all live.
    const behind = vi.fn();
    window.addEventListener('keydown', behind);
    open(() => {});

    press('w');
    press('Escape');

    expect(behind).not.toHaveBeenCalled();
    window.removeEventListener('keydown', behind);
  });

  it('stops an event before it reaches what has focus, not after', () => {
    // The page's own listeners are not the only ones that must not fire: a
    // bubble-phase capture would already have run the focused element's
    // handlers by the time it stopped anything.
    const field = document.createElement('input');
    document.body.append(field);
    const onField = vi.fn();
    field.addEventListener('keydown', onField);
    open(() => {});

    press('a', field);

    expect(onField).not.toHaveBeenCalled();
    field.remove();
  });

  it('gives the page back when the overlay closes', () => {
    const behind = vi.fn();
    window.addEventListener('keydown', behind);

    const release = captureKeys(() => {});
    press('w');
    expect(behind).not.toHaveBeenCalled();

    release();
    press('w');
    expect(behind).toHaveBeenCalledTimes(1);

    window.removeEventListener('keydown', behind);
  });
});

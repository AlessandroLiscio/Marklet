import { describe, expect, it, vi } from 'vitest';
import { mount, unmount } from 'svelte';
import Sheet from '../../src/lib/chrome/shortcuts.svelte';
import { GROUPS } from '../../src/lib/shortcuts';

describe('the keyboard sheet', () => {
  it('mounts and lists every shortcut', () => {
    const target = document.createElement('div');
    document.body.append(target);
    const app = mount(Sheet, { target, props: { onclose: () => {} } });

    // The regression this file exists for: a keyed loop over rows that share a
    // key throws at mount, so the sheet never opened. Mounting is the test.
    expect(target.querySelector('[role="dialog"]')).not.toBeNull();
    const rows = GROUPS.flatMap((g) => g.items);
    expect(target.querySelectorAll('kbd').length).toBeGreaterThanOrEqual(rows.length);

    void unmount(app);
    target.remove();
  });

  it('closes on Escape and on F1', () => {
    const target = document.createElement('div');
    document.body.append(target);
    const onclose = vi.fn();
    const app = mount(Sheet, { target, props: { onclose } });
    // `mount` runs effects on the next microtask; flush so the key capture is armed.
    return Promise.resolve().then(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'F1' }));
      expect(onclose).toHaveBeenCalledTimes(2);
      void unmount(app);
      target.remove();
    });
  });
});

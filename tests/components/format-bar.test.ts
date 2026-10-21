import { describe, expect, it, vi } from 'vitest';
import { mount, unmount } from 'svelte';
import FormatBar from '../../src/lib/chrome/format-bar.svelte';

describe('the formatting toolbar', () => {
  it('shows icon-only buttons with a name in the tooltip, and runs the one clicked', () => {
    const target = document.createElement('div');
    document.body.append(target);
    const onrun = vi.fn();
    const app = mount(FormatBar, { target, props: { onrun } });

    const buttons = target.querySelectorAll('button');
    expect(buttons.length).toBeGreaterThanOrEqual(15);
    for (const b of buttons) {
      expect(b.textContent?.trim()).toBe('');
      expect(b.getAttribute('title')).toBeTruthy();
      expect(b.querySelector('svg')).not.toBeNull();
    }

    target.querySelector<HTMLButtonElement>('[data-kind="table"]')!.click();
    expect(onrun).toHaveBeenCalledWith('table');

    // Pressing must not move the focus out of the editor.
    const press = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    buttons[0]!.dispatchEvent(press);
    expect(press.defaultPrevented).toBe(true);

    void unmount(app);
    target.remove();
  });
});

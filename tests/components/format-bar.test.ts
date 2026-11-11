import { describe, expect, it, vi } from 'vitest';
import { mount, tick, unmount } from 'svelte';
import FormatBar from '../../src/lib/chrome/format-bar.svelte';

describe('the formatting toolbar', () => {
  it('shows icon-only buttons with a name in the tooltip, and runs the one clicked', () => {
    const target = document.createElement('div');
    document.body.append(target);
    const onrun = vi.fn();
    const app = mount(FormatBar, { target, props: { onrun, ontable: vi.fn() } });

    const buttons = target.querySelectorAll('button');
    expect(buttons.length).toBeGreaterThanOrEqual(15);
    for (const b of buttons) {
      expect(b.textContent?.trim()).toBe('');
      expect(b.getAttribute('title')).toBeTruthy();
      expect(b.querySelector('svg')).not.toBeNull();
    }

    target.querySelector<HTMLButtonElement>('[data-kind="image"]')!.click();
    expect(onrun).toHaveBeenCalledWith('image');

    // Pressing must not move the focus out of the editor.
    const press = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    buttons[0]!.dispatchEvent(press);
    expect(press.defaultPrevented).toBe(true);

    void unmount(app);
    target.remove();
  });
});

describe('the table size picker', () => {
  function open() {
    const target = document.createElement('div');
    document.body.append(target);
    const onrun = vi.fn();
    const ontable = vi.fn();
    const app = mount(FormatBar, { target, props: { onrun, ontable } });
    const button = target.querySelector<HTMLButtonElement>('[data-kind="table"]')!;
    const close = () => {
      void unmount(app);
      target.remove();
    };
    return { target, onrun, ontable, button, close };
  }

  it('opens from the Table button instead of inserting a fixed table', async () => {
    const { target, onrun, button, close } = open();
    expect(target.querySelector('.picker')).toBeNull();

    button.click();
    await tick();
    expect(onrun).not.toHaveBeenCalled();
    expect(target.querySelector('.picker')).not.toBeNull();
    expect(button.getAttribute('aria-expanded')).toBe('true');
    close();
  });

  it('inserts the size of the cell clicked', async () => {
    const { target, ontable, button, close } = open();
    button.click();
    await tick();

    const cell = target.querySelector<HTMLElement>('.cell[data-cols="4"][data-rows="3"]')!;
    cell.dispatchEvent(new MouseEvent('mouseenter'));
    await tick();
    expect(target.querySelector('.size')!.textContent!.trim()).toBe('4 × 3');
    expect(target.querySelectorAll('.cell.on').length).toBe(12);

    cell.click();
    await tick();
    expect(ontable).toHaveBeenCalledWith(4, 3);
    expect(target.querySelector('.picker')).toBeNull();
    close();
  });

  it('takes a size typed in the fields', async () => {
    const { target, ontable, button, close } = open();
    button.click();
    await tick();

    const [cols, rows] = target.querySelectorAll<HTMLInputElement>('.picker input');
    cols!.value = '25';
    cols!.dispatchEvent(new Event('input', { bubbles: true }));
    rows!.value = '6';
    rows!.dispatchEvent(new Event('input', { bubbles: true }));
    target.querySelector<HTMLFormElement>('.picker form')!.requestSubmit();
    await tick();
    expect(ontable).toHaveBeenCalledWith(25, 6);
    close();
  });

  it('closes on Escape without the editor hearing it, and on a click elsewhere', async () => {
    const { target, button, close } = open();
    button.click();
    await tick();

    const heard = vi.fn();
    document.addEventListener('keydown', heard);
    document.body.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );
    document.removeEventListener('keydown', heard);
    await tick();
    expect(target.querySelector('.picker')).toBeNull();
    expect(heard).not.toHaveBeenCalled();

    button.click();
    await tick();
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    await tick();
    expect(target.querySelector('.picker')).toBeNull();
    close();
  });
});

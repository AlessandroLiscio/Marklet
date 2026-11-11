import { describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Tree from '../../src/lib/sidebar/tree.svelte';
import type { Entry } from '../../src/lib/ipc';

const entries: Entry[] = [
  { path: 'notes', name: 'notes', dir: true, depth: 0, size: 0, mtime_ms: 0 },
  { path: 'notes/a.md', name: 'a.md', dir: false, depth: 1, size: 0, mtime_ms: 0 },
  { path: 'top.md', name: 'top.md', dir: false, depth: 0, size: 0, mtime_ms: 0 },
];

const tick = () => new Promise((r) => setTimeout(r, 0));

function setup(oncreate: (parent: string, name: string) => Promise<void>) {
  const target = document.createElement('div');
  document.body.append(target);
  const app = mount(Tree, {
    target,
    props: { entries, listed: null, oncreate },
  });
  return { target, done: () => (void unmount(app), target.remove()) };
}

function rightClick(el: Element): void {
  el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
  flushSync();
}

async function type(target: HTMLElement, name: string): Promise<void> {
  const input = target.ownerDocument.querySelector<HTMLInputElement>('.popup input')!;
  input.value = name;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  await tick();
}

describe('creating a note from the explorer', () => {
  it('right-clicking a folder offers New file in that folder', async () => {
    const oncreate = vi.fn().mockResolvedValue(undefined);
    const { target, done } = setup(oncreate);
    rightClick(target.querySelector('[data-path="notes"]')!);
    document.querySelector<HTMLButtonElement>('.popup [role="menuitem"]')!.click();
    flushSync();
    await type(target, 'idea');
    expect(oncreate).toHaveBeenCalledWith('notes', 'idea');
    expect(document.querySelector('.popup')).toBeNull();
    done();
  });

  it('right-clicking a file creates beside it, in its parent folder', async () => {
    const oncreate = vi.fn().mockResolvedValue(undefined);
    const { target, done } = setup(oncreate);
    // Expand `notes` so its child row exists.
    target.querySelector<HTMLElement>('[data-path="notes"]')!.click();
    flushSync();
    rightClick(target.querySelector('[data-path="notes/a.md"]')!);
    document.querySelector<HTMLButtonElement>('.popup [role="menuitem"]')!.click();
    flushSync();
    await type(target, 'b');
    expect(oncreate).toHaveBeenCalledWith('notes', 'b');
    done();
  });

  it('right-clicking empty space creates in the root', async () => {
    const oncreate = vi.fn().mockResolvedValue(undefined);
    const { target, done } = setup(oncreate);
    rightClick(target.querySelector('.scroller')!);
    document.querySelector<HTMLButtonElement>('.popup [role="menuitem"]')!.click();
    flushSync();
    await type(target, 'root');
    expect(oncreate).toHaveBeenCalledWith('', 'root');
    done();
  });

  it('keeps the field open and shows the refusal', async () => {
    const oncreate = vi.fn().mockRejectedValue({ message: 'top.md already exists in that folder' });
    const { target, done } = setup(oncreate);
    rightClick(target.querySelector('.scroller')!);
    document.querySelector<HTMLButtonElement>('.popup [role="menuitem"]')!.click();
    flushSync();
    await type(target, 'top');
    flushSync();
    expect(document.querySelector('.popup .problem')?.textContent).toContain('already exists');
    expect(document.querySelector('.popup input')).not.toBeNull();
    done();
  });
});

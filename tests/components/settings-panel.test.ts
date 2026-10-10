import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { DEFAULT_SETTINGS, PALETTE_HUES } from '../../src/lib/settings/model';
import type { Settings } from '../../src/lib/ipc';

const written: Settings[] = [];
let stored: Settings = { ...DEFAULT_SETTINGS };

vi.mock('../../src/lib/ipc', () => ({
  readSettings: () => Promise.resolve(stored),
  writeSettings: (s: Settings) => {
    written.push(s);
    return Promise.resolve();
  },
  checkUpdate: () => Promise.resolve(null),
  installUpdate: () => Promise.resolve(),
}));

// Imported after the mock is registered.
const { default: Panel } = await import('../../src/lib/settings/panel.svelte');

async function open(): Promise<{ root: HTMLElement; done: () => void }> {
  const root = document.createElement('div');
  document.body.append(root);
  const app = mount(Panel, { target: root });
  await Promise.resolve();
  await Promise.resolve();
  flushSync();
  (root.querySelector('button.toggle') as HTMLButtonElement).click();
  flushSync();
  return {
    root,
    done: () => {
      void unmount(app);
      root.remove();
    },
  };
}

beforeEach(() => {
  written.length = 0;
  stored = { ...DEFAULT_SETTINGS };
  window.matchMedia ??= ((q: string) => ({ matches: false, media: q })) as typeof window.matchMedia;
});

describe('the settings panel', () => {
  it('offers Light and Dark and nothing else for the theme', () => {
    return open().then(({ root, done }) => {
      const labels = [...root.querySelectorAll('[aria-label="Theme"] button')].map((b) => b.textContent?.trim());
      expect(labels).toEqual(['Light', 'Dark']);
      done();
    });
  });

  it('calls the typeface control Font, with Technical first', () => {
    return open().then(({ root, done }) => {
      expect(root.querySelector('label[for="typeface-select"]')?.textContent?.trim()).toBe('Font');
      const first = root.querySelector('#typeface-select option');
      expect(first?.textContent?.trim()).toBe('Technical');
      done();
    });
  });

  it('moves the hue slider to a palette when its chip is clicked', () => {
    return open().then(({ root, done }) => {
      const violet = [...root.querySelectorAll('.chips button')].find((b) => b.textContent?.trim() === 'Violet') as HTMLButtonElement;
      violet.click();
      flushSync();
      const slider = root.querySelector('#hue-range') as HTMLInputElement;
      expect(Number(slider.value)).toBe(PALETTE_HUES.violet);
      expect(written.at(-1)?.palette).toBe('violet');
      expect(written.at(-1)?.accent_hue).toBe(PALETTE_HUES.violet);
      done();
    });
  });

  it('has no separate Accent hue heading', () => {
    return open().then(({ root, done }) => {
      expect(root.textContent).not.toContain('Accent hue —');
      done();
    });
  });
});

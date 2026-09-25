// @vitest-environment happy-dom
/**
 * The acceptance requirement: **a read-only session loads zero CodeMirror
 * bytes.**
 *
 * Same shape as `tests/unit/rich-enrich.test.ts`, which proves the equivalent
 * property for hljs / KaTeX / Mermaid — the module that would pull the heavy
 * chunk is mocked at its boundary, and the test asserts nothing ever reaches
 * it. What is being tested is the *dispatch*: that creating the controller,
 * subscribing to keys, and pressing keys that are not F2/F3/Ctrl+E never
 * touches `./session`, which is the only path to `@codemirror/*`.
 *
 * Two other guards cover the halves this one cannot:
 *   - `npm run check:imports` fails on a *static* `import … from
 *     '@codemirror/…'` anywhere under `src/`, in either edition;
 *   - the chunk table from `npm run size` shows `codemirror` as its own
 *     output chunk, which is what makes "not loaded" observable at runtime.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';

const createSession = vi.fn(async (_opts: { path: string; decorated: boolean }) => ({
  view: {},
  cm: {},
  text: () => '',
  topLine: () => 1,
  goToLine: vi.fn(),
  cursor: () => ({ line: 1, column: 1 }),
  setDecorated: vi.fn(),
  flush: async () => {},
  destroy: vi.fn(),
}));

const linkScroll = vi.fn(() => ({
  onEditorScroll: vi.fn(),
  attach: vi.fn(),
  resync: vi.fn(),
  destroy: vi.fn(),
}));

vi.mock('../../src/lib/edit/session', () => ({ createSession, SAVE_DEBOUNCE_MS: 100 }));
vi.mock('../../src/lib/edit/split', () => ({ linkScroll, ECHO_MS: 150 }));

import { createEditController, editShortcut } from '../../src/lib/edit';

function controller(overrides: Record<string, unknown> = {}) {
  const docRoot = document.createElement('article');
  docRoot.id = 'doc';
  document.body.append(docRoot);

  return createEditController({
    docRoot,
    path: () => '/vault/note.md',
    readSource: async () => '# Note\n',
    ...overrides,
  });
}

function key(init: Partial<KeyboardEvent> & { key: string }): KeyboardEvent {
  return new KeyboardEvent('keydown', { cancelable: true, ...init });
}

afterEach(() => {
  createSession.mockClear();
  linkScroll.mockClear();
  document.body.replaceChildren();
  document.body.className = '';
});

describe('editShortcut', () => {
  it('maps the four editing keys', () => {
    expect(editShortcut(key({ key: 'F2' }))).toBe('live');
    expect(editShortcut(key({ key: 'F3' }))).toBe('split');
    expect(editShortcut(key({ key: 'F4' }))).toBe('external');
    expect(editShortcut(key({ key: 'Escape' }))).toBe('read');
    expect(editShortcut(key({ key: 'e', ctrlKey: true }))).toBe('external');
  });

  it('ignores everything else', () => {
    expect(editShortcut(key({ key: 'a' }))).toBeNull();
    expect(editShortcut(key({ key: 'F5' }))).toBeNull();
    expect(editShortcut(key({ key: 'e' }))).toBeNull();
  });

  it('leaves a modified function key to whoever else wants it', () => {
    expect(editShortcut(key({ key: 'F2', ctrlKey: true }))).toBeNull();
    expect(editShortcut(key({ key: 'F3', altKey: true }))).toBeNull();
    expect(editShortcut(key({ key: 'F2', shiftKey: true }))).toBeNull();
  });

  it('does not steal Ctrl+E from a text field', () => {
    // The settings panel has inputs. A shortcut that eats keystrokes in a form
    // is a bug the user cannot diagnose.
    const input = document.createElement('input');
    document.body.append(input);
    const event = key({ key: 'e', ctrlKey: true });
    Object.defineProperty(event, 'target', { value: input });
    expect(editShortcut(event)).toBeNull();
  });
});

describe('a read-only session loads zero CodeMirror bytes', () => {
  it('creates no session on construction', () => {
    controller();
    expect(createSession).not.toHaveBeenCalled();
    expect(linkScroll).not.toHaveBeenCalled();
  });

  it('creates no session for keys that are not editing keys', () => {
    const edit = controller();

    for (const k of ['a', 'F5', 'ArrowDown', 'Enter', 'F2']) {
      // F2 included with a modifier, which must NOT enter edit mode.
      const consumed = edit.handleKey(key(k === 'F2' ? { key: k, ctrlKey: true } : { key: k }));
      expect(consumed, k).toBe(false);
    }

    expect(createSession).not.toHaveBeenCalled();
  });

  it('does not consume Escape while already reading', () => {
    const edit = controller();
    expect(edit.handleKey(key({ key: 'Escape' }))).toBe(false);
    expect(createSession).not.toHaveBeenCalled();
  });

  it('loads the editor only when F2 is pressed', async () => {
    const edit = controller();

    expect(edit.handleKey(key({ key: 'F2' }))).toBe(true);
    await edit.setMode('live');

    expect(createSession).toHaveBeenCalledTimes(1);
    expect(createSession.mock.calls[0]?.[0]).toMatchObject({
      path: '/vault/note.md',
      decorated: true,
    });
    expect(document.body.classList.contains('marklet-mode-live')).toBe(true);
  });

  it('F3 builds the scroll link; F2 does not', async () => {
    const edit = controller();
    await edit.setMode('split');

    expect(createSession).toHaveBeenCalledTimes(1);
    expect(createSession.mock.calls[0]?.[0]).toMatchObject({ decorated: false });
    expect(linkScroll).toHaveBeenCalledTimes(1);
    expect(document.body.classList.contains('marklet-mode-split')).toBe(true);
  });

  it('switching F2 to F3 reuses the one editor rather than rebuilding it', async () => {
    const edit = controller();
    await edit.setMode('live');
    await edit.setMode('split');

    // One component serves both modes — that is what makes it affordable, and
    // rebuilding would throw away undo history and the cursor.
    expect(createSession).toHaveBeenCalledTimes(1);
    expect(edit.mode()).toBe('split');
  });

  it('never opens without a document', async () => {
    const edit = controller({ path: () => null });
    await edit.setMode('live');
    expect(createSession).not.toHaveBeenCalled();
    expect(edit.mode()).toBe('read');
  });
});

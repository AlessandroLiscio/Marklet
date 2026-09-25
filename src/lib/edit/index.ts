/**
 * The editing layer's front door — and the reason a read-only session
 * downloads **zero bytes** of CodeMirror.
 *
 * Everything this module imports statically is plain arithmetic over strings
 * and line numbers. The editor itself (`./session`, which pulls
 * `@codemirror/*` through `./cm`) is reached only by `import()`, and only from
 * inside {@link EditController.setMode} — so the code path that loads it does
 * not exist until F2, F3 or Ctrl+E is pressed. `npm run check:imports` proves
 * the static half; `tests/unit/edit-lazy.test.ts` proves the dynamic half.
 *
 * The four keys, and what each one is for:
 *
 * | Key        | Mode     | What it is                                        |
 * |------------|----------|---------------------------------------------------|
 * | `F2`       | live     | markdown source with the syntax hidden off-cursor  |
 * | `F3`       | split    | plain source left, rendered preview right, synced  |
 * | `F4`/`^E`  | external | hand the file to `MD_EDITOR` at the current line   |
 * | `Escape`   | read     | back to reading                                    |
 */
import { scrollToLine, visibleLine } from '../doc';
import type { Session } from './session';
import type { ScrollLink } from './split';

/** What the document area is currently showing. */
export type EditMode = 'read' | 'live' | 'split';

/** What a keystroke asked for. `external` is an action, not a mode. */
export type EditIntent = EditMode | 'external';

/**
 * The keystroke-to-intent mapping, as a pure function so it can be tested
 * without a window and read without one either.
 *
 * A modifier on `F2`/`F3`/`F4` means the user is talking to something else
 * (the browser, the window manager, a future shortcut), so those are left
 * alone rather than swallowed. `Ctrl+E` is ignored inside a text field for the
 * same reason: the settings panel has inputs, and a shortcut that eats
 * keystrokes in a form is a bug the user cannot diagnose.
 */
export function editShortcut(event: KeyboardEvent): EditIntent | null {
  if (event.altKey || event.metaKey) return null;

  const target = event.target as HTMLElement | null;
  const inField =
    target !== null &&
    (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable);

  if (!event.ctrlKey && !event.shiftKey) {
    if (event.key === 'F2') return 'live';
    if (event.key === 'F3') return 'split';
    if (event.key === 'F4') return 'external';
    if (event.key === 'Escape') return 'read';
  }

  if (event.ctrlKey && !event.shiftKey && event.key.toLowerCase() === 'e' && !inField) {
    return 'external';
  }

  return null;
}

/**
 * Everything the controller cannot reach on its own.
 *
 * Three of these are callbacks rather than direct IPC calls because their Rust
 * commands do not exist yet: this task owns `splice_range` and nothing else on
 * that side of the boundary. Their signatures are in the P7 receipt; wiring
 * them is a one-line change each, here.
 */
export interface EditWiring {
  /** The `<article id="doc">` the renderer fills. */
  docRoot: HTMLElement;
  /** The open document's canonical path. */
  path(): string | null;
  /** Reads the markdown source of the open document. Needs `read_source`. */
  readSource: () => Promise<string>;
  /**
   * Opens the document in the user's editor at a position.
   *
   * A line and a column, not a program and its arguments: which editor runs is
   * decided in `src-tauri/src/editor.rs` from `MD_EDITOR`, because spawning a
   * process is the widest privilege this app has and the webview must not be
   * the thing that names the executable.
   */
  revealInEditor?: (line: number, column: number) => Promise<void>;
  /** Writes clipboard bytes beside the document. Needs `save_pasted_image`. */
  savePastedImage?: (bytes: Uint8Array, ext: string) => Promise<string>;
  /** Called after each save, so the preview can be re-rendered. */
  onSaved?: (length: number) => void;
  /** Called when something the user should hear about went wrong. */
  onError?: (message: string) => void;
  /** Called whenever the mode changes, for the chrome to reflect it. */
  onMode?: (mode: EditMode) => void;
}

export interface EditController {
  mode(): EditMode;
  /** Handle a `keydown`. Returns `true` when it consumed the event. */
  handleKey(event: KeyboardEvent): boolean;
  setMode(mode: EditMode): Promise<void>;
  /** Re-aligns the split columns after the preview was re-rendered. */
  resync(): void;
  destroy(): Promise<void>;
}

/** The class the two editing layouts are styled from. See `editor.css`. */
const BODY_CLASS: Record<Exclude<EditMode, 'read'>, string> = {
  live: 'marklet-mode-live',
  split: 'marklet-mode-split',
};

export function createEditController(wiring: EditWiring): EditController {
  let mode: EditMode = 'read';
  let session: Session | null = null;
  let link: ScrollLink | null = null;
  let host: HTMLElement | null = null;
  /** Serializes mode changes: F2 and F3 pressed in the same frame must queue. */
  let queue: Promise<void> = Promise.resolve();

  function announce(): void {
    wiring.onMode?.(mode);
  }

  function applyBodyClass(): void {
    const body = document.body;
    body.classList.remove(BODY_CLASS.live, BODY_CLASS.split);
    if (mode !== 'read') body.classList.add(BODY_CLASS[mode]);
  }

  async function enter(next: Exclude<EditMode, 'read'>): Promise<void> {
    const path = wiring.path();
    if (path === null) return;

    // Already editing: switching between F2 and F3 is a decoration toggle and
    // a class swap, never a rebuild. Rebuilding would drop undo history and
    // the cursor, which is the whole reason one component serves both modes.
    if (session !== null) {
      mode = next;
      session.setDecorated(next === 'live');
      applyBodyClass();
      if (next === 'split') {
        const { linkScroll } = await import('./split');
        link ??= linkScroll(wiring.docRoot);
        link.attach(session);
        link.resync();
      } else {
        link?.destroy();
        link = null;
      }
      announce();
      return;
    }

    // Anchored to the nearest `data-l`, never a pixel offset — the same rule
    // `settings/model.ts` follows across a reflow, and for the same reason:
    // the metrics on either side of this toggle are not the same metrics.
    const line = visibleLine(wiring.docRoot);
    const source = await wiring.readSource();

    const [{ createSession }, { linkScroll }] = await Promise.all([
      import('./session'),
      import('./split'),
    ]);

    host = document.createElement('div');
    host.id = 'editor';
    document.body.append(host);

    const scrollLink = next === 'split' ? linkScroll(wiring.docRoot) : null;

    const created = await createSession({
      host,
      path,
      source,
      decorated: next === 'live',
      line,
      readSource: wiring.readSource,
      ...(wiring.savePastedImage ? { savePastedImage: wiring.savePastedImage } : {}),
      ...(wiring.onSaved ? { onSaved: wiring.onSaved } : {}),
      ...(wiring.onError ? { onError: wiring.onError } : {}),
      ...(scrollLink ? { onScroll: (l: number) => scrollLink.onEditorScroll(l) } : {}),
    });

    session = created;
    link = scrollLink;
    scrollLink?.attach(created);

    mode = next;
    applyBodyClass();
    announce();
  }

  async function leave(): Promise<void> {
    if (session === null) {
      mode = 'read';
      applyBodyClass();
      announce();
      return;
    }

    const line = session.topLine();
    await session.flush();

    link?.destroy();
    link = null;
    session.destroy();
    session = null;
    host?.remove();
    host = null;

    mode = 'read';
    applyBodyClass();

    // Two frames, then restore: the first commits the layout change the class
    // removal caused, the second lets it settle before anchor positions are
    // read against it.
    requestAnimationFrame(() => requestAnimationFrame(() => scrollToLine(line)));
    announce();
  }

  async function external(): Promise<void> {
    const path = wiring.path();
    if (path === null || !wiring.revealInEditor) return;

    const at = session?.cursor() ?? { line: visibleLine(wiring.docRoot) || 1, column: 1 };
    await session?.flush();
    await wiring.revealInEditor(at.line, at.column);
  }

  function run(job: () => Promise<void>): Promise<void> {
    queue = queue.then(job).catch((error: unknown) => {
      wiring.onError?.(error instanceof Error ? error.message : 'the editor could not open');
    });
    return queue;
  }

  return {
    mode: () => mode,

    handleKey(event) {
      const intent = editShortcut(event);
      if (intent === null) return false;

      if (intent === 'external') {
        event.preventDefault();
        void run(external);
        return true;
      }

      // Escape while already reading belongs to whatever else is listening —
      // a settings panel, a search box — so it is not consumed here.
      if (intent === 'read' && mode === 'read') return false;

      event.preventDefault();
      void this.setMode(intent === mode ? 'read' : intent);
      return true;
    },

    setMode(next) {
      return run(() => (next === 'read' ? leave() : enter(next)));
    },

    resync() {
      link?.resync();
    },

    destroy() {
      return run(leave);
    },
  };
}

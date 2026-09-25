/**
 * One editing session: a CodeMirror view over the markdown source, and the
 * save path that keeps the file byte-exact.
 *
 * **The buffer is the document.** There is no model, no AST, no serializer —
 * what CodeMirror holds is the same text that is on disk, and saving is
 * `splice_range` over the one range that changed. Everything the round-trip
 * promise is made of lives in those two sentences; see
 * `.claude/skills/tauri-ipc/SKILL.md` for the command and `./splice.ts` for
 * the range arithmetic.
 *
 * The same session serves **F2** (live preview: decorations on) and **F3**
 * (dual column: decorations off, plain source next to the rendered preview).
 * That is not an economy of code, it is the reason the feature fits the
 * budget at all: one editor, one chunk, paid for once.
 */
import { spliceRange, isIpcError } from '../ipc';
import type { CodeMirror, EditorView, Extension } from './cm';
import { loadCodeMirror } from './cm';
import { livePreview } from './livepreview';
import { extensionFor, imageMarkdown, pickImage } from './paste';
import { expandSnippet } from './snippets';
import { computeSplice, utf8Length } from './splice';
import './editor.css';

/**
 * How long after the last keystroke a save runs.
 *
 * 100 ms is the number the F3 dual column needs: the preview is rendered by
 * Rust *from the file*, so the file is what has to be current, and a slower
 * save would show a preview of text the user finished typing half a second
 * ago. It is fast enough to feel continuous and slow enough that a burst of
 * typing is one write rather than forty.
 */
export const SAVE_DEBOUNCE_MS = 100;

/** How many times a save re-reads and retries after a `Conflict` before giving up. */
const MAX_RETRIES = 2;

export interface SessionOptions {
  /** Where the editor mounts. Emptied on create and on destroy. */
  host: HTMLElement;
  /** The document's canonical path, as `OpenedDocument.path` spells it. */
  path: string;
  /** The markdown source, exactly as it is on disk. */
  source: string;
  /** F2 hides syntax; F3 shows it. */
  decorated: boolean;
  /** 1-based line to place the cursor and the viewport on. */
  line?: number;
  /**
   * Re-reads the file after a `Conflict`.
   *
   * Injected rather than imported: the command that does it (`read_source`)
   * is not written yet — this task owns `splice_range` and nothing else on the
   * Rust side. Wiring it is a one-line change at the call site.
   */
  readSource: () => Promise<string>;
  /**
   * Writes clipboard bytes next to the document and returns the path to
   * reference, relative to the document. Same reasoning as `readSource`: the
   * command is `save_pasted_image`, and it is reported, not written, here.
   */
  savePastedImage?: (bytes: Uint8Array, ext: string) => Promise<string>;
  /** Called after every successful save, with the file's new byte length. */
  onSaved?: (length: number) => void;
  /** Called when a save fails for a reason the user should hear about. */
  onError?: (message: string) => void;
  /** Called when the editor is scrolled, with the topmost visible source line. */
  onScroll?: (line: number) => void;
}

export interface Session {
  readonly view: EditorView;
  readonly cm: CodeMirror;
  /** The buffer's current text. */
  text(): string;
  /** The topmost visible 1-based source line. */
  topLine(): number;
  /** Scrolls so `line` is at the top. Does not move the cursor. */
  goToLine(line: number): void;
  /** The 1-based line and column of the primary cursor — what F4 hands the editor. */
  cursor(): { line: number; column: number };
  /** Turns live-preview decorations on or off without rebuilding the editor. */
  setDecorated(on: boolean): void;
  /** Saves now, rather than waiting out the debounce. */
  flush(): Promise<void>;
  destroy(): void;
}

/**
 * Creates the editor. The `import()` of CodeMirror happens here and nowhere
 * else on the way in, so a session that is never opened costs nothing.
 */
export async function createSession(options: SessionOptions): Promise<Session> {
  const cm = await loadCodeMirror();

  const { EditorState, Compartment } = cm.state;
  const { EditorView, keymap, drawSelection, highlightSpecialChars, rectangularSelection } = cm.view;
  const { defaultKeymap, history, historyKeymap, indentWithTab } = cm.commands;
  const { syntaxHighlighting, defaultHighlightStyle, indentOnInput, bracketMatching } = cm.language;
  const { markdown, markdownLanguage } = cm.markdown;

  const decorations = new Compartment();

  /** The text as Rust last confirmed it, and its length in bytes. */
  let baseline = options.source;
  let baselineBytes = utf8Length(options.source);
  let saving: Promise<void> = Promise.resolve();
  let timer: ReturnType<typeof setTimeout> | null = null;
  let destroyed = false;

  /**
   * Writes the difference between `baseline` and the buffer.
   *
   * On `Conflict` the file changed underneath — an external editor, a `git
   * checkout`, the watcher's own source of truth — so the fix is to re-read
   * and recompute the range against what is *now* on disk. Recomputing rather
   * than replaying is what keeps the external change: the new splice covers
   * only the region that still differs, so an edit somebody made in another
   * part of the file survives untouched.
   */
  async function save(attempt = 0): Promise<void> {
    if (destroyed) return;

    const current = view.state.doc.toString();
    const splice = computeSplice(baseline, current);
    if (splice === null) return;

    try {
      const result = await spliceRange(
        options.path,
        splice.start,
        splice.end,
        splice.replacement,
        baselineBytes,
      );
      baseline = current;
      baselineBytes = result.len;
      options.onSaved?.(result.len);
    } catch (error) {
      if (isIpcError(error) && error.kind === 'conflict' && attempt < MAX_RETRIES) {
        baseline = await options.readSource();
        baselineBytes = utf8Length(baseline);
        await save(attempt + 1);
        return;
      }
      const message = isIpcError(error) ? error.message : 'could not save the document';
      options.onError?.(message);
    }
  }

  function schedule(): void {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      // Serialized rather than concurrent: two saves in flight would compute
      // their ranges against the same baseline and the second would be stale
      // the instant the first succeeded.
      saving = saving.then(() => save());
    }, SAVE_DEBOUNCE_MS);
  }

  /**
   * Tab: expand a snippet if the word before the cursor is one, otherwise fall
   * through. Returning `false` matters — swallowing Tab unconditionally would
   * break indenting a nested list, which is what Tab does the rest of the time.
   */
  function tabExpands(target: EditorView): boolean {
    const { state } = target;
    const range = state.selection.main;
    if (!range.empty) return false;

    const line = state.doc.lineAt(range.head);
    const expansion = expandSnippet(line.text, range.head - line.from);
    if (expansion === null) return false;

    target.dispatch({
      changes: {
        from: line.from + expansion.from,
        to: line.from + expansion.to,
        insert: expansion.insert,
      },
      selection: { anchor: line.from + expansion.cursor },
      scrollIntoView: true,
    });
    return true;
  }

  function onPaste(event: ClipboardEvent, target: EditorView): boolean {
    const store = options.savePastedImage;
    if (!store) return false;

    const items = Array.from(event.clipboardData?.items ?? []).filter((i) => i.kind === 'file');
    if (items.length === 0) return false;

    const picked = pickImage(items.map((i) => ({ type: i.type })));
    if (picked === null) return false;

    const item = items.find((i) => i.type.toLowerCase() === picked.type);
    const file = item?.getAsFile();
    if (!file) return false;

    event.preventDefault();
    void file
      .arrayBuffer()
      .then((buffer) => store(new Uint8Array(buffer), extensionFor(picked.type) ?? picked.ext))
      .then((relative) => {
        if (destroyed) return;
        target.dispatch(target.state.replaceSelection(imageMarkdown(relative)));
      })
      .catch(() => options.onError?.('could not save the pasted image'));
    return true;
  }

  const extensions: Extension[] = [
    history(),
    drawSelection(),
    highlightSpecialChars(),
    rectangularSelection(),
    indentOnInput(),
    bracketMatching(),
    // `markdownLanguage` rather than the CommonMark default: it carries the
    // GFM extensions the render core enables — tables, task lists,
    // strikethrough — and `syntax.ts`'s rules are written against those node
    // names. No `codeLanguages`, deliberately: nested grammars for fenced code
    // would pull a second set of parsers into the chunk to syntax-highlight
    // text the reader already sees highlighted in the preview.
    markdown({ base: markdownLanguage }),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    EditorView.lineWrapping,
    EditorState.allowMultipleSelections.of(true),
    keymap.of([{ key: 'Tab', run: tabExpands }, ...defaultKeymap, ...historyKeymap, indentWithTab]),
    decorations.of(options.decorated ? livePreview(cm) : []),
    EditorView.domEventHandlers({
      paste: onPaste,
      scroll: (_event, target) => {
        options.onScroll?.(lineAtTop(target));
        return false;
      },
    }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) schedule();
    }),
  ];

  function lineAtTop(target: EditorView): number {
    const block = target.lineBlockAtHeight(target.scrollDOM.scrollTop);
    return target.state.doc.lineAt(block.from).number;
  }

  options.host.replaceChildren();
  options.host.classList.add('marklet-editor');

  const view = new EditorView({
    state: EditorState.create({ doc: options.source, extensions }),
    parent: options.host,
  });

  const session: Session = {
    view,
    cm,
    text: () => view.state.doc.toString(),
    topLine: () => lineAtTop(view),
    goToLine(line) {
      const n = Math.max(1, Math.min(view.state.doc.lines, Math.round(line)));
      view.dispatch({
        effects: EditorView.scrollIntoView(view.state.doc.line(n).from, { y: 'start' }),
      });
    },
    cursor() {
      const head = view.state.selection.main.head;
      const line = view.state.doc.lineAt(head);
      return { line: line.number, column: head - line.from + 1 };
    },
    setDecorated(on) {
      view.dispatch({
        effects: decorations.reconfigure(on ? livePreview(cm) : []),
      });
    },
    async flush() {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      saving = saving.then(() => save());
      await saving;
    },
    destroy() {
      destroyed = true;
      if (timer !== null) clearTimeout(timer);
      view.destroy();
      options.host.classList.remove('marklet-editor');
      options.host.replaceChildren();
    },
  };

  if (options.line !== undefined) {
    const n = Math.max(1, Math.min(view.state.doc.lines, Math.round(options.line)));
    const pos = view.state.doc.line(n).from;
    view.dispatch({
      selection: { anchor: pos },
      effects: EditorView.scrollIntoView(pos, { y: 'start' }),
    });
  }

  return session;
}

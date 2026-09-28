<script lang="ts">
  /**
   * The chrome — and, since the close of wave W4, the place the pieces meet.
   *
   * Never the document itself. That lives in a plain `<article id="doc">`
   * outside this tree, because a 3 MB markdown file must not go through a
   * reactive renderer. What this component owns is everything *around* it:
   * the outline, the settings panel, the vault sidebar, and the two events
   * that swap the document underneath them.
   */
  import { onDestroy, onMount } from 'svelte';

  import ActivityBar, { type PanelId } from './lib/chrome/activity-bar.svelte';
  import { currentDocument, setDocument } from './lib/doc';
  import { createEditController, type EditController, type EditMode } from './lib/edit';
  import Outline from './lib/outline.svelte';
  import SettingsPanel from './lib/settings/panel.svelte';
  import Sidebar from './lib/sidebar/sidebar.svelte';
  import {
    exportHtml,
    exportPdf,
    isIpcError,
    openDocument,
    openExternal,
    openNote,
    pickFile,
    pickFolder,
    readSource,
    revealInEditor,
    savePastedImage,
    type OpenedDocument,
  } from './lib/ipc';

  let doc = $state<OpenedDocument | null>(currentDocument());
  let mode = $state<EditMode>('read');
  /** The one transient line of feedback: an export's result, or a refusal. */
  let notice = $state<{ text: string; bad: boolean } | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | null = null;
  let edit: EditController | null = null;
  /** Serializes exports: two Ctrl+P in a row must not print into each other. */
  let exporting = false;
  /**
   * The folder the explorer is rooted at.
   *
   * A directory argument sets it at launch. Otherwise it becomes the open
   * document's own folder, the first time the explorer is opened — which is
   * what makes the tree reachable for anyone who double-clicked a file, rather
   * than only for someone who knew to launch on a directory.
   */
  let vaultPath = $state<string | null>(
    typeof window !== 'undefined' ? (window.__MARKLET_VAULT__ ?? null) : null
  );

  /** Which side panel is showing, or `null` for the document at full width. */
  let panel = $state<PanelId | null>(
    typeof window !== 'undefined' && window.__MARKLET_SETTINGS__ === true ? 'settings' : null
  );

  /**
   * How much room the fixed chrome takes, published to `:root` for
   * `content.css` to inset the body by.
   *
   * On the root element rather than on a component, because the thing being
   * inset — `#doc` — is outside the Svelte tree entirely, which is invariant 2
   * in CLAUDE.md. A custom property is the only channel between them.
   */
  $effect(() => {
    document.documentElement.style.setProperty(
      '--chrome-inset',
      panel === null ? 'var(--activity-bar-size)' : 'calc(var(--activity-bar-size) + 260px)'
    );
  });

  /** The folder of a document path, for rooting the explorer. */
  function folderOf(path: string): string {
    const cut = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return cut > 0 ? path.slice(0, cut) : path;
  }

  let unlisten: Array<() => void> = [];

  function say(text: string, bad = false): void {
    notice = { text, bad };
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => {
      notice = null;
    }, bad ? 6000 : 3500);
  }

  /** An `IpcError` reads better than whatever `String(error)` would produce. */
  function reason(error: unknown): string {
    if (isIpcError(error)) return error.message;
    return error instanceof Error ? error.message : 'that did not work';
  }

  /**
   * Swaps the document and re-runs the rich-render pass.
   *
   * `enrich` is reached through `import()` and never from a static import: it
   * pulls highlight.js, KaTeX and — in the full edition — Mermaid, and a
   * document containing none of those must download none of them. The import
   * itself is cheap; what it loads is decided by sniffing inside `enrich`.
   */
  async function show(next: OpenedDocument) {
    const root = document.getElementById('doc');
    if (!root) return;

    setDocument(root, next);
    doc = next;
    document.title = `${next.title} — Marklet`;

    const { enrich } = await import('./lib/rich');
    await enrich(root);
  }

  /**
   * Wiki-links are ordinary anchors with a real `marklet://` href, so a plain
   * click would make the webview *navigate* to the raw markdown instead of
   * rendering it. Intercepting here — one delegated listener rather than one
   * per link — keeps that out of the render core, which has no idea a window
   * exists.
   */
  async function onDocClick(event: MouseEvent) {
    const anchor = (event.target as HTMLElement | null)?.closest<HTMLAnchorElement>('a[href]');

    // An external link must leave the application, not replace it.
    //
    // Left alone, this webview navigates: the app becomes a website, with no
    // address bar and no way back, and the document is gone. That is also the
    // shape of an attack — a markdown file that quietly swaps the app for a
    // page dressed as it — so every http(s) link is handed to the OS browser
    // instead. Rust re-checks the scheme; this is the convenience half, not
    // the security half.
    if (anchor && !anchor.classList.contains('wikilink')) {
      const href = anchor.getAttribute('href') ?? '';
      if (/^https?:\/\//i.test(href)) {
        event.preventDefault();
        void openExternal(href).catch((error: unknown) => say(reason(error), true));
        return;
      }
    }

    const target = anchor?.classList.contains('wikilink') ? anchor : null;
    if (!target) return;

    const note = target.dataset['target'];
    if (!note || target.classList.contains('unresolved')) {
      // An unresolved wiki-link points at a note nobody has written yet. Doing
      // nothing is the honest response; navigating to a 404 is not.
      event.preventDefault();
      return;
    }

    event.preventDefault();
    try {
      await show((await openNote(note)) as OpenedDocument);
    } catch {
      /* the sidebar surfaces vault errors; a dead link is not worth a dialog */
    }
  }

  /**
   * PDF and standalone HTML, both from **what is on screen**.
   *
   * Not from the file on disk: KaTeX and Mermaid have run in this webview and
   * nowhere else, so printing or serializing the live document is the only way
   * either format contains them. `MD_HTML=1` produces the other thing — the
   * same document without them — and that difference is the whole reason the
   * two paths exist separately (see `src-tauri/src/export/html.rs`).
   */
  async function runExport(format: 'pdf' | 'html'): Promise<void> {
    const root = document.getElementById('doc');
    if (!doc || !root || exporting) return;

    exporting = true;
    say(format === 'pdf' ? 'Printing to PDF…' : 'Writing standalone HTML…');
    try {
      // print.css is loaded here rather than at boot on purpose. It is ~5 KiB
      // gzipped of rules that are entirely inside `@media print`, and
      // `src/styles/**` is gated at 12 KiB gzipped for the lite edition
      // (`.claude/skills/size-budget/SKILL.md`). Importing it at the moment of
      // an export costs the reader nothing and gets it into the document
      // before the native print engine looks at the page.
      await import('./styles/print.css');

      if (format === 'pdf') {
        say(`Saved ${await exportPdf(doc.path)}`);
      } else {
        const { serializeEnrichedDocument } = await import('./lib/rich/export');
        const { bodyHtml, extraCss } = await serializeEnrichedDocument(root);
        say(`Saved ${await exportHtml(doc.path, bodyHtml, extraCss)}`);
      }
    } catch (error) {
      say(reason(error), true);
    } finally {
      exporting = false;
    }
  }

  /**
   * Export shortcuts, checked **after** the editor has had the keystroke.
   *
   * `Ctrl+P` is the one everyone already knows, and Marklet's answer to it is
   * a file rather than a dialog. `Ctrl+Shift+S` is "save a copy that works
   * anywhere". Neither fires while a text field has focus — the settings
   * panel and the search box both have inputs.
   */
  function exportShortcut(event: KeyboardEvent): boolean {
    if (!event.ctrlKey || event.altKey || event.metaKey) return false;

    const target = event.target as HTMLElement | null;
    if (
      target !== null &&
      (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)
    ) {
      return false;
    }

    const key = event.key.toLowerCase();
    if (key === 'p' && !event.shiftKey) {
      event.preventDefault();
      void runExport('pdf');
      return true;
    }
    if (key === 's' && event.shiftKey) {
      event.preventDefault();
      void runExport('html');
      return true;
    }
    return false;
  }

  /**
   * A diagram asked to be written beside the document.
   *
   * Raised by `src/lib/rich/mermaid.ts`'s fullscreen viewer, which has the SVG
   * but knows nothing about which file is open and must not call `invoke`
   * itself. The bytes go through the same command a pasted image does: it
   * already writes into `assets/`, already picks a non-colliding name, and
   * already refuses anything that is not an image format.
   */
  function onSaveAsset(event: Event): void {
    const detail = (event as CustomEvent<{ bytes?: Uint8Array; ext?: string; error?: string }>)
      .detail;
    if (detail.error) {
      say(detail.error, true);
      return;
    }
    if (!doc || !detail.bytes || !detail.ext) return;
    void savePastedImage(doc.path, detail.bytes, detail.ext)
      .then((relative) => say(`Saved ${relative}`))
      .catch((error: unknown) => say(reason(error), true));
  }

  /**
   * Opens the explorer, rooting it at the document's folder the first time.
   *
   * Rooting happens here rather than at launch because listing a folder is
   * work, and until this button is pressed nobody has asked for it.
   */
  function showExplorer(): void {
    if (vaultPath === null && doc) vaultPath = folderOf(doc.path);
    panel = 'explorer';
  }

  /** The activity bar's split button. `F3` does the same thing. */
  function toggleSplit(): void {
    void edit?.setMode(mode === 'split' ? 'read' : 'split');
  }

  /** Nothing was opened: ask the OS for a file, or for a folder to browse. */
  async function openFromDialog(kind: 'file' | 'folder'): Promise<void> {
    try {
      const picked = kind === 'file' ? await pickFile() : await pickFolder();
      if (picked === null) return; // cancelled, which is a normal answer
      if (kind === 'folder') {
        vaultPath = picked;
        panel = 'explorer';
      } else {
        await show(await openDocument(picked));
      }
    } catch (error) {
      say(reason(error), true);
    }
  }

  function onKeyDown(event: KeyboardEvent): void {
    if (edit?.handleKey(event)) return;
    exportShortcut(event);
  }

  onMount(() => {
    const root = document.getElementById('doc');
    if (root) {
      edit = createEditController({
        docRoot: root,
        path: () => doc?.path ?? null,
        readSource: () => readSource(doc?.path ?? ''),
        revealInEditor: async (line, column) => {
          say(`Opened in ${await revealInEditor(doc?.path ?? '', line, column)}`);
        },
        savePastedImage: (bytes, ext) => savePastedImage(doc?.path ?? '', bytes, ext),
        onMode: (next) => {
          mode = next;
        },
        onError: (message) => say(message, true),
      });
      window.addEventListener('keydown', onKeyDown);
      document.addEventListener('marklet-save-asset', onSaveAsset as EventListener);
    }
    if (root) {
      // Enrich whatever the boot script already put on screen. The first paint
      // happened before this component existed — that is the point of the boot
      // injection — so this pass decorates it rather than producing it.
      void import('./lib/rich').then(({ enrich }) => enrich(root));
      root.addEventListener('click', onDocClick);
    }

    void (async () => {
      const { listen } = await import('@tauri-apps/api/event');

      // The file changed on disk. Re-read rather than patch: the renderer is
      // fast enough that diffing would be more code for less certainty.
      unlisten.push(
        await listen('file-changed', async () => {
          if (!doc) return;
          await show(await openDocument(doc.path));
          // The preview is a new DOM tree, so the split columns' line map is
          // stale until it is measured again. Harmless in read mode.
          edit?.resync();
        }),
      );

      // A second launch forwarded its path here instead of starting another
      // process, which is what `tauri-plugin-single-instance` buys us.
      unlisten.push(
        await listen<string>('open-file', async (event) => {
          await show(await openDocument(event.payload));
        }),
      );
    })();

    return () => {
      root?.removeEventListener('click', onDocClick);
      window.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('marklet-save-asset', onSaveAsset as EventListener);
    };
  });

  onDestroy(() => {
    for (const off of unlisten) off();
    unlisten = [];
    if (noticeTimer) clearTimeout(noticeTimer);
    void edit?.destroy();
    edit = null;
  });
</script>

<div class="chrome-layer">
  <ActivityBar
    {panel}
    split={mode === 'split'}
    onpanel={(next) => (next === 'explorer' ? showExplorer() : (panel = next))}
    onsplit={toggleSplit}
  />

  {#if panel !== null}
    <div class="panel-dock">
      {#if panel === 'explorer'}
        {#if vaultPath}
          <Sidebar
            {vaultPath}
            active={doc?.path ?? null}
            onopen={(path) => void openNote(path).then((d) => show(d as OpenedDocument))}
          />
        {:else}
          <div class="empty-panel">
            <p>No folder open.</p>
            <button type="button" onclick={() => void openFromDialog('folder')}>
              Open a folder…
            </button>
          </div>
        {/if}
      {:else if panel === 'outline'}
        <Outline open />
      {:else}
        <SettingsPanel open />
      {/if}
    </div>
  {/if}
</div>

{#if !doc}
  <!-- Launched with nothing. An empty window with no explanation is the worst
       version of this; the OS file picker is the one people already know. -->
  <div class="welcome">
    <h1>Marklet</h1>
    <p>Open a Markdown file, or a folder to browse.</p>
    <div class="welcome-actions">
      <button type="button" onclick={() => void openFromDialog('file')}>Open file…</button>
      <button type="button" class="secondary" onclick={() => void openFromDialog('folder')}>
        Open folder…
      </button>
    </div>
  </div>
{/if}

{#if notice}
  <!-- One line, self-clearing. A modal for "saved a file" would be worse than
       the silence it replaces; a refusal still has to be readable, so it stays
       up longer and is coloured. -->
  <p class="notice" class:bad={notice.bad} role="status" aria-live="polite">{notice.text}</p>
{/if}

{#if doc}
  <footer class="status" aria-label="Document status">
    {#if mode !== 'read'}
      <span class="status-item status-mode">{mode === 'live' ? 'live edit' : 'split'}</span>
    {/if}
    <span class="status-item">{doc.outline.length} headings</span>
    <span class="status-item">{doc.line_map.length} blocks</span>
    {#if doc.encoding !== 'utf8'}
      <!-- Surfaced rather than silently repaired: a reader seeing mojibake
           deserves to know the file is not UTF-8, and `unknown` means the file
           contradicted its own byte-order mark. -->
      <span class="status-item status-warn">{doc.encoding}</span>
    {/if}
  </footer>
{/if}

<style>
  /* The chrome is a fixed layer on the left edge, and the document is inset
     by exactly its width. Before this the sidebar was a plain flex column in
     normal document flow, which put it *below* the document — nobody had seen
     it, because reaching it needed launching on a directory. */
  .chrome-layer {
    position: fixed;
    inset-block: 0;
    inset-inline-start: 0;
    display: flex;
    z-index: 20;
    font-family: var(--font-ui);
  }

  .panel-dock {
    inline-size: 260px;
    block-size: 100%;
    overflow: hidden;
    background: var(--bg-subtle);
    border-inline-end: 1px solid var(--border);
  }

  .empty-panel {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-4);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }

  .welcome {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding-inline-start: var(--activity-bar-size);
    text-align: center;
    font-family: var(--font-ui);
    background: var(--bg);
    z-index: 15;
  }

  .welcome h1 {
    margin: 0;
    font-size: var(--text-h2);
    font-weight: 600;
  }

  .welcome p {
    margin: 0;
    color: var(--fg-muted);
  }

  .welcome-actions {
    display: flex;
    gap: var(--space-3);
    margin-block-start: var(--space-2);
  }

  .welcome button,
  .empty-panel button {
    padding: var(--space-2) var(--space-4);
    border: 1px solid var(--accent);
    border-radius: var(--radius-md);
    background: var(--accent);
    color: var(--on-accent);
    font-family: inherit;
    font-size: var(--text-small);
    cursor: pointer;
  }

  .welcome button.secondary {
    background: transparent;
    color: var(--accent);
  }

  .status {
    position: fixed;
    inset-block-end: 0;
    inset-inline-end: 0;
    display: flex;
    gap: var(--space-3);
    padding: var(--space-1) var(--space-3);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
    background: color-mix(in oklab, var(--bg) 88%, transparent);
    border-start-start-radius: var(--radius-md);
    border-block-start: 1px solid var(--border);
    border-inline-start: 1px solid var(--border);
    backdrop-filter: blur(6px);
    pointer-events: none;
  }

  .status-warn {
    color: var(--alert-warning);
  }

  .status-mode {
    color: var(--accent);
    font-weight: 600;
  }

  .notice {
    position: fixed;
    inset-block-end: var(--space-4);
    inset-inline-start: 50%;
    translate: -50% 0;
    max-inline-size: min(48ch, calc(100vw - 2 * var(--space-4)));
    margin: 0;
    padding: var(--space-2) var(--space-4);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg);
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 2px 12px oklch(0% 0 0 / 22%);
    z-index: 40;
  }

  .notice.bad {
    color: var(--alert-caution);
    border-color: var(--alert-caution);
  }
</style>

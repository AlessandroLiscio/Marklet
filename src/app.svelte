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
  import TabBar from './lib/chrome/tabbar.svelte';
  import {
    currentDocument,
    relOfAssetHref,
    revealLine,
    scrollToLine,
    setDocument,
    visibleLine,
  } from './lib/doc';
  import { createEditController, type EditController, type EditMode } from './lib/edit';
  import Outline from './lib/outline.svelte';
  import SettingsPanel from './lib/settings/panel.svelte';
  import Sidebar from './lib/sidebar/sidebar.svelte';
  import {
    activateTab,
    activeTab,
    closeTab,
    cycle,
    EMPTY,
    openTab,
    rememberLine,
    type TabState,
  } from './lib/tabs';
  import {
    exportHtml,
    exportPdf,
    isIpcError,
    indexVault,
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

  /**
   * The open documents.
   *
   * A tab is a record — see `lib/tabs.ts`. Only the active one's markup is in
   * the DOM, and switching re-renders from disk, which is the same path the
   * watcher's live reload already takes on every save.
   *
   * Seeded from whatever the boot script put on screen, so the document opened
   * by double-clicking a file is tab one rather than something the strip
   * learns about later.
   */
  let tabState = $state<TabState>(seedTabs());

  /** The strip at launch: whatever the boot script already put on screen. */
  function seedTabs(): TabState {
    const boot = currentDocument();
    if (boot === null) return EMPTY;
    return { tabs: [{ path: boot.path, rel: boot.rel, title: boot.title, line: 0 }], active: 0 };
  }

  /** Published for `content.css` and the split toggle, which sit under it. */
  $effect(() => {
    document.documentElement.style.setProperty(
      '--tabbar-height',
      tabState.tabs.length > 1 ? '2.25rem' : '0px'
    );
  });
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
  let panel = $state<PanelId | null>(null);

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
  /** How a document was asked for. */
  interface OpenOptions {
    /** A source line to reveal once it is on screen. */
    line?: number;
  }

  /**
   * Opens a document in a tab, or reveals the tab it is already in.
   *
   * The document is read before the tab is recorded, so a file that cannot be
   * opened does not leave a tab behind pointing at it.
   */
  async function openInTab(path: string, options: OpenOptions = {}): Promise<void> {
    let opened: OpenedDocument;
    try {
      opened = await openDocument(path);
    } catch (error) {
      say(reason(error), true);
      return;
    }

    const line = options.line ?? 0;
    tabState = openTab(rememberLine(tabState, doc === null ? 0 : visibleLine()), {
      path: opened.path,
      rel: opened.rel,
      title: opened.title,
      line,
    });

    await show(opened);
    revealAfterPaint(line);
  }

  /** Shows the tab at `index`, remembering where the reader was in this one. */
  async function switchTo(index: number): Promise<void> {
    if (index === tabState.active) return;
    const next = tabState.tabs[index];
    if (next === undefined) return;

    tabState = activateTab(rememberLine(tabState, visibleLine()), index);
    try {
      await show(await openDocument(next.path));
    } catch (error) {
      // The file moved or was deleted while the tab was open. Close it rather
      // than leave a tab that cannot be shown.
      say(reason(error), true);
      tabState = closeTab(tabState, index);
      void showActive();
      return;
    }
    revealAfterPaint(next.line);
  }

  /** Closes a tab, and shows whatever takes over. */
  async function closeAt(index: number): Promise<void> {
    const wasActive = index === tabState.active;
    tabState = closeTab(tabState, index);
    if (wasActive) await showActive();
  }

  /** Renders whatever the strip now says is active, or clears the view. */
  async function showActive(): Promise<void> {
    const active = activeTab(tabState);
    if (active === null) {
      const root = document.getElementById('doc');
      if (root) root.innerHTML = '';
      doc = null;
      document.title = 'Marklet';
      await edit?.setMode('read');
      return;
    }
    try {
      await show(await openDocument(active.path));
      revealAfterPaint(active.line);
    } catch (error) {
      say(reason(error), true);
    }
  }

  /**
   * Restores a reading position once the new document has settled.
   *
   * Two frames, not one: the first commits the markup, the second lets layout
   * settle so the anchor positions being read are the ones that will be on
   * screen. The same reasoning as the reflow restore in `settings/model.ts`.
   */
  function revealAfterPaint(line: number): void {
    if (line <= 0) return;
    requestAnimationFrame(() => requestAnimationFrame(() => revealLine(line)));
  }

  async function show(next: OpenedDocument) {
    const root = document.getElementById('doc');
    if (!root) return;

    // A different file while an editor is open: the session was constructed
    // with the old file's text and splices into the old file's path, so
    // leaving it in place would edit the document nobody is looking at. The
    // round trip through `read` flushes what is pending and re-reads the new
    // source; same file — a save, a watcher event — rebuilds nothing.
    const switched = doc !== null && doc.path !== next.path;
    const resume = switched && mode !== 'read' ? mode : null;
    if (resume) await edit?.setMode('read');

    setDocument(root, next);
    doc = next;
    document.title = `${next.title} — Marklet`;

    const { enrich } = await import('./lib/rich');
    await enrich(root);

    if (resume) await edit?.setMode(resume);

    // Not awaited: it may walk the whole folder, and the document is already
    // on screen. It re-renders when it has something better to say.
    void ensureWikiLinks();
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

    if (target.classList.contains('unresolved')) {
      // An unresolved wiki-link points at a note nobody has written yet. Doing
      // nothing is the honest response; navigating to a 404 is not.
      event.preventDefault();
      return;
    }

    // The **href**, not `data-target`. `data-target` is the name as the author
    // wrote it — `[[demo]]` — and `open_note` canonicalizes what it is given
    // against the vault root, so a name without an extension resolved to
    // nothing and every wiki-link click failed. The renderer already did the
    // resolution and put the answer in the href.
    const note = relOfAssetHref(target.getAttribute('href') ?? '');
    if (note === null) {
      event.preventDefault();
      return;
    }

    event.preventDefault();
    // Opens in its own tab, like every other way of reaching a note.
    // `openFromSidebar` says what went wrong out loud rather than swallowing
    // it: silence here is what hid `open_note` being called with the wrong
    // argument name for five months.
    await openFromSidebar(note);
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

  /**
   * Opens a note the sidebar asked for, and points at what was asked about.
   *
   * A search hit carries the line it was found on, and the block holding that
   * line is highlighted for a moment on arrival — the whole passage, which is
   * what the row in the search panel showed. Marking only the search term
   * inside it lit three characters in the middle of a paragraph and left the
   * reader to work out what they belonged to.
   *
   * Two frames before revealing, not one: the first commits the new document,
   * the second lets layout settle so the anchor positions being read are the
   * ones that will be on screen.
   */
  async function openFromSidebar(path: string, options: OpenOptions = {}): Promise<void> {
    let opened: OpenedDocument;
    try {
      opened = (await openNote(path)) as OpenedDocument;
    } catch (error) {
      say(reason(error), true);
      return;
    }

    const line = options.line ?? 0;
    tabState = openTab(rememberLine(tabState, doc === null ? 0 : visibleLine()), {
      path: opened.path,
      rel: opened.rel,
      title: opened.title,
      line,
    });

    await show(opened);
    revealAfterPaint(line);
  }

  /**
   * The vault whose wiki-link index has been built, if any.
   *
   * One build per vault, and only when something has actually asked for it —
   * see {@link ensureWikiLinks}.
   */
  let indexedVault: string | null = null;

  /** True while that build is running, so the sidebar can say so. */
  let indexing = $state(false);

  /**
   * Builds the wiki-link index — but only when a document needs it.
   *
   * It used to run when a folder was *opened*, which meant walking the tree
   * and parsing every note in it before anything had asked a question the
   * index answers. On a folder of 1 976 notes that is seconds of work for a
   * reader who wanted to look at one file, and most files have no `[[link]]`
   * in them at all.
   *
   * So the trigger is the document: if what is on screen contains a wiki-link
   * that did not resolve, and this vault has not been indexed yet, build it
   * and render the document again. A note with no wiki-links never pays, and
   * a vault pays once.
   *
   * `indexedVault` is set *before* the await, so a second document opened
   * while the build is running does not start a second one.
   */
  async function ensureWikiLinks(): Promise<void> {
    const root = document.getElementById('doc');
    if (root === null || vaultPath === null || indexedVault === vaultPath) return;
    if (root.querySelector('a.wikilink.unresolved') === null) return;

    indexedVault = vaultPath;
    indexing = true;
    try {
      await indexVault();
    } catch {
      // An unreadable note, a folder that vanished. Wiki-links stay
      // unresolved, which is what they already were.
      return;
    } finally {
      indexing = false;
    }
    await resolveWikiLinks();
  }

  /**
   * Renders the open document again, now that wiki-links can resolve.
   *
   * A document rendered before the vault index existed has every `[[link]]`
   * in the unresolved state permanently — resolution happens in Rust when the
   * HTML is built, not when the link is clicked. Every document opened at
   * launch is in that state, because `lib.rs` renders it before a vault is
   * opened, and so is any document opened in the seconds before the index
   * finishes. Opening the explorer on a file therefore showed a panel of
   * struck-through links that pointed at notes sitting right beside it.
   *
   * Guarded on there actually being an unresolved link, so a document with
   * none costs nothing and does not flicker. The reading position is carried
   * across by source line, the same way a reflow carries it.
   */
  async function resolveWikiLinks(): Promise<void> {
    const root = document.getElementById('doc');
    if (!doc || !root || root.querySelector('a.wikilink.unresolved') === null) return;

    const line = visibleLine();
    try {
      await show(await openDocument(doc.path));
    } catch {
      // The file moved or became unreadable between the two reads. What is on
      // screen is still the document; only its links stay unresolved.
      return;
    }
    requestAnimationFrame(() => requestAnimationFrame(() => scrollToLine(line)));
  }

  /** The top-right split toggle. `F3` does the same thing. */
  function toggleSplit(): void {
    void edit?.setMode(mode === 'split' ? 'read' : 'split');
  }

  /**
   * Where the divider sits, as a fraction of the space left of the chrome.
   *
   * Published to `:root` rather than held as a style on one element, because
   * `editor.css` derives two lengths from it — the editor's width and the
   * body's start padding — and those two must agree exactly or the divider
   * stops lining up with the preview's edge.
   */
  const SPLIT_MIN = 0.2;
  const SPLIT_MAX = 0.8;
  let splitRatio = $state(0.5);

  function setSplitRatio(next: number): void {
    splitRatio = Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, next));
    document.documentElement.style.setProperty('--split-ratio', String(splitRatio));
  }

  /**
   * Drags the divider.
   *
   * The offset the ratio is measured from is read once, at pointer-down, off
   * the editor's own box — it is the one element that already knows where the
   * chrome ends, whatever `--chrome-inset` currently resolves to.
   */
  function onDividerDown(event: PointerEvent): void {
    const editor = document.getElementById('editor');
    if (!editor) return;
    const inset = editor.getBoundingClientRect().left;
    const span = window.innerWidth - inset;
    if (span <= 0) return;

    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    dragging = true;

    const move = (e: PointerEvent): void => setSplitRatio((e.clientX - inset) / span);
    const up = (): void => {
      dragging = false;
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', up);
  }

  /** Arrow keys move the divider too: a mouse must not be the only way. */
  function onDividerKey(event: KeyboardEvent): void {
    const step = event.shiftKey ? 0.1 : 0.02;
    if (event.key === 'ArrowLeft') setSplitRatio(splitRatio - step);
    else if (event.key === 'ArrowRight') setSplitRatio(splitRatio + step);
    else if (event.key === 'Home' || event.key === 'Enter') setSplitRatio(0.5);
    else return;
    event.preventDefault();
  }

  let dragging = $state(false);

  /** Nothing was opened: ask the OS for a file, or for a folder to browse. */
  async function openFromDialog(kind: 'file' | 'folder'): Promise<void> {
    try {
      const picked = kind === 'file' ? await pickFile() : await pickFolder();
      if (picked === null) return; // cancelled, which is a normal answer
      if (kind === 'folder') {
        vaultPath = picked;
        panel = 'explorer';
      } else {
        await openInTab(picked);
      }
    } catch (error) {
      say(reason(error), true);
    }
  }

  /**
   * The tab shortcuts, checked before the editor sees the keystroke.
   *
   * Before, because `Ctrl+W` inside a CodeMirror session still means "close
   * this document" — the editor has no use for it and a reader who has one
   * open still expects it to work.
   */
  function tabShortcut(event: KeyboardEvent): boolean {
    if (!event.ctrlKey || event.altKey || event.metaKey) return false;

    if (event.key.toLowerCase() === 'w' && !event.shiftKey) {
      event.preventDefault();
      if (tabState.active !== -1) void closeAt(tabState.active);
      return true;
    }
    if (event.key === 'Tab') {
      event.preventDefault();
      const next = cycle(tabState, event.shiftKey ? -1 : 1);
      void switchTo(next.active);
      return true;
    }
    return false;
  }

  function onKeyDown(event: KeyboardEvent): void {
    if (tabShortcut(event)) return;
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
        // The preview is re-rendered from the file the splice just wrote.
        //
        // Split mode is two views of one document, and the right-hand one was
        // static: the watcher in `lib.rs` only ever watches the path the app
        // was *launched* with, so a file reached through the explorer or the
        // picker produced no `file-changed` at all, and even the launched one
        // re-rendered only by luck of ordering. This is the deterministic
        // path — the save has returned, so the bytes are on disk.
        onSaved: () => {
          if (!doc || mode === 'read') return;
          void openDocument(doc.path)
            .then(async (next) => {
              await show(next);
              // New DOM, so the split columns' line map is stale until it is
              // measured again.
              edit?.resync();
            })
            .catch(() => {
              // The editor still holds the text and the file is already
              // written; a stale preview is not worth a dialog over it.
            });
        },
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
          // A second launch forwarded a path here. It becomes a tab, in front,
          // because the user asked for that file just now.
          await openInTab(event.payload);
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
    onpanel={(next) => (next === 'explorer' ? showExplorer() : (panel = next))}
  />

  {#if panel !== null}
    <div class="panel-dock">
      {#if panel === 'explorer'}
        {#if vaultPath}
          <!-- `active` is the VAULT-RELATIVE path, not the absolute one: the
               tree's rows and the backlinks index are both keyed on that
               spelling, and the absolute path matched neither. -->
          <Sidebar
            {vaultPath}
            active={doc?.rel ?? null}
            {indexing}
            onopen={(path, options) => void openFromSidebar(path, options)}
          />
        {:else}
          <div class="empty-panel">
            <p>No folder open.</p>
            <button type="button" onclick={() => void openFromDialog('folder')}>
              Open a folder…
            </button>
          </div>
        {/if}
      {:else}
        <Outline open />
      {/if}
    </div>
  {/if}
</div>

<!-- Floating, bottom-right, and owning its own open state: settings is not a
     view of the document the way the explorer and the outline are. -->
<SettingsPanel />

<TabBar
  tabs={tabState.tabs}
  active={tabState.active}
  onselect={(i) => void switchTo(i)}
  onclose={(i) => void closeAt(i)}
/>

{#if doc}
  <!-- The split toggle sits in the document's own top-right corner, not in
       the activity bar: it changes how the document is shown rather than what
       is beside it, which is where every editor puts its view controls. It
       reads as pressed by fill, not by colour alone — the same rule the
       activity bar follows. -->
  <button
    type="button"
    class="split-toggle"
    class:on={mode === 'split'}
    aria-pressed={mode === 'split'}
    title="Split editor (F3)"
    onclick={toggleSplit}
  >
    <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <rect x="3" y="4" width="14" height="12" rx="1" stroke="currentColor" stroke-width="1.4" />
      <path d="M10 4v12" stroke="currentColor" stroke-width="1.4" />
    </svg>
    <span>Split</span>
  </button>
{/if}

{#if mode === 'split'}
  <!-- Drag to re-balance the two columns; double-click, Home or Enter to put
       it back in the middle. A `separator` that takes focus is the ARIA role
       for exactly this, and arrow keys move it, because a window split is not
       something only a mouse should be able to do. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- Both silenced deliberately: ARIA 1.2 makes `separator` a *widget* role
       once it is focusable, with `aria-valuenow` and arrow keys, which is
       exactly what this is. Svelte's rule classifies the role by its
       non-focusable form. -->
  <div
    class="divider"
    class:dragging
    role="separator"
    aria-orientation="vertical"
    aria-label="Resize the editor"
    aria-valuemin={Math.round(SPLIT_MIN * 100)}
    aria-valuemax={Math.round(SPLIT_MAX * 100)}
    aria-valuenow={Math.round(splitRatio * 100)}
    tabindex="0"
    onpointerdown={onDividerDown}
    onkeydown={onDividerKey}
    ondblclick={() => setSplitRatio(0.5)}
  ></div>
{/if}

{#if !doc && vaultPath === null}
  <!-- Launched with nothing, and still pointed at nothing. An empty window
       with no explanation is the worst version of that; the OS file picker is
       the one people already know.

       It goes as soon as a folder is chosen, before any note is opened: the
       explorer that appears beside it *is* the answer to "open a Markdown
       file, or a folder to browse", and leaving the invitation standing next
       to it reads as though the choice had not registered. -->
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

  .split-toggle {
    position: fixed;
    /* Under the tab strip when there is one, so the two never overlap. */
    inset-block-start: calc(var(--space-3) + var(--tabbar-height, 0px));
    inset-inline-end: var(--space-3);
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
    background: color-mix(in oklab, var(--bg) 88%, transparent);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    backdrop-filter: blur(6px);
    cursor: pointer;
    z-index: 30;
  }

  .split-toggle:hover {
    color: var(--fg);
    border-color: var(--fg-muted);
  }

  .split-toggle.on {
    color: var(--on-accent);
    background: var(--accent);
    border-color: var(--accent);
  }

  .split-toggle:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* Centred on the boundary `editor.css` computed, so the grab area straddles
     the seam rather than sitting beside it. */
  .divider {
    position: fixed;
    inset-block-start: var(--tabbar-height, 0px);
    inset-block-end: 0;
    inset-inline-start: var(--split-left, 50vw);
    inline-size: 9px;
    translate: -50% 0;
    cursor: col-resize;
    /* Without this a touch drag scrolls the page instead of moving the
       divider, and the pointer events stop arriving mid-gesture. */
    touch-action: none;
    z-index: 25;
  }

  .divider::after {
    content: '';
    position: absolute;
    inset-block: 0;
    inset-inline-start: 50%;
    inline-size: 2px;
    translate: -50% 0;
    background: transparent;
    transition: background var(--duration-fast) ease;
  }

  .divider:hover::after,
  .divider.dragging::after {
    background: var(--accent);
  }

  .divider:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
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

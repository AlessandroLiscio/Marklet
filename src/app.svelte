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

  import { currentDocument, setDocument } from './lib/doc';
  import Outline from './lib/outline.svelte';
  import SettingsPanel from './lib/settings/panel.svelte';
  import Sidebar from './lib/sidebar/sidebar.svelte';
  import { openDocument, openNote, type OpenedDocument } from './lib/ipc';

  let doc = $state<OpenedDocument | null>(currentDocument());
  const vaultPath = typeof window !== 'undefined' ? (window.__MARKLET_VAULT__ ?? null) : null;

  let unlisten: Array<() => void> = [];

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
    const target = (event.target as HTMLElement | null)?.closest<HTMLAnchorElement>('a.wikilink');
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

  onMount(() => {
    const root = document.getElementById('doc');
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
          if (doc) await show(await openDocument(doc.path));
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
    };
  });

  onDestroy(() => {
    for (const off of unlisten) off();
    unlisten = [];
  });
</script>

<Outline />
<SettingsPanel />
{#if vaultPath}
  <Sidebar {vaultPath} active={doc?.path ?? null} onopen={(path) => void openNote(path).then((d) => show(d as OpenedDocument))} />
{/if}

{#if doc}
  <footer class="status" aria-label="Document status">
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
</style>

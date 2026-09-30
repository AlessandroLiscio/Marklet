<script lang="ts">
  /**
   * The vault sidebar: folder tree, search, backlinks.
   *
   * Owns the scan stream and nothing else. Entries arrive as
   * `vault-scan-progress` batches and are appended to a flat array — the tree
   * below virtualizes it, so a 5 000-note vault paints its first rows while the
   * walk is still running and never becomes 5 000 components.
   *
   * The document itself is not in this tree, and must not be: Svelte owns the
   * chrome, and a 3 MB markdown file goes into a plain `<article>` through
   * `innerHTML`. `onopen` is how this panel asks for a note, not how it renders
   * one.
   */
  import Links from './links.svelte';
  import SearchPanel from './search.svelte';
  import Tree from './tree.svelte';
  import { listDir, openVault, type Entry, type VaultInfo } from '../ipc';
  import { spliceChildren } from './tree';

  interface Props {
    /** The folder to open. Changing it opens a different vault. */
    vaultPath?: string | null;
    /** The open note, vault-relative — highlighted, and its backlinks shown. */
    active?: string | null;
    /** Opens a note in its own tab. `line` scrolls to it and highlights it. */
    onopen?: (path: string, options?: { line?: number }) => void;
    /**
     * The wiki-link index is being built, by the app shell.
     *
     * Shown here because this is where the folder is, and an application that
     * looks idle while it is working is one the reader assumes is broken.
     */
    indexing?: boolean;
    /**
     * `open_vault` has returned, so Rust knows about this folder.
     *
     * The app shell re-reads the open document on this, because a document
     * rendered before a vault existed has no vault-relative path and that is
     * what the tree's highlight and the Links panel are keyed on.
     */
    onvaultopen?: () => void;
    /**
     * Asks for the OS picker again.
     *
     * Choosing a folder used to be a one-way door: the welcome screen offered
     * it, and once anything was open the offer was gone — changing your mind
     * meant restarting the application.
     */
    onpick?: (kind: 'file' | 'folder') => void;
  }

  let {
    vaultPath = null,
    active = null,
    onopen,
    indexing = false,
    onvaultopen,
    onpick,
  }: Props = $props();

  let info = $state<VaultInfo | null>(null);
  let entries = $state<Entry[]>([]);
  let scanning = $state(false);
  let error = $state<string | null>(null);
  let tab = $state<'files' | 'search'>('files');

  /**
   * Folders whose children have been fetched.
   *
   * The tree needs this to know whether a folder with no rows under it is
   * empty or merely unopened — see `visibleRows`. It is also what stops a
   * second expand from re-listing a folder that has not changed.
   */
  let listed = $state<Set<string>>(new Set());

  $effect(() => {
    const path = vaultPath;
    if (path === null) {
      info = null;
      entries = [];
      return;
    }

    let current = true;
    entries = [];
    listed = new Set();
    error = null;
    scanning = true;

    // One level, not a walk. Opening a note roots this at the note's folder,
    // which may be a home directory; listing the children shows the same rows
    // a walk would show first, without reading everything underneath them.
    // The walk still happens — on the Search tab, which cannot work without it.
    void (async () => {
      try {
        const opened = await openVault(path);
        if (!current) return;
        info = opened;
        onvaultopen?.();
        const top = await listDir('', 0);
        if (!current) return;
        entries = top;
        listed = new Set(['']);

      } catch (e) {
        if (!current) return;
        error = e instanceof Object && 'message' in e ? String(e.message) : 'Could not open that folder';
        info = null;
      } finally {
        if (current) scanning = false;
      }
    })();

    return () => {
      current = false;
    };
  });
  /** Fetches a folder's children the first time it is opened. */
  async function expand(path: string, depth: number): Promise<void> {
    if (listed.has(path)) return;
    try {
      const children = await listDir(path, depth + 1);
      entries = spliceChildren(entries, path, children);
      listed = new Set(listed).add(path);
    } catch {
      // An unreadable folder stays expandable and shows nothing. Refusing to
      // open the rest of the tree over one permission error would be worse.
    }
  }

</script>

<aside class="sidebar" aria-label="Vault">
  <header>
    <span class="name" title={info?.root ?? ''}>{info?.name ?? 'No vault'}</span>
    <!-- Not a row count: the tree is listed lazily and its size is not what is
         being waited for. What is worth saying is that something is still
         running. -->
    {#if scanning || indexing}
      <span class="counting" title={indexing ? 'Building the wiki-link index' : 'Opening the folder'}>
        &hellip;
      </span>
    {/if}

    <!-- Inline SVG per the icon rule: no icon font, no icon package. Beside
         the folder's name because that is where "which folder am I in" is
         answered, so it is where "a different one" should be asked. -->
    <span class="pick">
      <button type="button" title="Open a file… (Ctrl+O)" aria-label="Open a file" onclick={() => onpick?.('file')}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path
            d="M4 1.75h5L12.25 5v9.25H4z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.3"
            stroke-linejoin="round"
          />
          <path d="M8.75 2v3.25H12" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" />
        </svg>
      </button>
      <button
        type="button"
        title="Open a different folder… (Ctrl+Shift+O)"
        aria-label="Open a different folder"
        onclick={() => onpick?.('folder')}
      >
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path
            d="M1.75 4a.75.75 0 0 1 .75-.75h3l1.1 1.5h7.65a.75.75 0 0 1 .75.75v7a.75.75 0 0 1-.75.75H2.5a.75.75 0 0 1-.75-.75z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.3"
            stroke-linejoin="round"
          />
        </svg>
      </button>
    </span>
  </header>

  <nav class="tabs">
    <button type="button" class:on={tab === 'files'} onclick={() => (tab = 'files')}>Files</button>
    <!-- No walk here any more. Opening this tab used to run `scan_vault` over
         the whole tree first, and nothing consumed it: `vault::search::search`
         takes the root and walks it itself, per query. It was 1.4 s of I/O on
         a large folder, thrown away. -->
    <button type="button" class:on={tab === 'search'} onclick={() => (tab = 'search')}>
      Search
    </button>
  </nav>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if tab === 'files'}
    <Tree
      {entries}
      {active}
      {listed}
      onexpand={(path, depth) => void expand(path, depth)}
      onopen={(path) => onopen?.(path)}
    />
  {:else}
    <SearchPanel
      enabled={info !== null}
      onopen={(path, line) => onopen?.(path, { line })}
    />
  {/if}

  <!-- Not keyed on anything: the panel listens for the document being replaced,
       which is the event that actually changes its answer. Remounting it when
       the index finished was the wrong signal — the re-render it triggers has
       not happened yet at that moment, so the fresh component read the same
       stale markup. -->
  <Links path={active} onopen={(path) => onopen?.(path)} />
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    block-size: 100%;
    inline-size: 260px;
    background: var(--bg-subtle);
    border-inline-end: 1px solid var(--border);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    font-weight: 600;
    color: var(--fg);
  }

  /* Shrinks so the two buttons beside it always fit; `min-inline-size: 0` is
     what lets a flex item be narrower than its text. */
  .name {
    flex: 0 1 auto;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .counting {
    font-weight: 400;
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .tabs {
    display: flex;
    border-block-end: 1px solid var(--border);
  }

  .tabs button {
    flex: 1;
    padding: var(--space-1);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
    background: none;
    border: 0;
    border-block-end: 2px solid transparent;
    cursor: default;
  }

  .tabs button.on {
    color: var(--fg);
    border-block-end-color: var(--accent);
  }

  .tabs button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .error {
    margin: 0;
    padding: var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--alert-warning);
  }
</style>

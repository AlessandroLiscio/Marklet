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
  import {
    indexVault,
    listDir,
    onScanDone,
    openVault,
    scanVault,
    subscribeAll,
    type Entry,
    type VaultInfo,
  } from '../ipc';
  import { spliceChildren } from './tree';

  interface Props {
    /** The folder to open. Changing it opens a different vault. */
    vaultPath?: string | null;
    /** The open note, vault-relative — highlighted, and its backlinks shown. */
    active?: string | null;
    /** Opens a note. `line` scrolls to it and highlights the block it is in. */
    onopen?: (path: string, line?: number) => void;
    /**
     * The wiki-link index has been built.
     *
     * The open document may have been rendered before there was one — every
     * document opened at launch was, because `lib.rs` renders it before a
     * vault exists — and a `[[link]]` rendered without an index is unresolved
     * for good unless the document is rendered again. The app shell is what
     * can do that, so it is told.
     */
    onindexed?: () => void;
  }

  let { vaultPath = null, active = null, onopen, onindexed }: Props = $props();

  let info = $state<VaultInfo | null>(null);
  let entries = $state<Entry[]>([]);
  let scanning = $state(false);
  /**
   * The wiki-link index is being built.
   *
   * Shown, because on a large folder it is seconds of work and an application
   * that looks idle while it is busy is one the reader assumes is broken.
   */
  let indexing = $state(false);
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

  /**
   * The full walk has run, so the Rust-side index Search queries is built.
   *
   * It says nothing about this component's `entries`, and must not: the walk
   * used to be streamed into that array, which wiped the lazily-listed tree
   * the moment the Search tab was opened and left the Files tab showing the
   * walk's own flat output when the user came back to it. The tree is lazy
   * now; the walk exists for the index and for nothing on this side.
   */
  let walked = $state(false);

  // One subscription for the component's life. `vault-scan-progress` batches
  // are deliberately NOT consumed — see `walked`. What is still worth knowing
  // is when the walk stops, because the header says it is running.
  $effect(() => subscribeAll([onScanDone(() => (scanning = false))]));

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
    walked = false;
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
        const top = await listDir('', 0);
        if (!current) return;
        entries = top;
        listed = new Set(['']);

        // The wiki-link index, in the background.
        //
        // Not awaited — the tree is already on screen and this walks the whole
        // folder and parses every note in it. On the Rust side it now runs off
        // the main thread as well, which is what stopped the window freezing
        // while it ran; here it only has to not block the rows that are
        // already painted.
        indexing = true;
        void indexVault()
          .then(() => {
            if (current) onindexed?.();
          })
          .catch(() => {
            // An unreadable note, a folder that vanished. Wiki-links stay
            // unresolved, which is what they were before this ran.
          })
          .finally(() => {
            if (current) indexing = false;
          });
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

  /**
   * Runs the full walk, once, when the Search tab is first opened.
   *
   * Deferred to here rather than done on open because this is the only thing
   * that needs it, and it is the expensive thing — see `vault::scan::list_dir`.
   */
  async function ensureWalked(): Promise<void> {
    if (walked || info === null) return;
    scanning = true;
    try {
      await scanVault();
      walked = true;
    } catch {
      // Search reports its own failure; the tree keeps the rows it has.
    } finally {
      scanning = false;
    }
  }
</script>

<aside class="sidebar" aria-label="Vault">
  <header>
    <span class="name" title={info?.root ?? ''}>{info?.name ?? 'No vault'}</span>
    <!-- Not a row count: the tree is listed lazily and its size is not what is
         being waited for. What is worth saying is that something is still
         running, and which of the two things it is. -->
    {#if scanning || indexing}
      <span class="counting" title={scanning ? 'Walking the folder for search' : 'Building the wiki-link index'}>
        &hellip;
      </span>
    {/if}
  </header>

  <nav class="tabs">
    <button type="button" class:on={tab === 'files'} onclick={() => (tab = 'files')}>Files</button>
    <button
      type="button"
      class:on={tab === 'search'}
      onclick={() => {
        tab = 'search';
        void ensureWalked();
      }}
    >
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
      onopen={(path, line) => onopen?.(path, line)}
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
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    font-weight: 600;
    color: var(--fg);
  }

  .name {
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

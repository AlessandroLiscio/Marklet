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
  import Backlinks from './backlinks.svelte';
  import SearchPanel from './search.svelte';
  import Tree from './tree.svelte';
  import {
    onScanDone,
    onScanProgress,
    openVault,
    scanVault,
    subscribeAll,
    type Entry,
    type VaultInfo,
  } from '../ipc';

  interface Props {
    /** The folder to open. Changing it opens a different vault. */
    vaultPath?: string | null;
    /** The open note, vault-relative — highlighted, and its backlinks shown. */
    active?: string | null;
    onopen?: (path: string, line?: number) => void;
  }

  let { vaultPath = null, active = null, onopen }: Props = $props();

  let info = $state<VaultInfo | null>(null);
  let entries = $state<Entry[]>([]);
  let scanning = $state(false);
  let error = $state<string | null>(null);
  let tab = $state<'files' | 'search'>('files');

  // One subscription for the component's life. Batches that arrive between
  // `openVault` and `scanVault` belong to this vault too, so the listener is
  // installed before either runs rather than per scan.
  $effect(() =>
    subscribeAll([
      onScanProgress(({ entries: batch }) => {
        // Append, never re-sort: the walk yields parents before children and
        // siblings in name order, which is exactly the order the tree wants.
        entries = entries.concat(batch);
      }),
      onScanDone(() => {
        scanning = false;
      }),
    ])
  );

  $effect(() => {
    const path = vaultPath;
    if (path === null) {
      info = null;
      entries = [];
      return;
    }

    let current = true;
    entries = [];
    error = null;
    scanning = true;

    void (async () => {
      try {
        const opened = await openVault(path);
        if (!current) return;
        info = opened;
        await scanVault();
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
</script>

<aside class="sidebar" aria-label="Vault">
  <header>
    <span class="name" title={info?.root ?? ''}>{info?.name ?? 'No vault'}</span>
    {#if scanning}<span class="counting">{entries.length}</span>{/if}
  </header>

  <nav class="tabs">
    <button type="button" class:on={tab === 'files'} onclick={() => (tab = 'files')}>Files</button>
    <button type="button" class:on={tab === 'search'} onclick={() => (tab = 'search')}>
      Search
    </button>
  </nav>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if tab === 'files'}
    <Tree {entries} {active} onopen={(path) => onopen?.(path)} />
  {:else}
    <SearchPanel
      enabled={info !== null}
      onopen={(path, line) => onopen?.(path, line)}
    />
  {/if}

  <Backlinks path={active} onopen={(path, line) => onopen?.(path, line)} />
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

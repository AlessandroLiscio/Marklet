<script lang="ts">
  /**
   * The folder tree — **virtualized**.
   *
   * Only the rows on screen exist in the DOM; two spacer divs hold the scroll
   * height for the rest. A 5 000-note vault is therefore about forty elements,
   * and expanding a folder re-slices an array rather than mounting a subtree.
   *
   * The row height is a constant shared with `tree.ts` and applied inline,
   * because the scroll maths and the layout must agree exactly — a stylesheet
   * that changes one and not the other produces a tree that drifts as you
   * scroll, which is the hardest kind of bug here to see.
   */
  import {
    ancestorsOf,
    displayName,
    ROW_HEIGHT,
    toggle,
    visibleRows,
    windowOf,
    type Row,
  } from './tree';
  import type { Entry } from '../ipc';

  interface Props {
    entries: readonly Entry[];
    /** The open note, vault-relative. Highlighted, and revealed when it changes. */
    active?: string | null;
    /**
     * Folders whose children have been fetched, or `null` when every folder
     * has — see `visibleRows`. Lazily, a folder with no rows under it is
     * unopened rather than empty, and must keep its disclosure arrow.
     */
    listed?: ReadonlySet<string> | null;
    /** Asked before a folder is expanded for the first time. */
    onexpand?: (path: string, depth: number) => void;
    /** Opens a note in its own tab. */
    onopen?: (path: string) => void;
  }

  let { entries, active = null, listed = null, onexpand, onopen }: Props = $props();

  let expanded = $state(new Set<string>());
  /** The row a single click highlighted. Not the open document — that is `active`. */
  let selected = $state<string | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(480);

  const rows = $derived(visibleRows(entries, expanded, listed));
  const slice = $derived(windowOf(rows.length, scrollTop, viewport));

  // Revealing the open note is a state change, not a render: expanding its
  // ancestors is what makes "open a note from search" leave the tree pointing
  // at where that note lives.
  $effect(() => {
    if (active === null) return;
    const missing = ancestorsOf(active).filter((dir) => !expanded.has(dir));
    if (missing.length === 0) return;
    const next = new Set(expanded);
    for (const dir of missing) next.add(dir);
    expanded = next;
  });

  function onScroll(event: Event): void {
    const el = event.currentTarget;
    if (el instanceof HTMLElement) scrollTop = el.scrollTop;
  }

  function measure(el: HTMLDivElement): () => void {
    viewport = el.clientHeight;
    const observer = new ResizeObserver(() => {
      viewport = el.clientHeight;
    });
    observer.observe(el);
    return () => observer.disconnect();
  }

  /**
   * One click: select. A folder also opens or closes — that is what a
   * disclosure triangle means everywhere — but a file is only highlighted.
   *
   * Opening a file takes a **double** click, asked for directly and the
   * convention every file explorer follows. A single click that swapped the
   * document made arrowing through a folder re-render it once per row.
   */
  function select(row: Row): void {
    selected = row.path;
    if (!row.dir) return;
    // Asked on every expand; the sidebar ignores a folder it has already
    // listed. Doing that check here would need this component to know what
    // has been fetched, which is the sidebar's business, not the tree's.
    if (!expanded.has(row.path)) onexpand?.(row.path, row.depth);
    expanded = toggle(expanded, row.path);
  }

  /** Two clicks: open, if it is something Marklet can show. */
  function open(row: Row): void {
    if (row.openable) onopen?.(row.path);
  }

  function onKey(event: KeyboardEvent, row: Row): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      // Enter is the keyboard's double-click: it opens. A folder has no
      // "open", so it toggles, which is what Enter does in every tree.
      if (row.dir) select(row);
      else open(row);
      return;
    }
    if (event.key === ' ') {
      event.preventDefault();
      select(row);
    }
  }
</script>

<div
  class="scroller"
  onscroll={onScroll}
  {@attach measure}
  role="tree"
  aria-label="Vault"
  tabindex="-1"
>
  {#if rows.length === 0}
    <p class="empty">No notes yet.</p>
  {:else}
    <div style:height="{slice.padTop}px" aria-hidden="true"></div>
    {#each rows.slice(slice.start, slice.end) as row (row.path)}
      <div
        class="row"
        class:dir={row.dir}
        class:other={!row.dir && !row.openable}
        class:active={row.path === active}
        class:selected={row.path === selected && row.path !== active}
        style:height="{ROW_HEIGHT}px"
        style:padding-inline-start="{4 + row.depth * 14}px"
        role="treeitem"
        aria-selected={row.path === active || row.path === selected}
        aria-expanded={row.expandable ? row.expanded : undefined}
        tabindex="0"
        onclick={() => select(row)}
        ondblclick={() => open(row)}
        onkeydown={(e) => onKey(e, row)}
      >
        <span class="twist" aria-hidden="true">
          {#if row.expandable}
            <!-- Inline SVG, never an icon font: this is about render cost and
                 flash-of-unstyled-icon as much as bytes. -->
            <svg viewBox="0 0 12 12" width="10" height="10" class:open={row.expanded}>
              <path d="M4 2.5 L8 6 L4 9.5" fill="none" stroke="currentColor" stroke-width="1.5" />
            </svg>
          {/if}
        </span>
        <span class="name">{displayName(row.name, row.dir)}</span>
      </div>
    {/each}
    <div style:height="{slice.padBottom}px" aria-hidden="true"></div>
  {/if}
</div>

<style>
  .scroller {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    contain: strict;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding-inline-end: var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }

  .row:hover {
    background: var(--bg-subtle);
  }

  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .row.dir {
    color: var(--fg-muted);
  }

  .row.active {
    background: var(--accent-muted);
    color: var(--fg);
  }

  .row.selected {
    background: color-mix(in oklab, var(--fg) 10%, transparent);
  }

  /* Listed, not offered. A `.png` belongs in the folder it is in — hiding it
     makes the folder look wrong — but double-clicking it does nothing, and
     that has to be visible before the click, not after. */
  .row.other {
    color: var(--fg-muted);
    opacity: 0.7;
  }

  .twist {
    display: inline-flex;
    inline-size: 12px;
    justify-content: center;
    color: var(--fg-muted);
  }

  .twist svg {
    transition: transform var(--duration-fast) ease;
  }

  .twist svg.open {
    transform: rotate(90deg);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .empty {
    padding: var(--space-3);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }
</style>

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
  } from './tree';
  import type { Entry } from '../ipc';

  interface Props {
    entries: readonly Entry[];
    /** The open note, vault-relative. Highlighted, and revealed when it changes. */
    active?: string | null;
    onopen?: (path: string) => void;
  }

  let { entries, active = null, onopen }: Props = $props();

  let expanded = $state(new Set<string>());
  let scrollTop = $state(0);
  let viewport = $state(480);

  const rows = $derived(visibleRows(entries, expanded));
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

  function activate(path: string, dir: boolean): void {
    if (dir) expanded = toggle(expanded, path);
    else onopen?.(path);
  }

  function onKey(event: KeyboardEvent, path: string, dir: boolean): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      activate(path, dir);
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
        class:active={row.path === active}
        style:height="{ROW_HEIGHT}px"
        style:padding-inline-start="{4 + row.depth * 14}px"
        role="treeitem"
        aria-selected={row.path === active}
        aria-expanded={row.expandable ? row.expanded : undefined}
        tabindex="0"
        onclick={() => activate(row.path, row.dir)}
        onkeydown={(e) => onKey(e, row.path, row.dir)}
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

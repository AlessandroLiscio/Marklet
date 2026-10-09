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
  import { tick } from 'svelte';
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
  /**
   * The row that holds the tree's one tab stop (a roving tabindex). Whatever
   * row last took focus; the first row until anything has.
   */
  let cursor = $state<string | null>(null);
  let scroller = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(480);

  const rows = $derived(visibleRows(entries, expanded, listed));
  const slice = $derived(windowOf(rows.length, scrollTop, viewport));
  /**
   * The path that carries `tabindex="0"`. If the cursor row is gone or has been
   * scrolled out of the rendered window, the first rendered row stands in, so
   * Tab can always find the tree.
   */
  const tabStop = $derived.by(() => {
    const visible = rows.slice(slice.start, slice.end);
    const own = visible.find((r) => r.path === cursor);
    return (own ?? visible[0])?.path ?? null;
  });

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
   * document would re-render the whole page every time the selection moved,
   * which is what selecting a row down a long folder does.
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

  /**
   * Moves the highlight and the keyboard focus to `rows[index]`, scrolling it
   * into the window first — a virtualized row that is not rendered cannot take
   * focus. **Selects, never opens**: the same rule a single click follows, or
   * holding `↓` would re-render the page once per row.
   */
  async function goto(index: number): Promise<void> {
    const target = rows[index];
    if (!target || !scroller) return;
    selected = target.path;
    cursor = target.path;
    const top = index * ROW_HEIGHT;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW_HEIGHT > scroller.scrollTop + viewport) {
      scroller.scrollTop = top + ROW_HEIGHT - viewport;
    }
    scrollTop = scroller.scrollTop;
    await tick();
    for (const el of scroller.querySelectorAll<HTMLElement>('.row')) {
      if (el.dataset.path === target.path) {
        el.focus();
        break;
      }
    }
  }

  /** The tree keys: `↑ ↓ → ← Home End`. Returns whether the key was one. */
  function navigate(event: KeyboardEvent, row: Row): boolean {
    const at = rows.findIndex((r) => r.path === row.path);
    if (at === -1) return false;
    switch (event.key) {
      case 'ArrowDown':
        void goto(Math.min(rows.length - 1, at + 1));
        return true;
      case 'ArrowUp':
        void goto(Math.max(0, at - 1));
        return true;
      case 'Home':
        void goto(0);
        return true;
      case 'End':
        void goto(rows.length - 1);
        return true;
      case 'ArrowRight':
        if (row.expandable && !row.expanded) select(row);
        else if (row.expanded && (rows[at + 1]?.depth ?? -1) > row.depth) void goto(at + 1);
        return true;
      case 'ArrowLeft': {
        if (row.expanded) {
          select(row);
          return true;
        }
        for (let i = at - 1; i >= 0; i--) {
          if ((rows[i]?.depth ?? row.depth) < row.depth) {
            void goto(i);
            break;
          }
        }
        return true;
      }
      default:
        return false;
    }
  }

  function onKey(event: KeyboardEvent, row: Row): void {
    if (!event.ctrlKey && !event.altKey && !event.metaKey && navigate(event, row)) {
      // Without this the arrow keys also scroll the panel under the focus.
      event.preventDefault();
      return;
    }
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
  bind:this={scroller}
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
        tabindex={row.path === tabStop ? 0 : -1}
        data-path={row.path}
        onfocus={() => (cursor = row.path)}
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

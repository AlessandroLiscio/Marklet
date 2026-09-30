<script lang="ts">
  /**
   * The left edge: panels at the top, actions at the bottom.
   *
   * Why a bar rather than more floating buttons. Before this there were two
   * controls in two corners, each owning its own open state, and adding a
   * third and a fourth would have meant four corners and no answer for the
   * fifth. A bar is one place to look, one place to add to, and the shape
   * every editor has trained people on.
   *
   * The top group is one toggle per panel — places to look. The bottom group
   * is things to *do*: open something, export this. Each opens a two-row menu
   * rather than acting outright, because each is really two commands. No
   * native dialog picks a file or a folder in one call (Windows' folder mode
   * is folder-only), and an export needs a format before it can start.
   *
   * It holds no state about which panel is open. That belongs to `app.svelte`,
   * because it is read by things outside this component — the document's
   * inset, the edit controller — and a toggle that owns the truth about a
   * layout it does not lay out is how the two drift apart. Which *menu* is
   * open is different: nothing outside can see it or needs to, so it is local.
   */
  export type PanelId = 'explorer' | 'outline';

  type MenuId = 'pick' | 'export';

  interface Row {
    label: string;
    run: () => void;
  }

  interface Props {
    /** The open panel, or `null` when the document has the full width. */
    panel: PanelId | null;
    onpanel: (panel: PanelId | null) => void;
    /** Native dialog for a file or a folder — two kinds because no dialog does both. */
    onpick: (kind: 'file' | 'folder') => void;
    onexport: (format: 'pdf' | 'html') => void;
    /** False with no document open. The button stays, dimmed, so the rail does not jump. */
    canExport: boolean;
  }

  let { panel, onpanel, onpick, onexport, canExport }: Props = $props();

  /** Clicking the open panel closes it — the same affordance as VS Code. */
  function choose(id: PanelId): void {
    onpanel(panel === id ? null : id);
  }

  /** One value for both menus, so two can never be open at once. */
  let open = $state<MenuId | null>(null);

  /** The wrapper, not the button: "outside" has to mean outside the menu too. */
  const slots: Record<MenuId, HTMLElement | undefined> = { pick: undefined, export: undefined };

  const PICK_ROWS: readonly Row[] = [
    { label: 'File…', run: () => onpick('file') },
    { label: 'Folder…', run: () => onpick('folder') },
  ];

  const EXPORT_ROWS: readonly Row[] = [
    { label: 'PDF', run: () => onexport('pdf') },
    { label: 'HTML', run: () => onexport('html') },
  ];

  function toggle(id: MenuId): void {
    open = open === id ? null : id;
  }

  /** Focus goes back to the opener for Escape and for a chosen row — the row
   *  is about to unmount, and focus on a removed node falls to `<body>`. A
   *  click elsewhere keeps its own focus. */
  function close(restoreFocus: boolean): void {
    const id = open;
    open = null;
    if (restoreFocus && id !== null) slots[id]?.querySelector<HTMLElement>('.item')?.focus();
  }

  function activate(row: Row): void {
    close(true);
    row.run();
  }

  function onOutside(event: PointerEvent): void {
    if (open === null) return;
    const inside = event.target instanceof Node && slots[open]?.contains(event.target);
    if (!inside) close(false);
  }

  // Capture plus `stopPropagation`: `app.svelte` turns Escape into "leave edit
  // mode" on a bubbling window listener, and one keypress must not dismiss a
  // menu and also throw the user out of the editor behind it.
  function onKey(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.stopPropagation();
    close(true);
  }

  // Registered only while a menu is open: a rail that listens to every click in
  // the window all session for the sake of a menu that is usually shut is a
  // listener paying rent for nothing.
  $effect(() => {
    if (open === null) return;
    window.addEventListener('pointerdown', onOutside);
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('pointerdown', onOutside);
      window.removeEventListener('keydown', onKey, true);
    };
  });

  // The document can close underneath an open export menu; offering PDF for
  // nothing would be a menu whose rows all fail.
  $effect(() => {
    if (!canExport && open === 'export') open = null;
  });
</script>

{#snippet menu(label: string, rows: readonly Row[])}
  <div class="menu" role="menu" aria-label={label}>
    {#each rows as row (row.label)}
      <button type="button" role="menuitem" class="row" onclick={() => activate(row)}>
        {row.label}
      </button>
    {/each}
  </div>
{/snippet}

<nav class="bar" aria-label="Views">
  <button
    type="button"
    class="item"
    class:active={panel === 'explorer'}
    aria-pressed={panel === 'explorer'}
    title="Explorer"
    aria-label="Explorer"
    onclick={() => choose('explorer')}
  >
    <!-- Inline SVG per the icon rule: no icon font, no icon package. -->
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <path
        d="M2.5 5.5a1 1 0 0 1 1-1h3.4l1.4 1.8h7.2a1 1 0 0 1 1 1v7.2a1 1 0 0 1-1 1h-12a1 1 0 0 1-1-1z"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linejoin="round"
      />
    </svg>
  </button>

  <button
    type="button"
    class="item"
    class:active={panel === 'outline'}
    aria-pressed={panel === 'outline'}
    title="Contents"
    aria-label="Contents"
    onclick={() => choose('outline')}
  >
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <path
        d="M4 5.5h12M4 10h12M4 14.5h8"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
      />
    </svg>
  </button>

  <!-- Pinned to the bottom so the two groups read as different kinds of thing,
       and so a menu opening from here grows upward into free space. -->
  <div class="actions">
    <div class="slot" bind:this={slots.pick}>
      <button
        type="button"
        class="item"
        aria-haspopup="menu"
        aria-expanded={open === 'pick'}
        title="Open a file or folder"
        aria-label="Open a file or folder"
        onclick={() => toggle('pick')}
      >
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <rect
            x="3"
            y="3"
            width="14"
            height="14"
            rx="3"
            stroke="currentColor"
            stroke-width="1.4"
          />
          <path
            d="M10 6.75v6.5M6.75 10h6.5"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
          />
        </svg>
      </button>
      {#if open === 'pick'}
        {@render menu('Open', PICK_ROWS)}
      {/if}
    </div>

    <div class="slot" bind:this={slots.export}>
      <button
        type="button"
        class="item"
        aria-haspopup="menu"
        aria-expanded={open === 'export'}
        disabled={!canExport}
        aria-disabled={!canExport}
        title="Export this document"
        aria-label="Export this document"
        onclick={() => toggle('export')}
      >
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <path
            d="M6.5 8.5H5a1 1 0 0 0-1 1v6.5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V9.5a1 1 0 0 0-1-1h-1.5M10 12V3M7 5.75 10 3l3 2.75"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>
      {#if open === 'export'}
        {@render menu('Export', EXPORT_ROWS)}
      {/if}
    </div>
  </div>

  <!-- The split-editor toggle is NOT here. It switches how the document is
       shown rather than which panel is beside it, so it stays with the
       document. -->

  <!-- Settings is NOT here either. It is a floating card in the bottom-right
       corner, because it is opened, changed and dismissed rather than read
       alongside the document — see `settings/panel.svelte`. -->
</nav>

<style>
  /* The fallback containing block: a menu that ever lands outside a `.slot`
     still anchors to the rail rather than to the viewport. */
  .bar {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
    inline-size: var(--activity-bar-size);
    block-size: 100%;
    padding-block: var(--space-2);
    background: var(--bg-subtle);
    border-inline-end: 1px solid var(--border);
  }

  .actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
    margin-block-start: auto;
  }

  /* Each menu anchors to its own button, not to the rail — the rail's block
     edge is the window's, and that is nowhere near the button. */
  .slot {
    position: relative;
  }

  .item {
    display: grid;
    place-items: center;
    inline-size: 2.25rem;
    block-size: 2.25rem;
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
  }

  .item:hover:not(:disabled),
  .item[aria-expanded='true'] {
    color: var(--fg);
    background: color-mix(in oklab, var(--fg) 8%, transparent);
  }

  /* Dimmed, not hidden: hiding it would make the bottom group jump every time
     a document opens or closes. */
  .item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  /* The active view is marked with a colour AND an edge bar, not colour
     alone — the same reason links are underlined on focus. */
  .item.active {
    color: var(--accent);
    position: relative;
  }

  .item.active::before {
    content: '';
    position: absolute;
    inset-inline-start: calc(var(--space-2) * -1);
    inset-block: 0.35rem;
    inline-size: 2px;
    border-radius: 1px;
    background: var(--accent);
  }

  .item:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* Bottom edges aligned, so the menu grows upward and never runs off the
     window. `z-index: 1` only has to beat the panel dock beside it: the
     chrome layer is already a stacking context at 20, above the tab strip and
     under the settings card, and a bigger number here would claim a level it
     cannot reach. */
  .menu {
    position: absolute;
    inset-inline-start: 100%;
    inset-block-end: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    min-inline-size: 8rem;
    margin-inline-start: var(--space-2);
    padding: var(--space-1);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 8px 24px oklch(0% 0 0 / 20%);
  }

  .row {
    padding: var(--space-1) var(--space-3);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--fg);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    text-align: start;
    white-space: nowrap;
    cursor: pointer;
  }

  .row:hover {
    background: var(--bg-subtle);
  }

  /* Inset, as in the settings card: rows touch, so an outset ring would be
     painted over by the next row's background. */
  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
</style>

<script lang="ts">
  /**
   * The left edge: one button per panel.
   *
   * Why a bar rather than more floating buttons. Before this there were two
   * controls in two corners, each owning its own open state, and adding a
   * third and a fourth would have meant four corners and no answer for the
   * fifth. A bar is one place to look, one place to add to, and the shape
   * every editor has trained people on.
   *
   * It holds no state. Which panel is open belongs to `app.svelte`, because it
   * is read by things outside this component — the document's inset, the edit
   * controller — and a toggle that owns the truth about a layout it does not
   * lay out is how the two drift apart.
   */
  export type PanelId = 'explorer' | 'outline';

  interface Props {
    /** The open panel, or `null` when the document has the full width. */
    panel: PanelId | null;
    onpanel: (panel: PanelId | null) => void;
  }

  let { panel, onpanel }: Props = $props();

  /** Clicking the open panel closes it — the same affordance as VS Code. */
  function choose(id: PanelId): void {
    onpanel(panel === id ? null : id);
  }
</script>

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

  <!-- The split-editor toggle is NOT here. It switches how the document is
       shown rather than which panel is beside it, and it lives in the
       document's own top-right corner where an editor's view controls sit. -->

  <!-- Settings is NOT here either. It is a floating card in the bottom-right
       corner, because it is opened, changed and dismissed rather than read
       alongside the document — see `settings/panel.svelte`. -->
</nav>

<style>
  .bar {
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

  .item:hover {
    color: var(--fg);
    background: color-mix(in oklab, var(--fg) 8%, transparent);
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
</style>

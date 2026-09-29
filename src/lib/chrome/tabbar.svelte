<script lang="ts">
  /**
   * The open documents, along the top of the reading column.
   *
   * **Shown only from the second tab onwards.** Most launches are a double
   * click on one file, and a strip carrying a single tab above a single
   * document is chrome that says nothing the title bar does not. It appears
   * when it starts to mean something.
   *
   * Holds no state: which tabs exist and which is showing belong to
   * `app.svelte`, because the document, the editor and the sidebar's
   * highlight all read them.
   */
  import { tabLabel, type Tab } from '../tabs';

  interface Props {
    tabs: readonly Tab[];
    /** Index into `tabs`. */
    active: number;
    onselect: (index: number) => void;
    onclose: (index: number) => void;
  }

  let { tabs, active, onselect, onclose }: Props = $props();

  /**
   * Middle-click closes, the way it does in every browser.
   *
   * On `auxclick` rather than `mousedown`: the button that went down is not
   * necessarily the one that comes up over the same element.
   */
  function onAux(event: MouseEvent, index: number): void {
    if (event.button !== 1) return;
    event.preventDefault();
    onclose(index);
  }
</script>

{#if tabs.length > 1}
  <div class="tabbar" role="tablist" aria-label="Open documents">
    {#each tabs as tab, i (tab.path)}
      <div class="tab" class:on={i === active}>
        <button
          type="button"
          class="label"
          role="tab"
          aria-selected={i === active}
          title={tab.path}
          onclick={() => onselect(i)}
          onauxclick={(event) => onAux(event, i)}
        >
          {tabLabel(tab)}
        </button>
        <button
          type="button"
          class="close"
          title="Close"
          aria-label="Close {tabLabel(tab)}"
          onclick={() => onclose(i)}
        >
          <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
            <path
              d="M3 3l6 6M9 3l-6 6"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  /* Fixed, and inset by the chrome exactly as the document is: the strip
     belongs to the reading column, not to the window. `--tabbar-height` is
     published to `:root` by `app.svelte`, which is what pushes the document
     and the split toggle down — nothing here can reach either of them. */
  .tabbar {
    position: fixed;
    inset-block-start: 0;
    inset-inline-start: var(--chrome-inset, var(--activity-bar-size));
    inset-inline-end: 0;
    display: flex;
    align-items: stretch;
    block-size: var(--tabbar-height, 2.25rem);
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    background: var(--bg-subtle);
    border-block-end: 1px solid var(--border);
    font-family: var(--font-ui);
    z-index: 18;
  }

  .tabbar::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    flex: 0 1 auto;
    min-inline-size: 0;
    max-inline-size: 16rem;
    border-inline-end: 1px solid var(--border);
    color: var(--fg-muted);
  }

  .tab:hover {
    background: color-mix(in oklab, var(--fg) 6%, transparent);
  }

  /* The showing tab reads as the top of the page below it: same background,
     and an accent edge rather than colour alone — the rule the activity bar
     follows too. */
  .tab.on {
    position: relative;
    color: var(--fg);
    background: var(--bg);
  }

  .tab.on::before {
    content: '';
    position: absolute;
    inset-block-start: 0;
    inset-inline: 0;
    block-size: 2px;
    background: var(--accent);
  }

  button {
    background: none;
    border: 0;
    color: inherit;
    font-family: inherit;
    cursor: default;
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .label {
    flex: 1;
    min-inline-size: 0;
    padding: 0 var(--space-1) 0 var(--space-3);
    font-size: var(--text-small);
    text-align: start;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .close {
    display: grid;
    place-items: center;
    flex: none;
    padding-inline: var(--space-2);
    block-size: 100%;
    opacity: 0.45;
  }

  .close:hover {
    opacity: 1;
    color: var(--alert-caution);
  }
</style>

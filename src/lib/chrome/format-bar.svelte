<script lang="ts">
  /**
   * The formatting toolbar: icon buttons, shown while editing.
   *
   * Every button asks the editor for the same thing a key does — the inline ones
   * are the Word-style formats behind `Ctrl+B` and friends, the block ones are
   * `applyBlock` in `edit/format.ts`. Names live in the tooltip and the
   * accessible label, never on the button.
   *
   * The buttons take no focus on press (`mousedown` is cancelled): the selection
   * the button acts on is in the editor, and a button that stole focus first
   * would make the editor forget it.
   *
   * The Table button does not insert anything itself: it opens a small picker,
   * Word's grid, where the size is chosen by pointing at it. The same picker has
   * two number fields for a size the grid is too small for, and for the keyboard.
   */
  import { TABLE_MAX, type FormatKind } from '../edit/format';

  interface Props {
    onrun: (kind: FormatKind) => void;
    /** Asked for when a table size was chosen; rows count the header. Any number: the editor holds it to the limits. */
    ontable: (cols: number, rows: number) => void;
  }

  let { onrun, ontable }: Props = $props();

  /** The grid's size: the largest table a pointer can pick. */
  const GRID = { cols: 10, rows: 8 };

  let picking = $state(false);
  /** The cell under the pointer, as a size; 0 × 0 when none. */
  let hover = $state({ cols: 0, rows: 0 });
  let cols = $state(3);
  let rows = $state(3);
  let picker: HTMLElement | undefined = $state();

  function choose(c: number, r: number): void {
    picking = false;
    hover = { cols: 0, rows: 0 };
    ontable(c, r);
  }

  function press(item: Item): void {
    if (item.kind === 'table') picking = !picking;
    else onrun(item.kind);
  }

  /** Escape closes the picker first, instead of leaving the editor. */
  function onkey(event: KeyboardEvent): void {
    if (picking && event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      picking = false;
    }
  }

  function onoutside(event: MouseEvent): void {
    if (picking && !picker?.contains(event.target as Node)) picking = false;
  }

  interface Item {
    kind: FormatKind;
    label: string;
    /** SVG path data on a 20×20 grid, stroked. */
    d: string;
  }

  /** Groups, separated by a rule. */
  const GROUPS: Item[][] = [
    [
      { kind: 'bold', label: 'Bold (Ctrl+B)', d: 'M6 4h5a3 3 0 0 1 0 6H6zM6 10h6a3 3 0 0 1 0 6H6z' },
      { kind: 'italic', label: 'Italic (Ctrl+I)', d: 'M8 4h6M6 16h6M11 4L9 16' },
      { kind: 'strike', label: 'Strikethrough (Ctrl+Shift+X)', d: 'M4 10h12M13 6.5C12.5 5 11.3 4.5 10 4.5 8.3 4.5 7 5.3 7 6.8c0 3 6 1.5 6 4.7 0 1.5-1.4 2.5-3.1 2.5-1.4 0-2.6-.6-3.1-2' },
      { kind: 'code', label: 'Inline code (Ctrl+`)', d: 'M7 6l-4 4 4 4M13 6l4 4-4 4' },
      { kind: 'link', label: 'Link (Ctrl+K)', d: 'M8.5 11.5a3 3 0 0 0 4 0l3-3a3 3 0 0 0-4-4l-1 1M11.5 8.5a3 3 0 0 0-4 0l-3 3a3 3 0 0 0 4 4l1-1' },
    ],
    [
      { kind: 'h1', label: 'Heading 1', d: 'M3 4v12M9 4v12M3 10h6M13 8l2-1.5V16' },
      { kind: 'h2', label: 'Heading 2', d: 'M2 4v12M8 4v12M2 10h6M12 8a2 2 0 1 1 4 0c0 2-4 4-4 6h4' },
      { kind: 'h3', label: 'Heading 3', d: 'M2 4v12M8 4v12M2 10h6M12 7h4l-2.5 3a2.2 2.2 0 1 1-1.5 3.8' },
    ],
    [
      { kind: 'bullet', label: 'Bulleted list', d: 'M8 5h9M8 10h9M8 15h9M3.5 5h.01M3.5 10h.01M3.5 15h.01' },
      { kind: 'numbered', label: 'Numbered list', d: 'M9 5h8M9 10h8M9 15h8M3 4l1.5-1v4M3 11.5c0-1.5 3-1.5 3 0 0 1.5-3 2-3 3.5h3' },
      { kind: 'task', label: 'Task list', d: 'M3 4.5h4v4H3zM3 11.5h4v4H3zM4 6.5l1 1 1.5-2M10 6.5h7M10 13.5h7' },
      { kind: 'quote', label: 'Quote', d: 'M4 6v8M8 6.5h8M8 10h8M8 13.5h5' },
    ],
    [
      { kind: 'codeblock', label: 'Code block', d: 'M3 4h14v12H3zM8 8l-2 2 2 2M12 8l2 2-2 2' },
      { kind: 'table', label: 'Table', d: 'M3 4h14v12H3zM3 8.5h14M3 12h14M8 4v12M13 4v12' },
      { kind: 'rule', label: 'Horizontal rule', d: 'M3 10h14M6 6.5h.01M10 6.5h.01M14 6.5h.01M6 13.5h.01M10 13.5h.01M14 13.5h.01' },
      { kind: 'image', label: 'Image', d: 'M3 4h14v12H3zM3 14l4-4 3 3 3-2 4 4M13 7.5h.01' },
    ],
  ];
</script>

<svelte:window onkeydowncapture={onkey} onmousedowncapture={onoutside} />

<div class="format-bar" role="toolbar" aria-label="Formatting">
  {#each GROUPS as group, g (g)}
    {#if g > 0}<span class="rule" aria-hidden="true"></span>{/if}
    {#each group as item (item.kind)}
      {#snippet button()}
        <button
          type="button"
          title={item.label}
          aria-label={item.label}
          aria-haspopup={item.kind === 'table' ? 'dialog' : undefined}
          aria-expanded={item.kind === 'table' ? picking : undefined}
          data-kind={item.kind}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => press(item)}
        >
          <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <path d={item.d} stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      {/snippet}

      {#if item.kind === 'table'}
        <span class="anchor" bind:this={picker}>
          {@render button()}
          {#if picking}
            <div class="picker" role="dialog" aria-label="Insert table">
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="grid"
                style:--cols={GRID.cols}
                onmouseleave={() => (hover = { cols: 0, rows: 0 })}
              >
                {#each { length: GRID.rows } as _, r (r)}
                  {#each { length: GRID.cols } as __, c (c)}
                    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                    <span
                      class="cell"
                      class:on={c < hover.cols && r < hover.rows}
                      data-cols={c + 1}
                      data-rows={r + 1}
                      onmousedown={(e) => e.preventDefault()}
                      onmouseenter={() => (hover = { cols: c + 1, rows: r + 1 })}
                      onclick={() => choose(c + 1, r + 1)}
                    ></span>
                  {/each}
                {/each}
              </div>
              <p class="size" aria-live="polite">
                {hover.cols > 0 ? `${hover.cols} × ${hover.rows}` : 'Columns × rows'}
              </p>
              <form
                novalidate
                onsubmit={(e) => {
                  e.preventDefault();
                  choose(cols, rows);
                }}
              >
                <label>
                  Columns
                  <input type="number" min="1" max={TABLE_MAX.cols} bind:value={cols} />
                </label>
                <label>
                  Rows
                  <input type="number" min="1" max={TABLE_MAX.rows} bind:value={rows} />
                </label>
                <button type="submit" class="insert">Insert</button>
              </form>
            </div>
          {/if}
        </span>
      {:else}
        {@render button()}
      {/if}
    {/each}
  {/each}
</div>

<style>
  .format-bar {
    position: fixed;
    inset-block-end: var(--space-3);
    inset-inline-start: 50%;
    transform: translateX(-50%);
    z-index: 30;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: center;
    gap: 2px;
    max-inline-size: calc(100vw - var(--activity-bar-size) - var(--space-4));
    padding: var(--space-1);
    background: color-mix(in oklab, var(--bg) 92%, transparent);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.14);
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: 28px;
    block-size: 28px;
    padding: 0;
    color: var(--fg-muted);
    background: none;
    border: 0;
    border-radius: var(--radius-sm, 4px);
    cursor: pointer;
  }

  button:hover {
    color: var(--fg);
    background: var(--bg-subtle);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .anchor {
    position: relative;
    display: inline-flex;
  }

  .picker {
    position: absolute;
    inset-block-end: calc(100% + var(--space-2));
    inset-inline-start: 50%;
    transform: translateX(-50%);
    padding: var(--space-2);
    color: var(--fg);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.14);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), 18px);
    gap: 2px;
  }

  .cell {
    inline-size: 18px;
    block-size: 18px;
    border: 1px solid var(--border);
    border-radius: 2px;
    cursor: pointer;
  }

  .cell.on {
    background: color-mix(in oklab, var(--accent) 35%, transparent);
    border-color: var(--accent);
  }

  .size {
    margin: var(--space-1) 0;
    font-size: 0.8125rem;
    text-align: center;
    color: var(--fg-muted);
  }

  form {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--space-2);
    font-size: 0.75rem;
    color: var(--fg-muted);
  }

  label {
    display: grid;
    gap: 2px;
  }

  input {
    inline-size: 3.25rem;
    padding: 2px 4px;
    color: var(--fg);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 4px);
  }

  .insert {
    inline-size: auto;
    padding-inline: var(--space-2);
    color: var(--fg);
    background: var(--bg-subtle);
    border: 1px solid var(--border);
  }

  .rule {
    inline-size: 1px;
    block-size: 18px;
    margin-inline: var(--space-1);
    background: var(--border);
  }
</style>

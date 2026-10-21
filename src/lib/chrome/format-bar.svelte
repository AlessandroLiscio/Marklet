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
   */
  import type { FormatKind } from '../edit/format';

  interface Props {
    onrun: (kind: FormatKind) => void;
  }

  let { onrun }: Props = $props();

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

<div class="bar" role="toolbar" aria-label="Formatting">
  {#each GROUPS as group, g (g)}
    {#if g > 0}<span class="rule" aria-hidden="true"></span>{/if}
    {#each group as item (item.kind)}
      <button
        type="button"
        title={item.label}
        aria-label={item.label}
        data-kind={item.kind}
        onmousedown={(e) => e.preventDefault()}
        onclick={() => onrun(item.kind)}
      >
        <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <path d={item.d} stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>
    {/each}
  {/each}
</div>

<style>
  .bar {
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

  .rule {
    inline-size: 1px;
    block-size: 18px;
    margin-inline: var(--space-1);
    background: var(--border);
  }
</style>

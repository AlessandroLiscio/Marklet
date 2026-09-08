<script lang="ts">
  // The chrome: sidebar, outline, settings, search. Never the document itself —
  // that lives in a plain <article> outside this tree, because a 3 MB markdown
  // file must not go through a reactive renderer.
  //
  // Phases P3 (outline, settings), P5 (vault sidebar, search) and P7 (edit
  // controls) fill this in. What is here now is the one thing the shell needs
  // before any of them: the document's own metadata, proving the boot payload
  // arrived intact and giving the phases that follow something real to hang on.
  import { currentDocument } from './lib/doc';

  const doc = currentDocument();
</script>

{#if doc}
  <footer class="status" aria-label="Document status">
    <span class="status-item">{doc.outline.length} headings</span>
    <span class="status-item">{doc.line_map.length} blocks</span>
    {#if doc.encoding !== 'utf8'}
      <!-- Surfaced rather than silently repaired: a reader seeing mojibake
           deserves to know the file is not UTF-8, and `unknown` means the file
           contradicted its own byte-order mark. -->
      <span class="status-item status-warn">{doc.encoding}</span>
    {/if}
  </footer>
{/if}

<style>
  .status {
    position: fixed;
    inset-block-end: 0;
    inset-inline-end: 0;
    display: flex;
    gap: var(--space-3);
    padding: var(--space-1) var(--space-3);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
    background: color-mix(in oklab, var(--bg) 88%, transparent);
    border-start-start-radius: var(--radius-md);
    border-block-start: 1px solid var(--border);
    border-inline-start: 1px solid var(--border);
    backdrop-filter: blur(6px);
    pointer-events: none;
  }

  .status-warn {
    color: var(--alert-warning);
  }
</style>

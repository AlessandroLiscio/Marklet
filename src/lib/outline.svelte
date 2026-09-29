<script lang="ts">
  /**
   * Floating table of contents: click a heading to jump to it, scroll-spy
   * highlights whichever heading the reader is currently under.
   *
   * Reads the current position with `doc.ts`'s `visibleLine()` — which
   * already does the DOM work once, over the `data-l` anchors built at
   * document-load time — then binary-searches the much smaller `outline`
   * array via `nearestHeading` (`outline.ts`) to find the active heading.
   * Nothing here re-queries the DOM per scroll event, and the scroll handler
   * is throttled to one lookup per animation frame.
   *
   * Self-contained: resolves `#doc` itself rather than requiring a prop,
   * because it is not wired into `app.svelte` this wave (main thread does
   * that at wave close) and a prop nobody passes yet would only be dead
   * weight until then.
   */
  import { onDestroy, onMount } from 'svelte';
  import { currentDocument, scrollToSlug, visibleLine } from './doc';
  import { nearestHeading } from './outline';

  const doc = currentDocument();
  const outline = doc?.outline ?? [];

  // Opened and closed by the activity bar, which is the one place that knows
  // which panel is showing. This component used to own its own button and its
  // own corner; four such corners was the layout this replaced.
  let { open = false }: { open?: boolean } = $props();
  let activeSlug = $state<string | null>(null);

  let docRoot: HTMLElement | null = null;
  let rafId: number | null = null;

  function updateActive(): void {
    if (!docRoot || outline.length === 0) return;
    const line = visibleLine();
    activeSlug = nearestHeading(outline, line)?.slug ?? null;
  }

  function onScroll(): void {
    // Already scheduled for this frame — a scroll event can fire many times
    // between two frames, and only the last position before paint matters.
    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      rafId = null;
      updateActive();
    });
  }

  /**
   * Jumps, and leaves the panel open.
   *
   * It used to close itself on every click, which made "skim three sections"
   * three round trips through the activity bar. A docked panel is not a menu;
   * closing it is the button's job, not the link's.
   */
  function jump(slug: string): void {
    if (!docRoot) return;
    scrollToSlug(docRoot, slug);
  }

  onMount(() => {
    docRoot = document.getElementById('doc');
    if (!docRoot || outline.length === 0) return;
    updateActive();
    // passive: this handler never calls preventDefault, and the browser
    // should not wait to find that out before scrolling.
    window.addEventListener('scroll', onScroll, { passive: true });
  });

  onDestroy(() => {
    window.removeEventListener('scroll', onScroll);
    if (rafId !== null) cancelAnimationFrame(rafId);
  });
</script>

{#if open && outline.length > 0}
  <nav class="outline" aria-label="Table of contents">

    <ol id="outline-list" class="list">
        {#each outline as heading (heading.slug)}
          <li class="entry" class:active={heading.slug === activeSlug}>
            <button
              type="button"
              style="padding-inline-start: calc(var(--space-2) + {heading.level - 1}ch)"
              onclick={() => jump(heading.slug)}
            >
              {heading.text}
            </button>
          </li>
        {/each}
    </ol>
  </nav>
{/if}

<style>
  .outline {
    block-size: 100%;
    overflow-y: auto;
    font-family: var(--font-ui);
  }



  /* Flush with the panel: no card, no inset. The rows are the panel's
     contents, and a row whose hit area stops short of the edge reads as a
     misaligned list — which is exactly how it read. */
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .entry button {
    display: block;
    inline-size: 100%;
    /* The hit area spans the full width of the panel; the indent is padding
       INSIDE it, applied inline from the heading level. */
    padding-block: var(--space-2);
    padding-inline-end: var(--space-2);
    background: none;
    border: none;
    color: var(--fg-muted);
    font-family: inherit;
    font-size: var(--text-small);
    text-align: start;
    cursor: pointer;
    transition: color var(--duration-fast) ease, background var(--duration-fast) ease;
  }

  .entry button:hover {
    color: var(--fg);
    background: var(--bg-subtle);
  }

  .entry.active button {
    color: var(--accent);
    font-weight: 600;
    background: var(--accent-muted);
  }
</style>

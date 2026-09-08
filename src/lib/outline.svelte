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

  let open = $state(false);
  let activeSlug = $state<string | null>(null);

  let docRoot: HTMLElement | null = null;
  let rafId: number | null = null;

  function updateActive(): void {
    if (!docRoot || outline.length === 0) return;
    const line = visibleLine(docRoot);
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

  function jump(slug: string): void {
    if (!docRoot) return;
    scrollToSlug(docRoot, slug);
    open = false;
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

{#if outline.length > 0}
  <nav class="outline" aria-label="Table of contents">
    <button
      type="button"
      class="toggle"
      aria-expanded={open}
      aria-controls="outline-list"
      onclick={() => (open = !open)}
    >
      <!-- Inline SVG per the icon rule — no icon font, no icon package. -->
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M2 4h12M2 8h12M2 12h8"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
        />
      </svg>
      <span>Contents</span>
    </button>

    {#if open}
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
    {/if}
  </nav>
{/if}

<style>
  .outline {
    position: fixed;
    inset-block-start: var(--space-4);
    inset-inline-end: var(--space-4);
    font-family: var(--font-ui);
    z-index: 10;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: color-mix(in oklab, var(--bg) 88%, transparent);
    color: var(--fg-muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    font-size: var(--text-small);
    font-family: inherit;
    cursor: pointer;
    backdrop-filter: blur(6px);
    transition: color var(--duration-fast) ease, border-color var(--duration-fast) ease;
  }

  .toggle:hover {
    color: var(--fg);
    border-color: var(--fg-muted);
  }

  .list {
    list-style: none;
    margin: var(--space-2) 0 0;
    padding: var(--space-2);
    max-block-size: min(70vh, 32rem);
    overflow-y: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  .entry button {
    display: block;
    width: 100%;
    padding: var(--space-1) var(--space-2);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
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
  }
</style>

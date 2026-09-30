<script lang="ts">
  /**
   * Everything the keyboard does, on one sheet.
   *
   * **Why a sheet and not more buttons.** An audit found 34 capabilities with
   * no on-screen control at all — live preview, image paste, the snippets,
   * most of the tab commands. Marklet was about a third discoverable. Thirty-
   * four buttons would have been a worse window than thirty-four secrets: this
   * is a reading window, and the reading column is what it is for. So the long
   * tail gets one page that lists it, and only the one control with an obvious
   * home — live preview, whose sibling Split already had a button — became a
   * button.
   *
   * **Why it is lazy.** `app.svelte` reaches this through `import()`, like
   * KaTeX and CodeMirror, so the markup and the list arrive only when someone
   * asks for them — a 2.4 KiB chunk of its own, off the boot path, which is
   * the rule for everything that is not reading (`CLAUDE.md`, architecture
   * invariant 3).
   *
   * Its *styles* are not lazy, and no component's are: Vite collects every
   * scoped block into the one boot stylesheet. This one costs about 0.1 KiB
   * gzipped of the 6.0 KiB budget, which is why the sheet is laid out with the
   * tokens already there rather than bringing any of its own.
   *
   * The content is `src/lib/shortcuts.ts` and the snippet table itself; nothing
   * is written twice. `scripts/check-shortcuts.mjs` fails the build when the
   * list and the handlers disagree.
   */
  import { GROUPS } from '../shortcuts';
  import { SNIPPETS } from '../edit/snippets';

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  /** Full-edition groups and rows are simply absent from the lite build. */
  const full = __MARKLET_EDITION__ === 'full';
  const groups = $derived(
    GROUPS.filter((group) => full || group.full !== true).map((group) => ({
      ...group,
      items: group.items.filter((item) => full || item.full !== true),
    })),
  );

  const words = Object.keys(SNIPPETS);

  let card = $state<HTMLElement>();

  /**
   * Escape closes, in capture with `stopPropagation`.
   *
   * The same reason the rail's menus do it: `app.svelte` turns a bubbling
   * Escape into "leave edit mode", and one keypress must not both dismiss this
   * sheet and throw the reader out of the editor behind it.
   */
  function onKey(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.stopPropagation();
    onclose();
  }

  $effect(() => {
    window.addEventListener('keydown', onKey, true);
    // Focus moves in so the sheet can be read and dismissed by keyboard alone
    // — it is the one panel most likely to be opened by someone who has just
    // discovered the keyboard does anything.
    card?.focus();
    return () => window.removeEventListener('keydown', onKey, true);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div
  class="card"
  role="dialog"
  aria-modal="true"
  aria-label="Keyboard shortcuts"
  tabindex="-1"
  bind:this={card}
>
  <header>
    <h2>Keyboard</h2>
    <button type="button" class="close" aria-label="Close" onclick={onclose}>
      <svg viewBox="0 0 14 14" width="13" height="13" aria-hidden="true">
        <path
          d="M3.5 3.5l7 7M10.5 3.5l-7 7"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
        />
      </svg>
    </button>
  </header>

  <div class="scroll">
    {#each groups as group (group.title)}
      <section>
        <h3>{group.title}</h3>
        {#if group.blurb}<p class="blurb">{group.blurb}</p>{/if}

        {#if group.snippets}
          <ul class="words">
            {#each words as word (word)}
              <li><kbd>{word}</kbd></li>
            {/each}
          </ul>
        {:else}
          <dl>
            {#each group.items as item (item.keys)}
              <dt>
                {#each item.keys.split(', ') as chord, i (chord)}
                  {#if i > 0}<span class="or">or</span>{/if}
                  <kbd>{chord}</kbd>
                {/each}
              </dt>
              <dd>{item.what}</dd>
            {/each}
          </dl>
        {/if}
      </section>
    {/each}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in oklab, var(--fg) 34%, transparent);
    z-index: 44;
  }

  /* Above the settings card, because it is opened over everything and
     dismissed; below nothing, because nothing else is modal. */
  .card {
    position: fixed;
    inset-block: 3rem;
    inset-inline-start: 50%;
    translate: -50% 0;
    inline-size: min(44rem, calc(100vw - 2rem));
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg, 10px);
    box-shadow: 0 18px 48px oklch(0% 0 0 / 28%);
    font-family: var(--font-ui);
    color: var(--fg);
    z-index: 45;
  }

  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-block-end: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    font-size: var(--text-h4);
    font-weight: 600;
  }

  .close {
    display: grid;
    place-items: center;
    inline-size: 1.75rem;
    block-size: 1.75rem;
    padding: 0;
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--fg-muted);
    cursor: pointer;
  }

  .close:hover {
    color: var(--fg);
    background: color-mix(in oklab, var(--fg) 8%, transparent);
  }

  .close:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .scroll {
    overflow-y: auto;
    padding: var(--space-2) var(--space-4) var(--space-4);
  }

  section {
    margin-block-start: var(--space-4);
  }

  h3 {
    margin: 0;
    font-size: var(--text-small);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
  }

  .blurb {
    margin: var(--space-1) 0 0;
    font-size: var(--text-small);
    color: var(--fg-muted);
  }

  /* Two columns on anything but a narrow window: the keys line up down the
     left, which is how a sheet like this is read — you scan the right-hand
     side for the thing you want and take the key beside it. */
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    align-items: baseline;
    gap: var(--space-1) var(--space-3);
    margin: var(--space-2) 0 0;
  }

  @media (max-width: 34rem) {
    dl {
      grid-template-columns: 1fr;
      gap: 0;
    }

    dd {
      margin-block-end: var(--space-2);
    }
  }

  dt {
    white-space: nowrap;
  }

  dd {
    margin: 0;
    font-size: var(--text-small);
    color: var(--fg-muted);
  }

  kbd {
    display: inline-block;
    padding: 0.1em 0.4em;
    border: 1px solid var(--border);
    border-block-end-width: 2px;
    border-radius: 4px;
    background: var(--bg-subtle);
    color: var(--fg);
    font-family: var(--font-code);
    font-size: 0.8em;
    white-space: nowrap;
  }

  .or {
    font-size: var(--text-small);
    color: var(--fg-muted);
    margin-inline: 0.25em;
  }

  .words {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    margin: var(--space-2) 0 0;
    padding: 0;
    list-style: none;
  }
</style>

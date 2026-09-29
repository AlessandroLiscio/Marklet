<script lang="ts">
  /**
   * Who links here.
   *
   * The panel that makes a folder of markdown files a vault: a wiki-link is
   * one-directional in the text and bidirectional in the graph, and this is
   * the second direction. It is a lookup in the index, not a scan, which is why
   * it can be shown for every note without a cost the user notices.
   *
   * An empty panel is a real answer and says so. So is a note nothing links to
   * yet — a vault is mostly that, at the start.
   */
  import { backlinksFor, openInNewWindow, type Backlink } from '../ipc';

  interface Props {
    /** The open note, vault-relative. `null` when no vault note is open. */
    path?: string | null;
    onopen?: (path: string, line: number) => void;
  }

  let { path = null, onopen }: Props = $props();

  let links = $state<Backlink[]>([]);
  let loading = $state(false);

  /**
   * Click opens here; Ctrl+click (Cmd on macOS) opens a second window.
   *
   * The same two gestures every browser has, and the reason the row carries a
   * visible open glyph: a list of paths does not otherwise say that it is a
   * list of things you can go to.
   */
  function go(event: MouseEvent, link: Backlink): void {
    if (event.ctrlKey || event.metaKey) {
      void openInNewWindow(link.path).catch(() => {
        // The window could not be created — fall back to opening it here,
        // which is what the plain click would have done.
        onopen?.(link.path, link.line);
      });
      return;
    }
    onopen?.(link.path, link.line);
  }

  $effect(() => {
    const target = path;
    if (target === null) {
      links = [];
      return;
    }

    // Guarded against the note changing while the round trip is in flight:
    // clicking through three notes quickly must not leave the panel showing
    // the first one's referrers.
    let current = true;
    loading = true;
    void backlinksFor(target)
      .then((result) => {
        if (current) links = result;
      })
      .catch(() => {
        if (current) links = [];
      })
      .finally(() => {
        if (current) loading = false;
      });

    return () => {
      current = false;
    };
  });
</script>

<section class="backlinks" aria-label="Backlinks">
  <!-- "Linked from", not "Backlinks": the word is jargon and it does not say
       which way the arrow points. Every row here is a note that points AT the
       one being read, and the heading is the only place that can say so. -->
  <h2>
    Linked from
    {#if links.length > 0}<span class="count">{links.length}</span>{/if}
  </h2>

  {#if path === null}
    <p class="empty">No note open.</p>
  {:else if loading}
    <p class="empty">…</p>
  {:else if links.length === 0}
    <p class="empty">Nothing links here yet.</p>
  {:else}
    <ul>
      {#each links as link, i (`${link.path}:${link.line}:${i}`)}
        <li>
          <!-- The path leads, with its line, laid out exactly like a search
               hit — same shape, same accent on the line number. It used to
               lead with the note's H1 over a bare `[[target]] · line 3`, and
               since the target is always the note you are already reading,
               every row said the same thing twice and named the source last.
               The title is still here, as the row's tooltip. -->
          <button
            type="button"
            title="{link.title} — Ctrl+click to open in a new window"
            onclick={(event) => go(event, link)}
          >
            <span class="where">{link.path}<span class="line">:{link.line}</span></span>
            <!-- Inline SVG per the icon rule: no icon font, no icon package.
                 A sheet with an arrow leaving it — "open this elsewhere" —
                 rather than a bare arrow, which reads as a direction and not
                 as a destination. -->
            <svg class="go" viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
              <path
                d="M9.5 2.5H13v3.5M13 2.5 7.75 7.75"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
              <path
                d="M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .backlinks {
    display: flex;
    flex-direction: column;
    max-block-size: 40%;
    border-block-start: 1px solid var(--border);
  }

  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    padding: var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--fg-muted);
  }

  .count {
    padding-inline: var(--space-1);
    font-weight: 400;
    color: var(--fg-muted);
    background: var(--bg-subtle);
    border-radius: var(--radius-sm);
  }

  ul {
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  /* A rule between rows, so two paths read as two answers rather than as one
     wrapped line. */
  li + li {
    border-block-start: 1px solid var(--border);
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    inline-size: 100%;
    padding: var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    text-align: start;
    color: var(--fg);
    background: none;
    border: 0;
    cursor: default;
  }

  button:hover {
    background: var(--bg-subtle);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .where {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg);
  }

  .line {
    color: var(--accent);
  }

  /* Dim at rest, accent under the pointer: present enough to say the row goes
     somewhere, quiet enough not to compete with four paths above it. */
  .go {
    flex: none;
    color: var(--fg-muted);
    opacity: 0.55;
    transition: color var(--duration-fast) ease, opacity var(--duration-fast) ease;
  }

  button:hover .go,
  button:focus-visible .go {
    color: var(--accent);
    opacity: 1;
  }

  .empty {
    margin: 0;
    padding: 0 var(--space-2) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }
</style>

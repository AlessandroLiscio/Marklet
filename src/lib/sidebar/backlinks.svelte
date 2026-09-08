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
  import { backlinksFor, type Backlink } from '../ipc';

  interface Props {
    /** The open note, vault-relative. `null` when no vault note is open. */
    path?: string | null;
    onopen?: (path: string, line: number) => void;
  }

  let { path = null, onopen }: Props = $props();

  let links = $state<Backlink[]>([]);
  let loading = $state(false);

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
  <h2>
    Backlinks
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
          <button type="button" onclick={() => onopen?.(link.path, link.line)}>
            <span class="title">{link.title}</span>
            <span class="via">[[{link.target}]] · line {link.line}</span>
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

  button {
    display: block;
    inline-size: 100%;
    padding: var(--space-1) var(--space-2);
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

  .title,
  .via {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .via {
    font-family: var(--font-code);
    color: var(--fg-muted);
  }

  .empty {
    margin: 0;
    padding: 0 var(--space-2) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }
</style>

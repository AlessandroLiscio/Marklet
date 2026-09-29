<script lang="ts">
  /**
   * What this note points at.
   *
   * It was the other direction for one release — *Linked from*, the notes
   * pointing **at** the open one. Asked to be turned around, and the request
   * is the better half: the outgoing list is the one you can act on while
   * reading, it needs no index to be built first, and it can include the
   * `https://` links, which a vault graph has no opinion about.
   *
   * `backlinks_for` is still a command and still correct — the panel that
   * consumed it is what changed.
   */
  import { openExternal, openInNewWindow } from '../ipc';
  import { collectLinks, type DocLink } from './links';

  interface Props {
    /**
     * The open note, vault-relative — not read, only watched.
     *
     * The links come out of the rendered document, so what this prop is for is
     * knowing *when* to read it again. `app.svelte` replaces `#doc`'s markup
     * before it updates the state this is derived from, so by the time the
     * effect runs the new document is already in the DOM.
     */
    path?: string | null;
    onopen?: (path: string, line: number) => void;
  }

  let { path = null, onopen }: Props = $props();

  let links = $state<DocLink[]>([]);

  $effect(() => {
    // Named so the effect depends on it; the document itself is the source.
    void path;
    const root = document.getElementById('doc');
    links = root === null ? [] : collectLinks(root);
  });

  /**
   * Click follows the link; Ctrl+click (Cmd on macOS) opens it in a second
   * window — the two gestures a browser has, for the two kinds of link.
   *
   * A web link goes to the OS browser either way: it already lands in a
   * separate window, and Rust re-checks the scheme before handing it over.
   */
  function go(event: MouseEvent, link: DocLink): void {
    if (link.kind === 'unresolved') return;

    if (link.kind === 'web') {
      void openExternal(link.target).catch(() => {
        /* the scheme allowlist refused it, or no browser answered */
      });
      return;
    }

    if (event.ctrlKey || event.metaKey) {
      void openInNewWindow(link.target).catch(() => {
        // The window could not be created — open it here instead, which is
        // what the plain click would have done.
        onopen?.(link.target, link.line);
      });
      return;
    }
    onopen?.(link.target, link.line);
  }

  function hint(link: DocLink): string {
    if (link.kind === 'unresolved') return `Nothing in this vault answers to “${link.target}”`;
    if (link.kind === 'web') return `${link.target} — opens in your browser`;
    return `${link.target} — Ctrl+click to open in a new window`;
  }
</script>

<section class="links" aria-label="Links in this note">
  <h2>
    Links
    {#if links.length > 0}<span class="count">{links.length}</span>{/if}
  </h2>

  {#if path === null}
    <p class="empty">No note open.</p>
  {:else if links.length === 0}
    <p class="empty">This note links nowhere.</p>
  {:else}
    <ul>
      {#each links as link, i (`${link.kind}:${link.target}:${link.line}:${i}`)}
        <li>
          <button
            type="button"
            class={link.kind}
            disabled={link.kind === 'unresolved'}
            title={hint(link)}
            onclick={(event) => go(event, link)}
          >
            <!-- Inline SVG per the icon rule: no icon font, no icon package.
                 The leading glyph is the one thing that has to be readable at
                 a glance — a note and a web page are followed in different
                 places and a reader should know which before clicking. -->
            {#if link.kind === 'web'}
              <svg class="kind" viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                <circle cx="8" cy="8" r="5.75" fill="none" stroke="currentColor" stroke-width="1.3" />
                <ellipse cx="8" cy="8" rx="2.5" ry="5.75" fill="none" stroke="currentColor" stroke-width="1.3" />
                <path d="M2.5 6.2h11M2.5 9.8h11" stroke="currentColor" stroke-width="1.3" />
              </svg>
            {:else}
              <svg class="kind" viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                <path
                  d="M4 1.75h5L12.25 5v9.25H4z"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.3"
                  stroke-linejoin="round"
                />
                <path d="M8.75 2v3.25H12" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" />
              </svg>
            {/if}

            <span class="label">{link.label}</span>
            {#if link.line > 0}<span class="line">:{link.line}</span>{/if}

            {#if link.kind !== 'unresolved'}
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
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .links {
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

  /* A rule between rows, so two links read as two answers rather than as one
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

  button:hover:not(:disabled) {
    background: var(--bg-subtle);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  /* A wiki-link to a note nobody has written yet. Struck through and dimmed,
     the same answer the document itself gives it — and inert, because
     navigating to a note that does not exist is not an improvement on doing
     nothing. */
  button:disabled {
    color: var(--fg-muted);
    opacity: 0.6;
  }

  button:disabled .label {
    text-decoration: line-through;
  }

  .label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    flex: none;
    color: var(--fg-muted);
  }

  button.web .kind {
    color: var(--accent);
  }

  .line {
    flex: none;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }

  /* Dim at rest, accent under the pointer: present enough to say the row goes
     somewhere, quiet enough not to compete with the labels above it. */
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

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
  import { onDestroy, onMount } from 'svelte';
  import { DOCUMENT_EVENT, revealElement } from '../doc';
  import { openExternal, openInNewWindow } from '../ipc';
  import { collectLinkRows, type DocLink, type LinkRow } from './links';

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
    /** Opens another note. No line: see {@link open}. */
    onopen?: (path: string) => void;
  }

  let { path = null, onopen }: Props = $props();

  let rows = $state<LinkRow[]>([]);

  function reread(): void {
    const root = document.getElementById('doc');
    rows = root === null ? [] : collectLinkRows(root);
  }

  // Driven by the document being replaced, not by the `path` prop.
  //
  // The prop is the wrong signal and reading it was a bug: a document is also
  // replaced *without* its path changing — re-rendered once the vault index
  // exists, so its wiki-links resolve, or reloaded by the watcher after a
  // save. The panel then kept showing links read from markup that no longer
  // existed, which is why notes sitting in the tree beside it stayed struck
  // through after the document itself had been fixed.
  onMount(() => {
    reread();
    document.addEventListener(DOCUMENT_EVENT, reread);
  });

  onDestroy(() => document.removeEventListener(DOCUMENT_EVENT, reread));

  /**
   * The row takes you to where the link is **written**; only the button
   * follows it.
   *
   * Two different destinations were behind one gesture, and the row is the
   * easier of the two to hit by accident: a panel listing a document's links
   * is a way of navigating the document, and a stray click that replaced the
   * document — or opened a browser — lost the reader's place to answer a
   * question they had not asked. Following a link is now something you aim at.
   */
  function jump(row: LinkRow): void {
    revealElement(row.el, row.link.line);
  }

  /**
   * Follows the link: a note here, or in a second window with Ctrl (Cmd on
   * macOS), and a web link to the OS browser — which is already a separate
   * window, and where Rust re-checks the scheme before handing it over.
   */
  function open(event: MouseEvent, link: DocLink): void {
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
        onopen?.(link.target);
      });
      return;
    }
    // No line: `link.line` is where the link is written in *this* document and
    // means nothing in the one being opened.
    onopen?.(link.target);
  }

  function openHint(link: DocLink): string {
    if (link.kind === 'web') return `Open ${link.target} in your browser`;
    return `Open ${link.target} — Ctrl+click for a new window`;
  }
</script>

<section class="links" aria-label="Links in this note">
  <h2>
    Links
    {#if rows.length > 0}<span class="count">{rows.length}</span>{/if}
  </h2>

  {#if path === null}
    <p class="empty">No note open.</p>
  {:else if rows.length === 0}
    <p class="empty">This note links nowhere.</p>
  {:else}
    <ul>
      {#each rows as { link }, i (`${link.kind}:${link.target}:${link.line}:${i}`)}
        <li class={link.kind}>
          <button
            type="button"
            class="jump"
            disabled={link.line === 0}
            title={link.line > 0
              ? `Show where this link is written, on line ${link.line}`
              : 'This link is not inside a numbered block'}
            onclick={() => jump(rows[i]!)}
          >
            <!-- Inline SVG per the icon rule: no icon font, no icon package.
                 The leading glyph is the one thing that has to be readable at
                 a glance — a note and a web page are followed in different
                 places and a reader should know which before aiming at the
                 button. -->
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
          </button>

          <!-- A button of its own, not a glyph inside the row: it is a second
               destination, and a nested <button> is not valid HTML anyway. -->
          {#if link.kind !== 'unresolved'}
            <button
              type="button"
              class="open"
              title={openHint(link)}
              aria-label={openHint(link)}
              onclick={(event) => open(event, link)}
            >
              <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
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
          {/if}
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

  /* Two targets on one row, so the row is the flex container and each button
     is its own hit area — a nested <button> is not valid HTML, and these are
     genuinely two destinations. */
  li {
    display: flex;
    align-items: stretch;
  }

  button {
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg);
    background: none;
    border: 0;
    cursor: default;
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .jump {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: 1;
    min-inline-size: 0;
    padding: var(--space-2);
    text-align: start;
  }

  .jump:hover:not(:disabled) {
    background: var(--bg-subtle);
  }

  .open {
    display: grid;
    place-items: center;
    flex: none;
    padding-inline: var(--space-2);
    color: var(--fg-muted);
    opacity: 0.55;
    transition: color var(--duration-fast) ease, opacity var(--duration-fast) ease,
      background var(--duration-fast) ease;
  }

  .open:hover,
  .open:focus-visible {
    color: var(--accent);
    opacity: 1;
    background: var(--bg-subtle);
  }

  /* A wiki-link to a note nobody has written yet. Struck through and dimmed,
     the same answer the document itself gives it — and with no open button,
     because navigating to a note that does not exist is not an improvement on
     doing nothing. The row still jumps to where it is written. */
  li.unresolved .label {
    color: var(--fg-muted);
    text-decoration: line-through;
  }

  li.unresolved .kind {
    opacity: 0.6;
  }

  .jump:disabled {
    color: var(--fg-muted);
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

  li.web .kind {
    color: var(--accent);
  }

  .line {
    flex: none;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }

  .empty {
    margin: 0;
    padding: 0 var(--space-2) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }
</style>

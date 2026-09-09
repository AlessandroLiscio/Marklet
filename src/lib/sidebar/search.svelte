<script lang="ts">
  /**
   * The search box, consuming **streamed** results.
   *
   * Rust emits `search-result` per hit while the walk is still running, so the
   * first match appears before the last file is read. Results are appended as
   * they arrive rather than collected and sorted: sorting would mean waiting
   * for the end, which is the one thing the streaming design exists to avoid.
   *
   * A newer search invalidates an older one. Rust tags every hit with a search
   * id and stops the stale walk on its next callback; the guard here is the
   * frontend half of the same rule, for hits already in flight.
   */
  import {
    onSearchDone,
    onSearchResult,
    searchVault,
    subscribeAll,
    type Hit,
    type SearchStats,
  } from '../ipc';

  interface Props {
    /** Reported by the sidebar so search can be disabled without a vault. */
    enabled?: boolean;
    onopen?: (path: string, line: number) => void;
  }

  let { enabled = true, onopen }: Props = $props();

  let text = $state('');
  let regex = $state(false);
  let hits = $state<Hit[]>([]);
  let stats = $state<SearchStats | null>(null);
  let running = $state(false);

  /** The search whose results are still wanted. */
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Subscribing once for the component's life, not once per query: `listen`
  // is a round trip, and doing it per keystroke would drop the first hits of
  // every search to a race it cannot win.
  $effect(() =>
    subscribeAll([
      onSearchResult(({ hit }) => {
        if (running) hits = [...hits, hit];
      }),
      onSearchDone((s) => {
        stats = s;
        running = false;
      }),
    ])
  );

  /**
   * Debounced, because a keystroke is not a query.
   *
   * 140 ms is under the ~200 ms at which a pause starts to feel like a wait,
   * and above the interval between keys of anyone typing normally — so a word
   * costs one search rather than five.
   */
  function onInput(): void {
    clearTimeout(timer);
    timer = setTimeout(run, 140);
  }

  async function run(): Promise<void> {
    const query = text.trim();
    const mine = ++generation;

    hits = [];
    stats = null;

    if (!enabled || query.length === 0) {
      running = false;
      return;
    }

    running = true;
    try {
      const result = await searchVault({ text: query, regex });
      if (mine === generation) {
        stats = result;
        running = false;
      }
    } catch {
      // A search failing (no vault, a vanished folder) is not worth an alert:
      // the empty state with no "searching" spinner already says it.
      if (mine === generation) running = false;
    }
  }

  function summary(s: SearchStats): string {
    if (s.regex_unavailable) return 'Regex search needs the full edition — searched literally';
    if (s.hits === 0) return `No matches in ${s.files_scanned} notes`;
    const more = s.truncated ? '+' : '';
    return `${s.hits}${more} in ${s.files_matched} of ${s.files_scanned} notes · ${s.elapsed_ms} ms`;
  }
</script>

<div class="search">
  <div class="box">
    <input
      type="search"
      placeholder="Search the vault"
      aria-label="Search the vault"
      bind:value={text}
      oninput={onInput}
      disabled={!enabled}
    />
    {#if __MARKLET_EDITION__ === 'full'}
      <!-- Full only: the ripgrep stack is +1.2-1.8 MB, which is 45% of lite's
           entire installer ceiling. Lite gets literal multi-term AND. -->
      <label class="regex" title="Regular expression">
        <input type="checkbox" bind:checked={regex} onchange={run} disabled={!enabled} />
        <span>.*</span>
      </label>
    {/if}
  </div>

  {#if stats}
    <p class="summary" class:warn={stats.regex_unavailable}>{summary(stats)}</p>
  {:else if running}
    <p class="summary">Searching…</p>
  {/if}

  <ul class="results">
    {#each hits as hit, i (`${hit.path}:${hit.line}:${i}`)}
      <li>
        <button type="button" onclick={() => onopen?.(hit.path, hit.line)}>
          <span class="where">{hit.path}<span class="line">:{hit.line}</span></span>
          <span class="excerpt">
            <!-- Spans, not offsets, and text nodes, not innerHTML: the excerpt
                 is untrusted file content and never becomes markup. -->
            {#each hit.spans as span, j (j)}{#if span.hit}<mark>{span.text}</mark
                >{:else}{span.text}{/if}{/each}
          </span>
        </button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .search {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    flex: 1;
  }

  .box {
    display: flex;
    gap: var(--space-1);
    padding: var(--space-2);
    border-block-end: 1px solid var(--border);
  }

  input[type='search'] {
    flex: 1;
    min-inline-size: 0;
    padding: var(--space-1) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  input[type='search']:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .regex {
    display: flex;
    align-items: center;
    gap: 2px;
    font-family: var(--font-code);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }

  .summary {
    margin: 0;
    padding: var(--space-1) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    color: var(--fg-muted);
  }

  .summary.warn {
    color: var(--alert-warning);
  }

  .results {
    flex: 1;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  .results button {
    display: block;
    inline-size: 100%;
    padding: var(--space-1) var(--space-2);
    font-family: var(--font-ui);
    font-size: var(--text-small);
    text-align: start;
    color: var(--fg);
    background: none;
    border: 0;
    border-block-end: 1px solid var(--border);
    cursor: default;
  }

  .results button:hover {
    background: var(--bg-subtle);
  }

  .results button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .where {
    display: block;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .line {
    color: var(--accent);
  }

  .excerpt {
    display: block;
    font-family: var(--font-code);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  mark {
    color: var(--fg);
    background: var(--accent-muted);
    border-radius: 2px;
  }
</style>

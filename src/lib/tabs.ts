/**
 * Open documents, as a list with one of them active.
 *
 * **A tab is a record, not a rendered document.** Only the active tab's markup
 * is in the DOM; switching re-renders from disk. Keeping N documents parsed and
 * attached would be the obvious implementation and the wrong one here: a 3 MB
 * markdown file is tens of thousands of nodes, `doc.ts` holds exactly one
 * anchor index, and the whole design rests on there being one document to be
 * the document. Re-rendering costs what the watcher's live reload already
 * costs on every save, and it keeps memory flat however long the session runs.
 *
 * What a tab therefore has to carry is what re-rendering does not restore:
 * where the reader was. Everything else comes back from the file.
 *
 * Pure — no DOM, no IPC. `tests/unit/tabs.test.ts`.
 */

/** One open document. */
export interface Tab {
  /** The absolute path. The identity: one tab per file, never two. */
  path: string;
  /** The vault-relative path, or `null` outside a vault. */
  rel: string | null;
  /** What the tab is labelled with. */
  title: string;
  /**
   * The source line that was at the top of the viewport when this tab was last
   * left, or 0 for the top.
   *
   * A source line and not a pixel offset, for the same reason scroll restore
   * across a reflow uses one: the metrics after a re-render are not guaranteed
   * to be the metrics before it.
   */
  line: number;
}

/** The tab strip: every open document, and which one is showing. */
export interface TabState {
  tabs: Tab[];
  /** Index into `tabs`, or `-1` when nothing is open. */
  active: number;
}

export const EMPTY: TabState = { tabs: [], active: -1 };

/** The active tab, or `null`. */
export function activeTab(state: TabState): Tab | null {
  return state.tabs[state.active] ?? null;
}

/** Where `path` already is, or `-1`. */
export function indexOfPath(state: TabState, path: string): number {
  return state.tabs.findIndex((t) => t.path === path);
}

/**
 * Opens a document, or reveals it if it is already open.
 *
 * **Never two tabs for one file.** Opening something already open activates it
 * instead — which is what every editor does, and what stops a wiki-link
 * followed twice from filling the strip with the same note.
 *
 * A new tab goes *after the active one*, not at the end: a tab opened from a
 * link belongs next to what linked to it, and a strip that grows outward from
 * where you are stays readable in a way one that grows at the far right does
 * not.
 *
 * The new tab is shown. There is no background variant: opening something is
 * asking to read it, and a modifier that opened it somewhere you could not see
 * was one more thing to know before the feature did anything.
 */
export function openTab(state: TabState, tab: Tab): TabState {
  const at = indexOfPath(state, tab.path);
  if (at !== -1) {
    // Already open. Its remembered line is the reader's, not this caller's, so
    // it is left alone unless the caller asked for a specific one.
    const tabs =
      tab.line > 0 ? state.tabs.map((t, i) => (i === at ? { ...t, line: tab.line } : t)) : state.tabs;
    return { tabs, active: at };
  }

  const insert = state.active === -1 ? state.tabs.length : state.active + 1;
  return {
    tabs: [...state.tabs.slice(0, insert), tab, ...state.tabs.slice(insert)],
    active: insert,
  };
}

/**
 * Closes one tab.
 *
 * The tab to the right takes over, and the one to the left when there is no
 * right — closing the last tab of a group should not jump you to the first.
 * Closing an inactive tab must not change which document is showing, which is
 * why the active index is recomputed from the tab rather than from the number.
 */
export function closeTab(state: TabState, index: number): TabState {
  if (index < 0 || index >= state.tabs.length) return state;

  const wasActive = state.tabs[state.active] ?? null;
  const tabs = state.tabs.filter((_, i) => i !== index);
  if (tabs.length === 0) return EMPTY;

  if (index !== state.active && wasActive !== null) {
    return { tabs, active: tabs.indexOf(wasActive) };
  }
  return { tabs, active: Math.min(index, tabs.length - 1) };
}

/** Shows a tab by index; out of range is ignored rather than clearing. */
export function activateTab(state: TabState, index: number): TabState {
  if (index < 0 || index >= state.tabs.length) return state;
  return { ...state, active: index };
}

/** Records where the reader was in the active tab, before leaving it. */
export function rememberLine(state: TabState, line: number): TabState {
  if (state.active === -1) return state;
  return {
    ...state,
    tabs: state.tabs.map((t, i) => (i === state.active ? { ...t, line } : t)),
  };
}

/** Moves the active tab by `delta`, wrapping — Ctrl+Tab and Ctrl+Shift+Tab. */
export function cycle(state: TabState, delta: number): TabState {
  if (state.tabs.length === 0) return state;
  const n = state.tabs.length;
  return { ...state, active: (((state.active + delta) % n) + n) % n };
}

/** The label for a tab: its title, falling back to the file name. */
export function tabLabel(tab: Tab): string {
  if (tab.title !== '') return tab.title;
  const cut = Math.max(tab.path.lastIndexOf('/'), tab.path.lastIndexOf('\\'));
  return cut === -1 ? tab.path : tab.path.slice(cut + 1);
}

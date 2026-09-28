/**
 * The folder tree's model, and the arithmetic that keeps it virtualized.
 *
 * **A 5 000-node tree must not become 5 000 components.** Svelte would survive
 * creating them; the browser would not enjoy 5 000 DOM subtrees, and every
 * expand/collapse would touch all of them. So the tree is a flat array of rows
 * and the component renders only the slice that is on screen — typically forty
 * of them — with two spacer divs standing in for the rest.
 *
 * Everything here is pure. No DOM, no Svelte, no IPC: it is the part worth
 * unit-testing, and `tests/unit/sidebar-tree.test.ts` does.
 */
import type { Entry } from '../ipc';

/**
 * Row height in pixels, shared with the component's CSS.
 *
 * Virtualization needs one number that the layout and the scroll maths agree
 * on. It is applied as an inline height rather than read back from the DOM,
 * because a measured height that disagrees with the assumed one produces a
 * tree that drifts as you scroll — the hardest kind of bug to see and the
 * easiest to introduce with a stylesheet change.
 */
export const ROW_HEIGHT = 26;

/** Rows rendered above and below the viewport, so a fast scroll is not blank. */
export const OVERSCAN = 6;

/** One rendered line of the tree. */
export interface Row {
  path: string;
  name: string;
  dir: boolean;
  depth: number;
  /** A folder with at least one child in the scan. */
  expandable: boolean;
  expanded: boolean;
}

/**
 * Flattens the scan into the rows that are currently visible.
 *
 * The scan arrives in depth-first order with parents before children, which is
 * what makes this a single pass with no tree structure in between: a collapsed
 * folder is skipped by its path prefix, and the outermost collapsed folder's
 * prefix already covers everything nested inside it.
 *
 * Folders are collapsed by default. A vault's top level is a handful of rows;
 * expanding everything on open would present five thousand and hide the shape.
 *
 * `listed` is the set of folders whose children have been fetched, and it is
 * what makes lazy loading work: a folder that nobody has opened has no
 * children *in the array*, which says nothing about the disk, so it stays
 * expandable until a listing proves it empty. Passing `null` — the default,
 * and what the full-scan path does — means "every folder has been listed", so
 * a folder with no children really is empty and gets no disclosure arrow.
 */
export function visibleRows(
  entries: readonly Entry[],
  expanded: ReadonlySet<string>,
  listed: ReadonlySet<string> | null = null
): Row[] {
  const rows: Row[] = [];
  let skip: string | null = null;

  for (let i = 0; i < entries.length; i += 1) {
    const entry = entries[i];
    if (entry === undefined) continue;

    if (skip !== null) {
      if (entry.path.startsWith(skip)) continue;
      skip = null;
    }

    const isExpanded = entry.dir && expanded.has(entry.path);
    rows.push({
      path: entry.path,
      name: entry.name,
      dir: entry.dir,
      depth: entry.depth,
      expandable: entry.dir && ((listed !== null && !listed.has(entry.path)) || hasChildren(entries, i)),
      expanded: isExpanded,
    });

    if (entry.dir && !isExpanded) skip = `${entry.path}/`;
  }

  return rows;
}

/**
 * Splices `children` into `entries` directly after `parent`.
 *
 * The flat array is required to be depth-first with parents before children —
 * `visibleRows` reads nothing else — and inserting a folder's listing at the
 * position right after it preserves exactly that, with no re-sort and no tree
 * structure in between. Which is why lazy loading needed no new model.
 */
export function spliceChildren(
  entries: readonly Entry[],
  parent: string,
  children: readonly Entry[]
): Entry[] {
  const at = entries.findIndex((e) => e.path === parent);
  if (at === -1) return [...entries, ...children];

  // Anything already under this parent is replaced, so re-listing a folder
  // after it changed on disk does not duplicate every row in it.
  let end = at + 1;
  while (end < entries.length && entries[end]!.path.startsWith(`${parent}/`)) end += 1;

  return [...entries.slice(0, at + 1), ...children, ...entries.slice(end)];
}

/**
 * Whether the entry at `i` has children.
 *
 * Depth-first order means a folder's children, if any, are the very next
 * entry — so this is one comparison rather than a search. During a streaming
 * scan the last folder in the batch may briefly look childless; the next batch
 * corrects it, which is cheaper than buffering the whole walk to be sure.
 */
function hasChildren(entries: readonly Entry[], i: number): boolean {
  const next = entries[i + 1];
  const self = entries[i];
  if (next === undefined || self === undefined) return false;
  return next.path.startsWith(`${self.path}/`);
}

/** The slice of rows to render, and the spacers that stand in for the rest. */
export interface Window {
  start: number;
  end: number;
  padTop: number;
  padBottom: number;
}

/**
 * Which rows are on screen.
 *
 * Fixed row height, so this is division rather than measurement: a
 * variable-height virtual list has to measure, cache and invalidate, and a
 * file tree has no reason to have variable rows.
 */
export function windowOf(
  count: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number = ROW_HEIGHT,
  overscan: number = OVERSCAN
): Window {
  if (count <= 0 || rowHeight <= 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };

  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const start = Math.max(0, first - overscan);
  const visible = Math.ceil(Math.max(0, viewportHeight) / rowHeight) + overscan * 2 + 1;
  const end = Math.min(count, start + visible);

  return {
    start,
    end,
    padTop: start * rowHeight,
    padBottom: Math.max(0, (count - end) * rowHeight),
  };
}

/**
 * Every folder on the way to `path`, so revealing a note opens its ancestors.
 *
 * `journal/2026/03/note.md` yields `journal`, `journal/2026`, `journal/2026/03`.
 */
export function ancestorsOf(path: string): string[] {
  const parts = path.split('/');
  const out: string[] = [];
  for (let i = 1; i < parts.length; i += 1) {
    out.push(parts.slice(0, i).join('/'));
  }
  return out;
}

/** The set with `path` toggled — expanded folders are plain data. */
export function toggle(expanded: ReadonlySet<string>, path: string): Set<string> {
  const next = new Set(expanded);
  if (!next.delete(path)) next.add(path);
  return next;
}

/** The filename without its markdown extension, for display. */
export function displayName(name: string, dir: boolean): string {
  if (dir) return name;
  const dot = name.lastIndexOf('.');
  if (dot <= 0) return name;
  const ext = name.slice(dot + 1).toLowerCase();
  return ext === 'md' || ext === 'markdown' || ext === 'mdown' || ext === 'mkd'
    ? name.slice(0, dot)
    : name;
}

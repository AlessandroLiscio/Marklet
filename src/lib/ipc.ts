/**
 * The typed edge of the Rust boundary. **Nothing else in the frontend calls
 * `invoke`.**
 *
 * One wrapper per command, so a payload change in Rust surfaces here as a type
 * error rather than as a runtime surprise three screens away. The types below
 * mirror `src-tauri/src/{render,ipc}.rs` — when one moves, the other must.
 *
 * See `.claude/skills/tauri-ipc/SKILL.md`.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** Mirrors `render::Encoding`. */
export type Encoding = 'utf8' | 'utf8-bom' | 'utf16-le' | 'utf16-be' | 'windows1252' | 'unknown';

/** Mirrors `render::Heading`. */
export interface Heading {
  level: number;
  text: string;
  slug: string;
  line: number;
}

/** Mirrors `render::Link`. */
export interface Link {
  href: string;
  text: string;
  line: number;
  wiki: boolean;
  resolved: boolean;
}

/**
 * Mirrors `render::BlockSpan`. One entry per element carrying `data-l`, in
 * document order, so `line` is non-decreasing and the array is binary-searchable.
 */
export interface BlockSpan {
  line: number;
  start_byte: number;
  end_byte: number;
}

/** Mirrors `ipc::OpenedDocument`, which flattens `render::RenderedDoc` into itself. */
export interface OpenedDocument {
  path: string;
  title: string;
  html: string;
  outline: Heading[];
  line_map: BlockSpan[];
  links: Link[];
  frontmatter: unknown | null;
  encoding: Encoding;
}

/** Mirrors `ipc::IpcError`. */
export interface IpcError {
  kind: 'not-found' | 'denied' | 'conflict' | 'io' | 'invalid';
  message: string;
  path: string | null;
}

export function isIpcError(value: unknown): value is IpcError {
  return typeof value === 'object' && value !== null && 'kind' in value && 'message' in value;
}

/** Mirrors `store::Theme`. */
export type Theme = 'light' | 'dark' | 'system';

/**
 * Mirrors `store::Typeface`. Exposed as a control only in the full edition —
 * `src/lib/settings/**` gates it behind `__MARKLET_EDITION__ === 'full'` at
 * compile time — but the field round-trips through `settings.json` in lite
 * too, so opening the same vault in both editions never clobbers a choice
 * full made.
 */
export type Typeface = 'editorial' | 'literary' | 'technical';

/** Mirrors `store::Palette`. Same full-only-control, both-editions-persist
 * reasoning as {@link Typeface}. */
export type Palette = 'default' | 'teal' | 'amber' | 'forest' | 'violet';

/** Mirrors `store::Density`. */
export type Density = 'compact' | 'normal' | 'spacious';

/** Mirrors `store::Motion`. */
export type Motion = 'on' | 'off';

/** Mirrors `store::Settings`. One field per control in `src/lib/settings/**`. */
export interface Settings {
  theme: Theme;
  /** Degrees, 0-360. Ignored while `palette` is anything but `'default'`. */
  accent_hue: number;
  typeface: Typeface;
  palette: Palette;
  density: Density;
  motion: Motion;
  /** Column width in `ch`, clamped 48-100. */
  measure: number;
}

/** Mirrors `store::ReadingPosition`. */
export interface ReadingPosition {
  line: number;
  /** Milliseconds since the Unix epoch. */
  scrolled_at: number;
}

/**
 * Reads the persisted UI preferences — theme, accent, density, motion,
 * measure, and (full edition) typeface/palette. Never rejects: a missing or
 * corrupt `settings.json` on the Rust side reads back as defaults rather
 * than failing the app open.
 */
export function readSettings(): Promise<Settings> {
  return invoke<Settings>('read_settings');
}

/** Writes every field of `settings` to `settings.json`, atomically. */
export function writeSettings(settings: Settings): Promise<void> {
  return invoke<void>('write_settings', { settings });
}

/**
 * The last recorded reading position for `path`, or `null` if it has never
 * been scrolled. `path` should be the canonicalized path `OpenedDocument`
 * already carries, so the same file is always the same key.
 */
export function readingPosition(path: string): Promise<ReadingPosition | null> {
  return invoke<ReadingPosition | null>('reading_position', { path });
}

/** Records that `path` was last scrolled to `line`, atomically. */
export function recordReadingPosition(path: string, line: number): Promise<void> {
  return invoke<void>('record_reading_position', { path, line });
}

/**
 * Opens a document and points the `marklet://` asset scheme at its directory.
 *
 * Not needed for the file the app was launched with — that one is already
 * rendered and injected as `window.__MARKLET_BOOT__` before the window exists,
 * which is what makes the first paint immediate.
 */
export function openDocument(path: string): Promise<OpenedDocument> {
  return invoke<OpenedDocument>('open_document', { path });
}

// ---------------------------------------------------------------------------
// Vault
// ---------------------------------------------------------------------------
//
// Moved here from src/lib/sidebar/vault.ts at the close of wave W4. It was
// written there because ipc.ts belonged to the main thread that wave, but a
// second module calling `invoke` defeats the reason this file exists: one
// place where a payload change in Rust becomes a type error instead of a
// runtime surprise.

/** Mirrors `vault::VaultInfo`. */
export interface VaultInfo {
  /** Absolute and canonical. Shown; never sent back. */
  root: string;
  /** The folder's own name, for the sidebar header. */
  name: string;
}

/**
 * Mirrors `vault::scan::Entry`.
 *
 * `path` is vault-relative with forward slashes on every platform — it is the
 * identity a note has in the tree, in a search hit, in the backlinks panel and
 * in the index cache. There is one spelling, and this is it.
 */
export interface Entry {
  path: string;
  name: string;
  dir: boolean;
  /** 0 for a direct child of the root. The tree indents from this. */
  depth: number;
  size: number;
  mtime_ms: number;
}

/** Mirrors `vault::scan::ScanStats`. */
export interface ScanStats {
  dirs: number;
  notes: number;
  skipped: number;
  elapsed_ms: number;
  cancelled: boolean;
}

/** Mirrors `vault::IndexStats`. */
export interface IndexStats {
  notes: number;
  parsed: number;
  cached: number;
  memory_bytes: number;
}

/** Mirrors `vault::index::NoteMeta`. */
export interface NoteMeta {
  path: string;
  title: string;
  aliases: string[];
  mtime_ms: number;
  size: number;
}

/** Mirrors `vault::index::Backlink`. */
export interface Backlink {
  path: string;
  title: string;
  line: number;
  /** The target as the referrer wrote it, `[[Note|display]]` included. */
  target: string;
}

/**
 * Mirrors `vault::search::Span`.
 *
 * The excerpt arrives pre-split rather than as byte offsets: a Rust byte offset
 * is wrong in a UTF-16 JavaScript string the moment a note contains an emoji,
 * and pre-split spans make rendering a loop over `<mark>` or not — no
 * arithmetic, no `innerHTML`.
 */
export interface Span {
  text: string;
  hit: boolean;
}

/** Mirrors `vault::search::Hit`. */
export interface Hit {
  path: string;
  /** 1-based, so it can be handed to `data-l` or to an editor unchanged. */
  line: number;
  spans: Span[];
}

/** Mirrors `vault::search::SearchStats`. */
export interface SearchStats {
  files_scanned: number;
  files_matched: number;
  hits: number;
  elapsed_ms: number;
  first_hit_ms: number;
  cancelled: boolean;
  truncated: boolean;
  /** A regex query in a lite build. The UI says so rather than showing zero. */
  regex_unavailable: boolean;
}

/** Mirrors `vault::search::Query`. */
export interface Query {
  text: string;
  regex: boolean;
  limit: number;
  lines_per_file: number;
}

/** Opens a vault and widens the `marklet://` asset root to it. */
export function openVault(path: string): Promise<VaultInfo> {
  return invoke<VaultInfo>('open_vault', { path });
}

export function closeVault(): Promise<void> {
  return invoke<void>('close_vault');
}

/**
 * Walks the vault. Entries arrive as `vault-scan-progress` events *while* this
 * promise is pending; it resolves with the totals once the walk is done.
 *
 * Subscribe before calling, or the first batches are lost — which on a small
 * vault is all of them.
 */
export function scanVault(): Promise<ScanStats> {
  return invoke<ScanStats>('scan_vault');
}

/** Builds the wiki-link and backlink index. Progress streams as events. */
export function indexVault(): Promise<IndexStats> {
  return invoke<IndexStats>('index_vault');
}

/**
 * Runs a search. Hits arrive as `search-result` events, tagged with the `id`
 * this returns through `search-done`; a newer search invalidates an older one,
 * so results whose id is not the current one must be dropped by the receiver.
 */
export function searchVault(query: Partial<Query>): Promise<SearchStats> {
  return invoke<SearchStats>('search_vault', {
    query: { text: '', regex: false, limit: 0, lines_per_file: 0, ...query },
  });
}

/** Where a `[[target]]` points, or `null` when nothing answers to that name. */
export function resolveWikilink(target: string): Promise<NoteMeta | null> {
  return invoke<NoteMeta | null>('resolve_wikilink', { target });
}

/** Every note linking to `path`. */
export function backlinksFor(path: string): Promise<Backlink[]> {
  return invoke<Backlink[]>('backlinks_for', { path });
}

/** Opens a note by its vault-relative path. */
export function openNote(path: string): Promise<unknown> {
  return invoke<unknown>('open_note', { path });
}

/** A batch of tree entries, mid-walk. */
export interface ScanProgress {
  entries: Entry[];
}

/** One streamed search hit, tagged with the search it belongs to. */
export interface SearchResult {
  id: number;
  hit: Hit;
}

/** Progress through an index build. */
export interface IndexProgress {
  done: number;
  total: number;
  cached: number;
}

export function onScanProgress(fn: (p: ScanProgress) => void): Promise<UnlistenFn> {
  return listen<ScanProgress>('vault-scan-progress', (e) => fn(e.payload));
}

export function onScanDone(fn: (s: ScanStats) => void): Promise<UnlistenFn> {
  return listen<ScanStats>('vault-scan-done', (e) => fn(e.payload));
}

export function onIndexProgress(fn: (p: IndexProgress) => void): Promise<UnlistenFn> {
  return listen<IndexProgress>('vault-index-progress', (e) => fn(e.payload));
}

export function onSearchResult(fn: (r: SearchResult) => void): Promise<UnlistenFn> {
  return listen<SearchResult>('search-result', (e) => fn(e.payload));
}

export function onSearchDone(fn: (s: SearchStats) => void): Promise<UnlistenFn> {
  return listen<SearchStats>('search-done', (e) => fn(e.payload));
}

/**
 * Subscribes, and hands back one function that undoes all of it.
 *
 * `listen` resolves to an unlisten function, so a component that subscribes to
 * five events and forgets one leaks a handler per vault it opens. Collecting
 * them here makes the cleanup a single call in one `$effect`.
 */
export function subscribeAll(
  subscriptions: Array<Promise<UnlistenFn>>
): () => void {
  let live = true;
  const unlisteners: UnlistenFn[] = [];

  for (const pending of subscriptions) {
    void pending.then((un) => {
      // The component may have been destroyed while `listen` was in flight.
      if (live) unlisteners.push(un);
      else un();
    });
  }

  return () => {
    live = false;
    for (const un of unlisteners) un();
    unlisteners.length = 0;
  };
}

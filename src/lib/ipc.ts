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

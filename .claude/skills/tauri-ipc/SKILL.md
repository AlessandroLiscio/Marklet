---
name: tauri-ipc
description: Conventions for Marklet's Rust/webview boundary — command naming, payload shapes, the error type, event names, and the path-validation rule. Read before adding or changing a #[tauri::command], before emitting a new event, and before adding a permission to the capabilities file.
---

# The IPC boundary

`src-tauri/src/ipc.rs` is the **only** module that declares `#[tauri::command]`. It stays
thin: it validates, then delegates to `render::`, `vault::`, `store::`, `export::` or
`platform::`. Business logic in a command body is a sign the logic is in the wrong module.

## Why the surface is this small

`src-tauri/capabilities/default.json` grants the frontend **no** filesystem, shell, or http
permission. That is deliberate: the app renders untrusted markdown with `innerHTML`, so an
XSS anywhere would inherit whatever the frontend can reach. Keeping the frontend unable to
touch the disk means an injected script can only call the same validated commands the UI
calls.

Never add `core:fs:*`, `shell:*` or `http:*` to the capabilities file to make something
easier. Add a narrow command instead.

## Naming

`snake_case`, verb first, no `get_` prefix on reads that are obviously reads.

```
open_document      read_settings      write_settings
splice_range       scan_vault         search_vault
resolve_wikilink   backlinks_for      export_pdf
export_html        save_pasted_image  reveal_in_editor
```

Frontend wrappers live in `src/lib/ipc.ts`, one typed function per command, and nothing
else in the frontend calls `invoke()` directly. That file is the single place where a
payload shape change shows up as a type error rather than a runtime surprise.

## Payloads

- Arguments are named, never positional tuples.
- Paths cross the boundary as strings and are **always** canonicalized on the Rust side.
- Byte offsets, never character offsets. The document is bytes; `data-l` is lines; nothing
  in between is trustworthy across a UTF-8 boundary.
- Large HTML is returned once per document load. Do not stream it in chunks — the boot-HTML
  injection path already avoids the round trip that would make streaming worth it.

## Errors

Every fallible command returns `Result<T, IpcError>`:

```rust
#[derive(serde::Serialize)]
pub struct IpcError {
    pub kind: IpcErrorKind,  // NotFound | Denied | Conflict | Io | Invalid
    pub message: String,     // human-readable, shown in the UI
    pub path: Option<String>,
}
```

`Denied` specifically means a path validation failure. Do not collapse it into `Io` — the UI
distinguishes "this file is missing" from "this file is outside the vault", and so should
the logs.

Never `unwrap()` in a command. A panic in a command takes down the webview with no message.

## Path validation is not optional

Two functions guard every path that arrives from the frontend, and they live in Rust:

- `protocol.rs` guards the `marklet://` asset scheme,
- `ipc.rs` guards command arguments.

Both canonicalize, then verify the result is inside the current document's directory or the
open vault root. `..` traversal returns `Denied`, not a silent empty result. There is a test
asserting `../../etc/passwd` is rejected.

Validation happens **after** canonicalization. Checking a raw string for `..` before
resolving symlinks is a bypass waiting to happen.

## `splice_range` — the write path

Edits never rewrite a whole file:

```rust
splice_range(path: String, start_byte: usize, end_byte: usize, replacement: String)
```

It validates the range against the file's **current** length and returns `Conflict` if the
file changed underneath — the file watcher and the editor can race, and losing a user's
external edit is unacceptable. The frontend re-reads and retries on `Conflict`.

This is what makes editing byte-exact: the bytes outside the spliced range are untouched, so
hand-aligned tables and reference-link blocks elsewhere in the file survive verbatim.

## Events

Rust to frontend, `kebab-case`:

```
file-changed        vault-scan-progress    vault-scan-done
search-result       search-done            theme-changed
```

Emit events for things the user did not ask for right now (a file changed on disk, a scan
progressed). Use a command's return value for things the user is waiting on. A command that
emits its own result as an event is harder to reason about and impossible to await.

Search results stream as `search-result` events precisely because the user should see the
first hit before the last file is read.

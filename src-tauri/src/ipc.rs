//! The Rust/webview boundary. The **only** module that declares
//! `#[tauri::command]`.
//!
//! It stays thin on purpose: validate, then delegate. Logic in a command body
//! is a sign the logic is in the wrong module.
//!
//! The frontend holds no filesystem, shell or http permission (see
//! `capabilities/default.json`), so this surface plus [`crate::protocol`] is
//! everything an injected script could reach — which is the point. Widening the
//! capabilities file to make something easier undoes that in one line.
//!
//! See `.claude/skills/tauri-ipc/SKILL.md`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::platform;
use crate::protocol::AssetRoot;
use crate::render::{self, RenderOpts, RenderedDoc};
use crate::store;
use crate::vault::{
    index::{Backlink, NoteMeta},
    scan::{Entry, ScanStats},
    search::{Hit, Query, SearchStats},
    IndexStats, VaultInfo, VaultState,
};

/// Why a command refused.
///
/// `Denied` specifically means path validation failed, and is deliberately not
/// folded into `Io`: the UI distinguishes "this file is missing" from "this
/// file is outside the vault", and so should anyone reading a log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IpcErrorKind {
    NotFound,
    Denied,
    Conflict,
    Io,
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
pub struct IpcError {
    pub kind: IpcErrorKind,
    /// Human-readable, shown in the UI as written.
    pub message: String,
    pub path: Option<String>,
}

impl IpcError {
    fn new(kind: IpcErrorKind, message: impl Into<String>, path: Option<&Path>) -> Self {
        Self {
            kind,
            message: message.into(),
            path: path.map(|p| p.display().to_string()),
        }
    }
}

/// A document plus everything the window needs to display it.
#[derive(Debug, Serialize)]
pub struct OpenedDocument {
    pub path: String,
    pub title: String,
    #[serde(flatten)]
    pub doc: RenderedDoc,
}

/// Reads a file, resolves it, and points the asset scheme at its directory.
///
/// Shared with the boot path in [`crate::run`], which needs exactly this before
/// a window exists — so it takes plain values and returns a plain struct, with
/// no `tauri` types anywhere in its signature.
pub fn open_path(
    root: &AssetRoot,
    vault: Option<&VaultState>,
    path: &Path,
) -> Result<OpenedDocument, IpcError> {
    let resolved = path.canonicalize().map_err(|e| {
        IpcError::new(
            IpcErrorKind::NotFound,
            format!("could not open the file: {e}"),
            Some(path),
        )
    })?;

    if !resolved.is_file() {
        return Err(IpcError::new(
            IpcErrorKind::Invalid,
            "that path is a directory, not a document",
            Some(&resolved),
        ));
    }

    let bytes = std::fs::read(&resolved).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not read the file: {e}"),
            Some(&resolved),
        )
    })?;

    // Images resolve relative to the document's own directory. When a vault is
    // open the root is already widened to the vault, and narrowing it here for
    // every note opened inside one would break `![](../assets/x.png)` — an
    // ordinary shape in a vault. `VaultState::open` owns that decision.
    if vault.is_none() {
        if let Some(dir) = resolved.parent() {
            root.set(dir);
        }
    }

    // Wiki-links resolve through the vault index when there is one. Without a
    // vault every `[[Note]]` is unresolved, which is the correct answer rather
    // than a degraded one: a single file has no notion of a sibling note.
    let resolver = vault.map(|v| v.resolver());
    let doc = render::render(
        &bytes,
        RenderOpts {
            asset_base: Some(AssetRoot::url_prefix().into()),
            wiki_resolver: resolver
                .as_ref()
                .map(|r| r as &dyn Fn(&str) -> Option<String>),
        },
    );

    Ok(OpenedDocument {
        title: document_title(&resolved, &doc),
        path: resolved.display().to_string(),
        doc,
    })
}

/// The window title: the document's own H1 when it has one, else the filename.
///
/// A file called `index.md` whose first heading reads "Deployment runbook" is
/// far easier to find in a taskbar under the second name.
fn document_title(path: &Path, doc: &RenderedDoc) -> String {
    doc.outline
        .iter()
        .find(|h| h.level == 1)
        .map(|h| h.text.clone())
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled")
                .to_string()
        })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------
//
// Every command below is a thin wrapper: validate, delegate, return. The bodies
// stay short so this section can be read end to end as an audit of everything a
// compromised webview could reach — which is the entire reason the subsystems
// (store, vault, watch, platform) write plain functions and this file wraps
// them, rather than each declaring its own commands where they would be
// scattered across four modules.

/// The app data directory, or an error a user can act on.
///
/// Every store command needs it and none of them should each invent their own
/// failure message for the same missing directory.
fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, IpcError> {
    app.path().app_data_dir().map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("no application data directory: {e}"),
            None,
        )
    })
}

#[tauri::command]
pub fn open_document(
    path: String,
    root: tauri::State<'_, AssetRoot>,
    vault: tauri::State<'_, VaultState>,
) -> Result<OpenedDocument, IpcError> {
    let vault_ref = vault.is_open().then_some(&*vault);
    open_path(&root, vault_ref, &PathBuf::from(path))
}

// --- settings and reading position (P3) ------------------------------------

#[tauri::command]
pub fn read_settings(app: tauri::AppHandle) -> Result<store::Settings, IpcError> {
    Ok(store::read_settings(&data_dir(&app)?))
}

#[tauri::command]
pub fn write_settings(app: tauri::AppHandle, settings: store::Settings) -> Result<(), IpcError> {
    let dir = data_dir(&app)?;
    store::write_settings(&dir, &settings).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not save settings: {e}"),
            None,
        )
    })
}

#[tauri::command]
pub fn reading_position(
    app: tauri::AppHandle,
    path: String,
) -> Result<Option<store::ReadingPosition>, IpcError> {
    Ok(store::reading_position(&data_dir(&app)?, &path))
}

#[tauri::command]
pub fn record_reading_position(
    app: tauri::AppHandle,
    path: String,
    line: usize,
) -> Result<(), IpcError> {
    let dir = data_dir(&app)?;
    // `store` takes the timestamp rather than reading the clock itself, which
    // is what makes its LRU eviction testable. The boundary is where the real
    // clock belongs.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    store::record_reading_position(&dir, &path, line, now).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not save the reading position: {e}"),
            None,
        )
    })
}

// --- vault (P5) -------------------------------------------------------------

#[tauri::command]
pub fn open_vault(
    path: String,
    root: tauri::State<'_, AssetRoot>,
    vault: tauri::State<'_, VaultState>,
) -> Result<VaultInfo, IpcError> {
    vault.open(&root, &PathBuf::from(path))
}

#[tauri::command]
pub fn close_vault(vault: tauri::State<'_, VaultState>) {
    vault.close();
}

/// Walks the vault, streaming batches of entries as it goes.
///
/// Streamed rather than returned whole because a 5,000-note vault must show a
/// tree before the walk finishes. The final stats come back as the return
/// value, so the caller can tell "still going" from "done and empty".
#[tauri::command]
pub fn scan_vault(
    window: tauri::WebviewWindow,
    vault: tauri::State<'_, VaultState>,
) -> Result<ScanStats, IpcError> {
    let stats =
        vault.scan(&mut |batch: &[Entry]| window.emit("vault-scan-progress", batch).is_ok())?;
    let _ = window.emit("vault-scan-done", &stats);
    Ok(stats)
}

#[tauri::command]
pub fn index_vault(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    vault: tauri::State<'_, VaultState>,
) -> Result<IndexStats, IpcError> {
    let _ = data_dir(&app)?;
    vault.reindex(None, &mut |progress| {
        window.emit("vault-index-progress", progress).is_ok()
    })
}

/// Streams search hits as they are found.
///
/// The generation id is the cancellation mechanism: typing another character
/// starts a new search, and the old one stops emitting as soon as it notices it
/// is no longer current. Without it a fast typist accumulates searches that all
/// keep walking the vault.
#[tauri::command]
pub fn search_vault(
    window: tauri::WebviewWindow,
    vault: tauri::State<'_, VaultState>,
    query: Query,
) -> Result<SearchStats, IpcError> {
    let id = vault.begin_search();
    let stats = vault.search(&query, &mut |hit: &Hit| {
        if !vault.search_is_current(id) {
            return false;
        }
        window.emit("search-result", (id, hit)).is_ok()
    })?;
    if vault.search_is_current(id) {
        let _ = window.emit("search-done", &stats);
    }
    Ok(stats)
}

#[tauri::command]
pub fn resolve_wikilink(vault: tauri::State<'_, VaultState>, target: String) -> Option<NoteMeta> {
    vault.resolve(&target)
}

#[tauri::command]
pub fn backlinks_for(vault: tauri::State<'_, VaultState>, note: String) -> Vec<Backlink> {
    vault.backlinks_for(&note)
}

/// Opens a note by its vault-relative path.
///
/// Separate from `open_document` because the frontend never learns absolute
/// paths for vault notes — resolution stays on this side of the boundary, which
/// is also what keeps it inside the vault root.
#[tauri::command]
pub fn open_note(
    rel: String,
    root: tauri::State<'_, AssetRoot>,
    vault: tauri::State<'_, VaultState>,
) -> Result<OpenedDocument, IpcError> {
    let path = vault.resolve_path(&rel)?;
    open_path(&root, Some(&vault), &path)
}

// --- editing (P7) -----------------------------------------------------------

/// What a successful splice leaves behind.
///
/// The new length, and nothing else: it is the client's next `expected_len`,
/// so an editor that saves twice in a row never has to re-read the file to
/// stay in step with it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Spliced {
    /// The file's length in bytes **after** the splice.
    pub len: usize,
}

/// Replaces the bytes in `[start_byte, end_byte)` of `path` with `replacement`.
///
/// **This is the entire write path, and it never rewrites a whole file.** Every
/// byte outside the range is copied through untouched, which is what makes a
/// hand-aligned table or a reference-link block elsewhere in the document
/// survive an edit verbatim — round-trip fidelity as a property of the
/// architecture rather than as a quality of a serializer. There is no
/// serializer; the document is markdown text from the moment it is read to the
/// moment it is written.
///
/// `expected_len` is the length the client believed the file had when it
/// computed the range. A mismatch means the file changed underneath — the
/// watcher and the editor genuinely race, an external `git checkout` or a
/// second editor is an ordinary event, and applying a stale range to changed
/// bytes would silently destroy someone's work. That case returns
/// [`IpcErrorKind::Conflict`] and the frontend re-reads and retries.
///
/// Plain values and a plain return, like [`open_path`]: the command below is
/// the only part that knows `tauri` exists, and this part is unit-testable
/// without a window.
pub fn splice(
    root: &AssetRoot,
    path: &Path,
    start_byte: usize,
    end_byte: usize,
    replacement: &str,
    expected_len: usize,
) -> Result<Spliced, IpcError> {
    // Canonicalize, then check containment — never the other way round. The
    // reasoning, and the shared implementation, are in `resolve_inside`.
    let resolved = resolve_inside(root, path)?;

    let bytes = std::fs::read(&resolved).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not read the file: {e}"),
            Some(&resolved),
        )
    })?;

    if bytes.len() != expected_len {
        return Err(IpcError::new(
            IpcErrorKind::Conflict,
            "the file changed on disk since it was read",
            Some(&resolved),
        ));
    }

    // A range that no longer fits is stale for the same reason a length
    // mismatch is, so it is the same answer rather than `Invalid`: the client
    // recovers from both by re-reading.
    if start_byte > end_byte || end_byte > bytes.len() {
        return Err(IpcError::new(
            IpcErrorKind::Conflict,
            "the edited range no longer fits the file",
            Some(&resolved),
        ));
    }

    // Splicing at a byte that is not a character boundary would write a file
    // that is no longer UTF-8 — the one way this command could corrupt a
    // document rather than merely refuse to change it.
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        IpcError::new(
            IpcErrorKind::Invalid,
            "that file is not UTF-8 and cannot be edited here",
            Some(&resolved),
        )
    })?;
    if !text.is_char_boundary(start_byte) || !text.is_char_boundary(end_byte) {
        return Err(IpcError::new(
            IpcErrorKind::Invalid,
            "the edited range splits a character",
            Some(&resolved),
        ));
    }

    let mut out = Vec::with_capacity(bytes.len() - (end_byte - start_byte) + replacement.len());
    out.extend_from_slice(&bytes[..start_byte]);
    out.extend_from_slice(replacement.as_bytes());
    out.extend_from_slice(&bytes[end_byte..]);
    let len = out.len();

    // Temp file in the same directory, then rename over the target — the same
    // reasoning as `store::write_json_atomic`, applied to something far less
    // replaceable than a preferences file: a process killed between `create`
    // and the last `write` would otherwise leave the user's document
    // truncated.
    let name = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.md");
    let tmp = resolved.with_file_name(format!("{name}.marklet-tmp-{}", std::process::id()));
    std::fs::write(&tmp, &out).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not write the file: {e}"),
            Some(&resolved),
        )
    })?;
    if let Err(e) = std::fs::rename(&tmp, &resolved) {
        // Best effort, and deliberately ignored: reporting the rename failure
        // matters more than reporting a failure to clean up after it.
        let _ = std::fs::remove_file(&tmp);
        return Err(IpcError::new(
            IpcErrorKind::Io,
            format!("could not save the file: {e}"),
            Some(&resolved),
        ));
    }

    Ok(Spliced { len })
}

/// `snake_case` arguments so the payload spells the command exactly as
/// `.claude/skills/tauri-ipc/SKILL.md` documents it, rather than relying on
/// Tauri's camelCase conversion to be remembered correctly at the one call
/// site in `src/lib/ipc.ts`.
#[tauri::command(rename_all = "snake_case")]
pub fn splice_range(
    path: String,
    start_byte: usize,
    end_byte: usize,
    replacement: String,
    expected_len: usize,
    root: tauri::State<'_, AssetRoot>,
) -> Result<Spliced, IpcError> {
    splice(
        &root,
        &PathBuf::from(path),
        start_byte,
        end_byte,
        &replacement,
        expected_len,
    )
}

// --- reading, pasting, revealing, exporting (W5 close) -----------------------

/// Canonicalizes `path` and refuses anything outside the asset root.
///
/// The order is canonicalize-then-check, the same as [`crate::protocol::resolve`]
/// and for the same reason: screening the raw string first is a bypass waiting
/// for a symlink. The allowed root is whatever the asset scheme is already
/// serving — the open document's own directory, or the vault root when one is
/// open — so nothing here can reach a file the webview could not already read
/// through `marklet://`.
fn resolve_inside(root: &AssetRoot, path: &Path) -> Result<PathBuf, IpcError> {
    let resolved = path.canonicalize().map_err(|e| {
        IpcError::new(
            IpcErrorKind::NotFound,
            format!("could not open the file: {e}"),
            Some(path),
        )
    })?;

    if !resolved.is_file() {
        return Err(IpcError::new(
            IpcErrorKind::Invalid,
            "that path is a directory, not a document",
            Some(&resolved),
        ));
    }

    let base = root.get().ok_or_else(|| {
        IpcError::new(
            IpcErrorKind::Denied,
            "no document is open, so there is nothing to write to",
            Some(&resolved),
        )
    })?;
    if !resolved.starts_with(&base) {
        return Err(IpcError::new(
            IpcErrorKind::Denied,
            "that file is outside the open document's folder",
            Some(&resolved),
        ));
    }

    Ok(resolved)
}

/// The markdown source of an open document, byte-for-byte.
///
/// Not [`crate::render::decode`]: the editor's whole guarantee is that what it
/// writes back through [`splice`] differs from what it read only where the user
/// typed. Handing it a *re-encoded* copy of a Windows-1252 or UTF-16 file would
/// make the first save rewrite the entire document into UTF-8 — a change nobody
/// asked for, in a file they may share with a tool that cannot read it. So a
/// file that is not already UTF-8 is not editable, and says so, rather than
/// being silently converted.
#[tauri::command]
pub fn read_source(path: String, root: tauri::State<'_, AssetRoot>) -> Result<String, IpcError> {
    let resolved = resolve_inside(&root, &PathBuf::from(path))?;
    let bytes = std::fs::read(&resolved).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not read the file: {e}"),
            Some(&resolved),
        )
    })?;

    String::from_utf8(bytes).map_err(|_| {
        IpcError::new(
            IpcErrorKind::Invalid,
            "this file is not UTF-8, so it can be read but not edited",
            Some(&resolved),
        )
    })
}

/// The image formats a paste is allowed to produce, and their extensions.
///
/// Mirrors `IMAGE_TYPES` in `src/lib/edit/paste.ts`. An allowlist rather than
/// "whatever extension the webview sent": the extension becomes a real filename
/// on disk, and `ext` arriving as `..\\..\\autorun` or `php` must not be the
/// thing that decides it.
const PASTE_EXTENSIONS: &[&str] = &["png", "jpg", "gif", "webp", "svg", "avif"];

/// Where pasted images go, relative to the document. Mirrors `ASSET_DIR`.
const ASSET_DIR: &str = "assets";

/// The document's own name reduced to something safe on NTFS and readable
/// everywhere. The same shape as the render core's heading slug, and the same
/// rules as `documentSlug()` in `src/lib/edit/paste.ts`.
fn document_slug(stem: &str) -> String {
    let mut slug = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "image".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The first index not already taken under `slug` in `dir`.
///
/// Counts past the highest rather than filling the first gap: a deleted
/// `notes-2.png` must not have its name handed to a different picture, because
/// another note may still link to it and would then silently show the wrong
/// image.
fn next_asset_index(dir: &Path, slug: &str) -> usize {
    let mut highest = 0usize;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 1;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let lower = name.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix(&format!("{slug}-")) else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
            continue;
        }
        if let Ok(n) = digits.parse::<usize>() {
            highest = highest.max(n);
        }
    }
    highest + 1
}

/// Writes clipboard image bytes beside the document and returns the relative
/// markdown-ready path.
///
/// `snake_case` for the same reason [`splice_range`] uses it: the payload spells
/// the command the way `.claude/skills/tauri-ipc/SKILL.md` documents it.
#[tauri::command(rename_all = "snake_case")]
pub fn save_pasted_image(
    doc_path: String,
    bytes: Vec<u8>,
    ext: String,
    root: tauri::State<'_, AssetRoot>,
) -> Result<String, IpcError> {
    let ext = ext.to_ascii_lowercase();
    if !PASTE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(IpcError::new(
            IpcErrorKind::Invalid,
            format!("{ext} is not an image format Marklet saves"),
            None,
        ));
    }

    let resolved = resolve_inside(&root, &PathBuf::from(doc_path))?;
    let parent = resolved.parent().ok_or_else(|| {
        IpcError::new(
            IpcErrorKind::Invalid,
            "that document has no folder to save an image into",
            Some(&resolved),
        )
    })?;

    let dir = parent.join(ASSET_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not create the assets folder: {e}"),
            Some(&dir),
        )
    })?;

    let slug = document_slug(
        resolved
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image"),
    );
    let name = format!("{slug}-{}.{ext}", next_asset_index(&dir, &slug));
    let target = dir.join(&name);

    std::fs::write(&target, &bytes).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not save the image: {e}"),
            Some(&target),
        )
    })?;

    // Forward slashes on every platform: the result goes into markdown, where a
    // backslash is an escape character, not a separator.
    Ok(format!("{ASSET_DIR}/{name}"))
}

/// `MD_EDITOR` as `cli::parse` read it, held so [`reveal_in_editor`] can decide
/// which program to run without the webview naming one.
pub struct EditorSetting(pub Option<String>);

/// Opens the document in the user's editor at `line`:`column`.
///
/// **The webview names no program.** It says where the cursor is; everything
/// else — which editor, which arguments, in which order — is decided by
/// [`crate::editor`] from `MD_EDITOR` and a fixed table. See that module's doc
/// comment for why this is not a `(program, args)` command.
#[tauri::command]
pub fn reveal_in_editor(
    path: String,
    line: usize,
    column: usize,
    root: tauri::State<'_, AssetRoot>,
    editor: tauri::State<'_, EditorSetting>,
) -> Result<String, IpcError> {
    let resolved = resolve_inside(&root, &PathBuf::from(path))?;
    let candidates =
        crate::editor::candidates_for(editor.0.as_deref(), &resolved, line.max(1), column.max(1));

    crate::editor::reveal(&candidates).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("no editor started: {e}"),
            Some(&resolved),
        )
    })
}

/// The file an export writes, derived from the document rather than chosen in a
/// dialog.
///
/// No `tauri-plugin-dialog`: a native save dialog is roughly 300 KB of plugin
/// for a decision the user almost always makes the same way, and "it is next to
/// the note, named after the note" needs no explaining. See docs/editions.md.
fn export_target(doc: &Path, extension: &str) -> Result<PathBuf, IpcError> {
    let stem = doc.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
        IpcError::new(
            IpcErrorKind::Invalid,
            "that document has no name to base an export on",
            Some(doc),
        )
    })?;
    let parent = doc.parent().ok_or_else(|| {
        IpcError::new(
            IpcErrorKind::Invalid,
            "that document has no folder to export into",
            Some(doc),
        )
    })?;
    Ok(parent.join(format!("{stem}.{extension}")))
}

/// Prints the live, already-enriched document to a PDF beside it.
///
/// `async` deliberately: a synchronous Tauri command runs on the main thread,
/// and both platform print calls below must run *on* the webview's thread while
/// this one waits for them. Blocking the main thread to wait for work scheduled
/// onto the main thread is a deadlock; an async command runs on the async
/// runtime instead, so the wait is safe.
#[tauri::command]
pub async fn export_pdf(
    window: tauri::WebviewWindow,
    path: String,
    root: tauri::State<'_, AssetRoot>,
) -> Result<String, IpcError> {
    let resolved = resolve_inside(&root, &PathBuf::from(path))?;
    let target = export_target(&resolved, "pdf")?;

    let (tx, rx) = std::sync::mpsc::channel();
    let output = target.clone();
    window
        .with_webview(move |webview| {
            let _ = tx.send(print_platform_pdf(&webview, &output));
        })
        .map_err(|e| {
            IpcError::new(
                IpcErrorKind::Io,
                format!("could not reach the webview to print: {e}"),
                Some(&target),
            )
        })?;

    rx.recv()
        .map_err(|_| {
            IpcError::new(
                IpcErrorKind::Io,
                "the print call ended without reporting a result",
                Some(&target),
            )
        })?
        .map_err(|e| IpcError::new(IpcErrorKind::Io, e.to_string(), Some(&target)))?;

    Ok(target.display().to_string())
}

/// The one place a platform webview type is named outside [`crate::export::pdf`].
///
/// Both arms are a single call. Everything that could go wrong in a way worth
/// reading lives in `export/pdf.rs`, which `./scripts/check-windows.sh`
/// type-checks from Linux; keeping this wrapper to one line per platform is
/// what keeps the part that *cannot* be checked here trivially small.
#[cfg(windows)]
fn print_platform_pdf(
    webview: &tauri::webview::PlatformWebview,
    output: &Path,
) -> Result<(), crate::export::pdf::PdfError> {
    crate::export::pdf::windows::print_controller_to_pdf(&webview.controller(), output)
}

#[cfg(target_os = "linux")]
fn print_platform_pdf(
    webview: &tauri::webview::PlatformWebview,
    output: &Path,
) -> Result<(), crate::export::pdf::PdfError> {
    crate::export::pdf::linux::print_to_pdf(&webview.inner(), output)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn print_platform_pdf(
    _webview: &tauri::webview::PlatformWebview,
    _output: &Path,
) -> Result<(), crate::export::pdf::PdfError> {
    crate::export::pdf::print_to_pdf()
}

/// Writes the live, already-enriched document as one standalone HTML file
/// beside it.
///
/// The body arrives from the webview because that is the only place KaTeX and
/// Mermaid have run — `export_file` in `export/html.rs` renders the same
/// document without them, which is what `MD_HTML=1` gets and why the two paths
/// exist separately. Images are inlined here, in Rust, against the document's
/// own directory: the webview cannot read the filesystem and must not start.
#[tauri::command(rename_all = "snake_case")]
pub fn export_html(
    path: String,
    body_html: String,
    extra_css: String,
    root: tauri::State<'_, AssetRoot>,
) -> Result<String, IpcError> {
    let resolved = resolve_inside(&root, &PathBuf::from(path))?;
    let target = export_target(&resolved, "html")?;

    let title = resolved
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Marklet document");

    let document =
        crate::export::html::assemble_standalone(title, &body_html, &extra_css, resolved.parent());

    std::fs::write(&target, document).map_err(|e| {
        IpcError::new(
            IpcErrorKind::Io,
            format!("could not write the export: {e}"),
            Some(&target),
        )
    })?;

    Ok(target.display().to_string())
}
/// Opens `url` in the user's browser.
///
/// **A link is not navigation.** Without this, clicking an `http` link in a
/// document navigates the *application's own webview* to that page: the app is
/// replaced by a website, with no address bar and no back button, and the
/// document the user was reading is gone. That is also the shape of an attack
/// — a markdown file that quietly replaces the app with a page that looks like
/// it — so the frontend intercepts every external link and sends it here
/// instead.
///
/// Only `http` and `https`. The scheme allowlist is the whole security of this
/// command: the handler on the other side is the operating system's, and on
/// Windows a registered protocol handler can be an arbitrary program with
/// arbitrary arguments. `file:` would open Explorer at a path of the
/// document's choosing; `ms-msdt:` was a remote-code-execution chain for
/// years. Neither the webview nor a markdown file gets to pick which handler
/// runs, so anything that is not plain web browsing is refused here.
///
/// The URL is *not* otherwise inspected. Nothing is shelled out — see
/// `platform::open_external` — so there is no metacharacter to escape and no
/// second parser to disagree with the first.
#[tauri::command]
pub fn open_external(url: String) -> Result<String, IpcError> {
    let scheme = url
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase());

    match scheme.as_deref() {
        Some("http") | Some("https") => {}
        _ => {
            return Err(IpcError::new(
                IpcErrorKind::Denied,
                "only http and https links can be opened",
                None,
            ))
        }
    }

    platform::open_external(&url)
        .map(str::to_string)
        .map_err(|e| {
            IpcError::new(
                IpcErrorKind::Io,
                format!("could not open that link: {e}"),
                None,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sandbox(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("marklet-ipc-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn opens_a_document_and_titles_it_from_its_h1() {
        let path = sandbox("titled.md", "# Deployment runbook\n\nBody.\n");
        let opened = open_path(&AssetRoot::new(), None, &path).unwrap();
        assert_eq!(opened.title, "Deployment runbook");
        assert!(opened.doc.html.contains("Deployment runbook"));
    }

    #[test]
    fn falls_back_to_the_filename_without_an_h1() {
        let path = sandbox("plain.md", "Just a paragraph.\n");
        let opened = open_path(&AssetRoot::new(), None, &path).unwrap();
        assert_eq!(opened.title, "plain.md");
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let err =
            open_path(&AssetRoot::new(), None, Path::new("/definitely/absent.md")).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::NotFound);
        assert!(err.path.is_some(), "the error names the path it refused");
    }

    #[test]
    fn a_directory_is_invalid_rather_than_io() {
        let path = sandbox("x.md", "x");
        let dir = path.parent().unwrap();
        let err = open_path(&AssetRoot::new(), None, dir).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Invalid);
    }

    #[test]
    fn opening_a_document_points_the_asset_root_at_its_directory() {
        let path = sandbox("assets.md", "![x](./x.png)\n");
        let root = AssetRoot::new();
        open_path(&root, None, &path).unwrap();
        assert_eq!(
            root.get().unwrap(),
            path.parent().unwrap().canonicalize().unwrap()
        );
    }
}

#[cfg(test)]
mod splice_tests {
    use super::*;
    use std::fs;

    /// A sandbox per test name, with the asset root pointed at it — the state
    /// the app is in whenever a document is open, and the only state in which
    /// `splice` will write anything at all.
    fn sandbox(name: &str, body: &str) -> (AssetRoot, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("marklet-splice-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.md");
        fs::write(&path, body).unwrap();

        let root = AssetRoot::new();
        root.set(&dir);
        (root, path)
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    /// The hand-aligned table from `tests/fixtures/table-aligned.md`'s shape:
    /// padding a human typed by eye, which no serializer would reproduce.
    const ALIGNED: &str = "\
# Report

| Component  | Installer | Loaded when     |
|:-----------|----------:|:---------------:|
| mermaid    |    750 KB | has a diagram   |
| katex      |    200 KB | has math        |
| codemirror |    180 KB | F2 / F3 / Ctrl+E|

Trailing paragraph, untouched.
";

    #[test]
    fn splices_a_range_and_reports_the_new_length() {
        let (root, path) = sandbox("basic", "hello world\n");
        let start = "hello ".len();
        let end = "hello world".len();

        let out = splice(&root, &path, start, end, "there", "hello world\n".len()).unwrap();

        assert_eq!(read(&path), "hello there\n");
        assert_eq!(out.len, "hello there\n".len());
    }

    /// The acceptance criterion, proved by diffing rather than by reading:
    /// editing ONE cell of a hand-aligned table leaves every other line of the
    /// file byte-identical. This is the whole reason there is no markdown
    /// serializer in this codebase.
    #[test]
    fn editing_one_table_cell_leaves_every_other_line_byte_identical() {
        let (root, path) = sandbox("aligned", ALIGNED);

        // The `200 KB` cell of the katex row, located by byte offset the way
        // the frontend locates it — from the source text, not from the DOM.
        let cell = "    200 KB";
        let start = ALIGNED.find(cell).unwrap();
        let end = start + cell.len();

        splice(&root, &path, start, end, "     63 KB", ALIGNED.len()).unwrap();

        let after = read(&path);
        let before_lines: Vec<&str> = ALIGNED.lines().collect();
        let after_lines: Vec<&str> = after.lines().collect();

        assert_eq!(
            before_lines.len(),
            after_lines.len(),
            "the splice must not add or remove a line"
        );

        let mut changed = Vec::new();
        for (i, (b, a)) in before_lines.iter().zip(after_lines.iter()).enumerate() {
            if b != a {
                changed.push(i);
            }
        }

        assert_eq!(changed.len(), 1, "exactly one line may differ: {changed:?}");
        assert_eq!(
            after_lines[changed[0]],
            "| katex      |     63 KB | has math        |"
        );

        // And the alignment the author typed by hand is still there, column
        // for column, on every row the edit did not touch.
        assert!(after.contains("| mermaid    |    750 KB | has a diagram   |"));
        assert!(after.contains("| codemirror |    180 KB | F2 / F3 / Ctrl+E|"));
        assert!(after.ends_with("Trailing paragraph, untouched.\n"));
    }

    #[test]
    fn an_insertion_is_a_zero_width_range() {
        let (root, path) = sandbox("insert", "ab\n");
        splice(&root, &path, 1, 1, "X", 3).unwrap();
        assert_eq!(read(&path), "aXb\n");
    }

    #[test]
    fn a_deletion_is_an_empty_replacement() {
        let (root, path) = sandbox("delete", "abc\n");
        splice(&root, &path, 1, 2, "", 4).unwrap();
        assert_eq!(read(&path), "ac\n");
    }

    #[test]
    fn a_file_that_changed_since_it_was_read_is_a_conflict() {
        let (root, path) = sandbox("conflict", "hello world\n");

        // Somebody else — a `git checkout`, a second editor, the watcher's own
        // source of truth — wrote the file after the frontend read it.
        fs::write(&path, "hello brave new world\n").unwrap();

        let err = splice(&root, &path, 6, 11, "there", "hello world\n".len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Conflict);
        assert_eq!(
            read(&path),
            "hello brave new world\n",
            "a conflict must not write anything at all"
        );
    }

    #[test]
    fn a_range_past_the_end_of_the_file_is_a_conflict() {
        let (root, path) = sandbox("range", "short\n");
        let err = splice(&root, &path, 0, 999, "x", "short\n".len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Conflict);
    }

    #[test]
    fn an_inverted_range_is_a_conflict() {
        let (root, path) = sandbox("inverted", "abcdef\n");
        let err = splice(&root, &path, 4, 2, "x", "abcdef\n".len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Conflict);
    }

    #[test]
    fn splitting_a_multibyte_character_is_refused() {
        // "é" is two bytes. Offset 1 is inside it, and writing there would
        // produce a file that is no longer UTF-8.
        let body = "café\n";
        let (root, path) = sandbox("boundary", body);
        let err = splice(&root, &path, 4, 4, "x", body.len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Invalid);
        assert_eq!(read(&path), body);
    }

    #[test]
    fn a_file_outside_the_open_document_folder_is_denied() {
        let (root, _path) = sandbox("denied-root", "inside\n");

        let outside_dir =
            std::env::temp_dir().join(format!("marklet-splice-outside-{}", std::process::id()));
        fs::create_dir_all(&outside_dir).unwrap();
        let outside = outside_dir.join("secret.md");
        fs::write(&outside, "not yours\n").unwrap();

        let err = splice(&root, &outside, 0, 0, "x", "not yours\n".len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Denied);
        assert_eq!(read(&outside), "not yours\n");
    }

    #[test]
    fn traversal_out_of_the_open_folder_is_denied() {
        let (root, path) = sandbox("denied-traversal", "inside\n");
        // A path the frontend could plausibly send: relative traversal out of
        // the folder the asset root allows, aimed at a file that really exists
        // — so canonicalization SUCCEEDS and the containment check is what
        // refuses it. A target that did not exist would 404 before ever
        // reaching that branch and would prove nothing.
        let escape = path.parent().unwrap().join("../marklet-splice-escape.md");
        fs::write(&escape, "outside\n").unwrap();

        let err = splice(&root, &escape, 0, 0, "x", "outside\n".len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Denied);
    }

    #[test]
    fn writing_with_no_document_open_is_denied() {
        let (_root, path) = sandbox("no-root", "hello\n");
        let empty = AssetRoot::new();
        let err = splice(&empty, &path, 0, 0, "x", "hello\n".len()).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Denied);
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let (root, path) = sandbox("missing", "x\n");
        let absent = path.with_file_name("absent.md");
        let err = splice(&root, &absent, 0, 0, "x", 0).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::NotFound);
    }

    #[test]
    fn no_temp_file_is_left_behind() {
        let (root, path) = sandbox("tmp", "hello\n");
        splice(&root, &path, 0, 5, "howdy", 6).unwrap();

        let leftovers: Vec<String> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".marklet-tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }
}

#[cfg(test)]
mod paste_and_export_tests {
    use super::*;

    /// The naming half of the paste path, which has no webview in it and so is
    /// testable here. Its mirror is `documentSlug()` / `nextAssetIndex()` in
    /// `src/lib/edit/paste.ts` — which deliberately no longer has one: naming
    /// the file is a privileged decision and lives on this side of the boundary
    /// alone, so these tests are the only place the rules are stated.
    #[test]
    fn a_slug_is_lowercase_hyphenated_and_never_empty() {
        assert_eq!(document_slug("My Notes"), "my-notes");
        assert_eq!(document_slug("a__b--c"), "a-b-c");
        assert_eq!(document_slug("  spaced  "), "spaced");
        assert_eq!(
            document_slug("日本語"),
            "image",
            "a name with no ASCII still needs a filename"
        );
        assert_eq!(document_slug(""), "image");
    }

    #[test]
    fn indexes_count_past_the_highest_rather_than_filling_gaps() {
        let dir = std::env::temp_dir().join(format!("marklet-assets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            next_asset_index(&dir, "note"),
            1,
            "an empty folder starts at 1"
        );

        std::fs::write(dir.join("note-1.png"), b"x").unwrap();
        std::fs::write(dir.join("note-3.png"), b"x").unwrap();
        // The gap at 2 is deliberate: another note may still link to the image
        // that used to be there, and handing its name to a different picture
        // would make that link show the wrong thing rather than nothing.
        assert_eq!(next_asset_index(&dir, "note"), 4);

        std::fs::write(dir.join("other-9.png"), b"x").unwrap();
        std::fs::write(dir.join("note-notanumber.png"), b"x").unwrap();
        assert_eq!(
            next_asset_index(&dir, "note"),
            4,
            "another slug is not this one"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_export_lands_beside_the_document_with_its_name() {
        let target = export_target(Path::new("/vault/notes/Release Plan.md"), "pdf").unwrap();
        assert_eq!(target, PathBuf::from("/vault/notes/Release Plan.pdf"));
    }

    #[test]
    fn only_image_extensions_are_accepted() {
        // The extension becomes a real filename, so it is an allowlist rather
        // than whatever string the webview happened to send.
        for ok in PASTE_EXTENSIONS {
            assert!(PASTE_EXTENSIONS.contains(ok));
        }
        for bad in ["php", "exe", "", "..", "png/../x"] {
            assert!(
                !PASTE_EXTENSIONS.contains(&bad),
                "{bad} must not be writable"
            );
        }
    }
}

#[cfg(test)]
mod external_link_tests {
    use super::*;

    /// The scheme allowlist is the entire security of `open_external`: whatever
    /// survives it is handed to the operating system's registered handler,
    /// which on Windows can be an arbitrary program. These cases are the ones
    /// that must never get that far.
    #[test]
    fn only_http_and_https_are_openable() {
        for refused in [
            "file:///etc/passwd",
            "file://C:/Windows/System32/calc.exe",
            "ms-msdt:/id PCWDiagnostic",
            "javascript://example.com/%0aalert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox(1)",
            "smb://attacker/share",
            "marklet://internal",
            "not a url at all",
            "",
        ] {
            let err = open_external(refused.to_string()).unwrap_err();
            assert_eq!(
                err.kind,
                IpcErrorKind::Denied,
                "{refused} must be refused, not attempted"
            );
        }
    }

    #[test]
    fn the_scheme_check_ignores_case_but_not_position() {
        // Upper-case schemes are legal and common in pasted URLs.
        assert!(!matches!(
            open_external("HTTPS://example.com".into()),
            Err(e) if e.kind == IpcErrorKind::Denied
        ));
        // A permitted scheme appearing later in the string is not a scheme.
        assert_eq!(
            open_external("file:///x?u=https://example.com".into())
                .unwrap_err()
                .kind,
            IpcErrorKind::Denied
        );
    }
}

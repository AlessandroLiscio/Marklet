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

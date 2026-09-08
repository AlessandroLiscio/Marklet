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

use crate::protocol::AssetRoot;
use crate::render::{self, RenderOpts, RenderedDoc};

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
pub fn open_path(root: &AssetRoot, path: &Path) -> Result<OpenedDocument, IpcError> {
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

    // Images resolve relative to the document's own directory until a vault is
    // open. Phase P5 widens this to the vault root.
    if let Some(dir) = resolved.parent() {
        root.set(dir);
    }

    let doc = render::render(
        &bytes,
        RenderOpts {
            asset_base: Some(AssetRoot::url_prefix().into()),
            ..RenderOpts::default()
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

#[tauri::command]
pub fn open_document(
    path: String,
    root: tauri::State<'_, AssetRoot>,
) -> Result<OpenedDocument, IpcError> {
    open_path(&root, &PathBuf::from(path))
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
        let opened = open_path(&AssetRoot::new(), &path).unwrap();
        assert_eq!(opened.title, "Deployment runbook");
        assert!(opened.doc.html.contains("Deployment runbook"));
    }

    #[test]
    fn falls_back_to_the_filename_without_an_h1() {
        let path = sandbox("plain.md", "Just a paragraph.\n");
        let opened = open_path(&AssetRoot::new(), &path).unwrap();
        assert_eq!(opened.title, "plain.md");
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let err = open_path(&AssetRoot::new(), Path::new("/definitely/absent.md")).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::NotFound);
        assert!(err.path.is_some(), "the error names the path it refused");
    }

    #[test]
    fn a_directory_is_invalid_rather_than_io() {
        let path = sandbox("x.md", "x");
        let dir = path.parent().unwrap();
        let err = open_path(&AssetRoot::new(), dir).unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Invalid);
    }

    #[test]
    fn opening_a_document_points_the_asset_root_at_its_directory() {
        let path = sandbox("assets.md", "![x](./x.png)\n");
        let root = AssetRoot::new();
        open_path(&root, &path).unwrap();
        assert_eq!(
            root.get().unwrap(),
            path.parent().unwrap().canonicalize().unwrap()
        );
    }
}

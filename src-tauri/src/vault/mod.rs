//! The vault: a folder of notes, the links between them, and search across all
//! of it.
//!
//! This is the feature that makes Marklet a different kind of tool from a
//! Markdown *viewer*, and it is deliberately built out of three small things
//! rather than one large one:
//!
//! - [`scan`] walks the folder, **streaming** — a tree paints before the walk
//!   ends,
//! - [`index`] keeps titles, aliases and the wiki-link graph, so `[[Note]]`
//!   resolves and a backlinks panel is a lookup rather than a scan,
//! - [`search`] reads files and looks for literal terms, with **no index at
//!   all**.
//!
//! ## Why there is no search index
//!
//! An index has to be built, kept in sync, invalidated, stored, and repaired
//! when it corrupts. Every one of those is a bug that reports itself as "search
//! did not find a note that is right there". Reading 5 000 files the page cache
//! already holds and running `aho-corasick` over them answers the same question
//! in 60-150 ms with none of those states — and the first result streams to the
//! UI long before the last file is read. `tantivy` would cost 4-6 MB to solve a
//! problem we do not have, and is denied by name in `deny.toml`. A JavaScript
//! index would be small on disk and pull the whole vault into webview memory,
//! which is a worse problem wearing a smaller number.
//!
//! ## No `#[tauri::command]` here
//!
//! `ipc.rs` is the one place the whole privileged surface can be audited, so
//! everything below is a plain `pub fn` taking plain values and callbacks. The
//! callbacks are what let `ipc.rs` turn a streaming scan into
//! `vault-scan-progress` events without this module knowing what an event is.
//!
//! ## The vault root widens the asset root
//!
//! Until a vault is open, `marklet://` serves the open document's own
//! directory. [`VaultState::open`] widens it to the vault root, so a note in
//! `journal/2026/` can reference `../../assets/diagram.png` the way every vault
//! tool lets it. That is the *only* widening; the canonicalize-then-check in
//! `protocol.rs` is unchanged and still the thing that refuses everything else.

pub mod index;
pub mod scan;
pub mod search;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub use index::{Backlink, Index, IndexProgress, NoteMeta, WikiRef};
pub use scan::{Entry, ScanStats};
pub use search::{Hit, Query, SearchStats, Span};

use crate::ipc::{IpcError, IpcErrorKind};
use crate::protocol::AssetRoot;

/// Builds an [`IpcError`]. `ipc.rs` keeps its own constructor private, and the
/// vault has no business inventing a second error vocabulary for the same UI.
pub(crate) fn err(kind: IpcErrorKind, message: impl Into<String>, path: Option<&Path>) -> IpcError {
    IpcError {
        kind,
        message: message.into(),
        path: path.map(|p| p.display().to_string()),
    }
}

/// What opening a vault tells the frontend, before any scanning has happened.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultInfo {
    /// Absolute, canonical. The frontend shows it and passes nothing back but
    /// vault-relative paths.
    pub root: String,
    /// The folder's own name, for the sidebar header.
    pub name: String,
}

/// The open vault, or none.
///
/// One `RwLock` rather than a lock per map: every reader takes it for a few
/// microseconds (a wiki-link resolution is one hash lookup) and the only writer
/// is a rebuild. Splitting it would buy contention that does not exist and cost
/// the guarantee that the index and the root always describe the same folder.
#[derive(Default, Clone)]
pub struct VaultState {
    open: Arc<RwLock<Option<Open>>>,
    /// Bumped by [`VaultState::begin_search`]. A running search checks it and
    /// stops when it is no longer the current one, which is how the next
    /// keystroke cancels the previous query instead of racing it to the
    /// sidebar. It lives here rather than in `ipc.rs` because "a newer search
    /// wins" is a property of searching, not of the IPC boundary.
    searches: Arc<AtomicU64>,
}

/// The contents of an open vault.
pub struct Open {
    pub root: PathBuf,
    pub index: Index,
}

impl VaultState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a vault: canonicalizes the root, points `marklet://` at it, and
    /// installs an **empty** index.
    ///
    /// Deliberately fast — it does no walking. The scan and the index build are
    /// separate calls precisely so the sidebar can paint a tree while they run;
    /// doing them here would mean a command that takes half a second to return
    /// and a window that looks frozen for all of it.
    pub fn open(&self, assets: &AssetRoot, root: &Path) -> Result<VaultInfo, IpcError> {
        let root = scan::canonical_root(root)?;

        // The widening. `protocol.rs` is untouched: it still canonicalizes and
        // still refuses anything that resolves outside whatever root it holds.
        assets.set(&root);

        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("vault")
            .to_string();
        let info = VaultInfo {
            root: root.display().to_string(),
            name,
        };

        if let Ok(mut guard) = self.open.write() {
            *guard = Some(Open {
                index: Index::default(),
                root,
            });
        }

        Ok(info)
    }

    /// Forgets the vault. `marklet://` is *not* narrowed back here — the
    /// document that is still open decides what it serves, and that is
    /// `ipc.rs`'s call to make when it reopens one.
    pub fn close(&self) {
        if let Ok(mut guard) = self.open.write() {
            *guard = None;
        }
    }

    pub fn root(&self) -> Option<PathBuf> {
        self.open.read().ok()?.as_ref().map(|v| v.root.clone())
    }

    pub fn is_open(&self) -> bool {
        self.open.read().is_ok_and(|g| g.is_some())
    }

    /// Streams the folder tree. See [`scan::walk`].
    pub fn scan(&self, on_batch: &mut dyn FnMut(&[Entry]) -> bool) -> Result<ScanStats, IpcError> {
        let root = self.require_root()?;
        Ok(scan::walk(&root, scan::BATCH, on_batch))
    }

    /// Builds (or rebuilds) the wiki-link index, reusing the cache in
    /// `cache_dir` when it is still valid.
    ///
    /// The build happens outside the lock and is swapped in at the end: a
    /// rebuild of a 5 000-note vault must not block a wiki-link resolution
    /// that is one hash lookup.
    pub fn reindex(
        &self,
        cache_dir: Option<&Path>,
        on_progress: &mut dyn FnMut(IndexProgress) -> bool,
    ) -> Result<IndexStats, IpcError> {
        let root = self.require_root()?;
        let index = Index::build(&root, cache_dir, on_progress);
        let stats = IndexStats {
            notes: index.len(),
            parsed: index.parsed,
            cached: index.cached,
            memory_bytes: index.memory_bytes(),
        };

        if let Ok(mut guard) = self.open.write() {
            if let Some(open) = guard.as_mut() {
                // Only if it is still the same vault: the user may have opened
                // another one while this build was running, and installing a
                // stale index over it would resolve links into the wrong folder.
                if open.root == root {
                    open.index = index;
                }
            }
        }

        Ok(stats)
    }

    /// Re-reads one note after a save or a rename. What the watcher calls.
    pub fn note_changed(&self, rel: &str) -> Result<(), IpcError> {
        let root = self.require_root()?;
        let meta = index::read_note_at(&root, rel);
        if let Ok(mut guard) = self.open.write() {
            if let Some(open) = guard.as_mut() {
                open.index.update(rel, meta);
            }
        }
        Ok(())
    }

    /// The `RenderOpts.wiki_resolver` callback.
    ///
    /// Handed to `render::render` as `Some(&state.resolver())`. With no vault
    /// open every target is unresolved, which is exactly right: a loose file on
    /// the desktop has no vault to resolve against, and an unresolved wiki-link
    /// is a normal rendered state rather than an error.
    pub fn resolver(&self) -> impl Fn(&str) -> Option<String> + '_ {
        move |target: &str| {
            let guard = self.open.read().ok()?;
            guard.as_ref()?.index.resolve_href(target)
        }
    }

    /// Where a `[[target]]` points, as a vault-relative path.
    ///
    /// This is what the UI calls when a wiki-link is clicked: `data-target` is
    /// on the element in both the resolved and the unresolved state, and the
    /// answer here decides between "open it" and "offer to create it".
    pub fn resolve(&self, target: &str) -> Option<NoteMeta> {
        let guard = self.open.read().ok()?;
        guard.as_ref()?.index.resolve(target).cloned()
    }

    /// Every note linking to `rel`.
    pub fn backlinks_for(&self, rel: &str) -> Vec<Backlink> {
        self.open
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|v| v.index.backlinks_for(rel)))
            .unwrap_or_default()
    }

    /// Every note linking to a name that resolves to nothing.
    pub fn unresolved_for(&self, target: &str) -> Vec<Backlink> {
        self.open
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|v| v.index.unresolved_for(target)))
            .unwrap_or_default()
    }

    /// Searches the vault, streaming hits. See [`search::search`].
    pub fn search(
        &self,
        query: &Query,
        on_hit: &mut dyn FnMut(&Hit) -> bool,
    ) -> Result<SearchStats, IpcError> {
        let root = self.require_root()?;
        Ok(search::search(&root, query, on_hit))
    }

    /// Claims the search slot, invalidating whatever was running.
    ///
    /// The caller passes the returned id back to [`Self::search_is_current`]
    /// from inside its hit callback and stops when it goes false. Cancellation
    /// is cooperative rather than a thread kill because the search holds a file
    /// buffer and a walk cursor, and killing it mid-read is how a temp file
    /// gets left behind.
    pub fn begin_search(&self) -> u64 {
        self.searches.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Whether `id` is still the newest search.
    pub fn search_is_current(&self, id: u64) -> bool {
        self.searches.load(Ordering::SeqCst) == id
    }

    /// Resolves a vault-relative path to a real file, refusing anything outside
    /// the vault. Every command that takes a path from the frontend goes
    /// through here.
    pub fn resolve_path(&self, rel: &str) -> Result<PathBuf, IpcError> {
        let root = self.require_root()?;
        scan::resolve_in(&root, rel)
    }

    /// The vault-relative form of an absolute path, when it is inside.
    ///
    /// The inverse direction: the open document knows its own absolute path,
    /// and the backlinks panel needs the vault-relative one to look it up.
    pub fn relative_of(&self, path: &Path) -> Option<String> {
        let root = self.root()?;
        let resolved = path.canonicalize().ok()?;
        let rel = resolved.strip_prefix(&root).ok()?.to_str()?;
        Some(if std::path::MAIN_SEPARATOR == '/' {
            rel.to_string()
        } else {
            rel.replace(std::path::MAIN_SEPARATOR, "/")
        })
    }

    fn require_root(&self) -> Result<PathBuf, IpcError> {
        self.root()
            .ok_or_else(|| err(IpcErrorKind::Invalid, "no vault is open", None))
    }
}

/// What a rebuild cost, for the status line and for the receipt.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct IndexStats {
    pub notes: usize,
    /// Notes re-read from disk.
    pub parsed: usize,
    /// Notes taken from the cache.
    pub cached: usize,
    /// Heap the index holds. The vault's whole budget is 30 MB for 5 000 notes.
    pub memory_bytes: usize,
}

/// A temporary vault on disk, for tests.
///
/// `#[cfg(test)]` rather than a `dev-dependencies` crate: it is thirty lines,
/// and `tempfile` would be a dependency added to a project whose entire
/// premise is a 2.8 MiB ceiling.
#[cfg(test)]
pub(crate) mod testing {
    use std::path::{Path, PathBuf};

    pub struct Sandbox {
        base: PathBuf,
        root: PathBuf,
    }

    impl Sandbox {
        pub fn new(name: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "marklet-vault-{}-{name}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&base);
            let root = base.join("vault");
            std::fs::create_dir_all(&root).expect("sandbox root");
            std::fs::create_dir_all(base.join("outside")).expect("sandbox sibling");
            Self {
                root: root.canonicalize().expect("canonical sandbox root"),
                base,
            }
        }

        pub fn root(&self) -> &Path {
            &self.root
        }

        pub fn cache_dir(&self) -> PathBuf {
            self.base.join("cache")
        }

        pub fn note(&self, rel: &str, body: &str) -> PathBuf {
            self.bytes(rel, body.as_bytes())
        }

        pub fn bytes(&self, rel: &str, body: &[u8]) -> PathBuf {
            let path = self.root.join(rel);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).expect("note directory");
            }
            std::fs::write(&path, body).expect("note");
            path
        }

        /// A file next to the vault that must stay unreachable from inside it.
        pub fn outside(&self, name: &str, body: &str) -> PathBuf {
            let path = self.base.join("outside").join(name);
            std::fs::write(&path, body).expect("outside file");
            path
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{self, RenderOpts};
    use testing::Sandbox;

    fn opened(s: &Sandbox) -> (VaultState, AssetRoot) {
        let assets = AssetRoot::new();
        let state = VaultState::new();
        state.open(&assets, s.root()).expect("opens");
        state.reindex(None, &mut |_| true).expect("indexes");
        (state, assets)
    }

    #[test]
    fn opening_a_vault_widens_the_asset_root_to_it() {
        let s = Sandbox::new("state-assets");
        s.note("journal/2026/entry.md", "# Entry\n");
        let assets = AssetRoot::new();
        // Start where `open_path` leaves it: the document's own directory.
        assets.set(&s.root().join("journal/2026"));

        let state = VaultState::new();
        let info = state.open(&assets, s.root()).expect("opens");

        assert_eq!(assets.get().as_deref(), Some(s.root()));
        assert_eq!(info.name, "vault");
    }

    #[test]
    fn the_resolver_plugs_straight_into_the_render_core() {
        let s = Sandbox::new("state-resolver");
        s.note("Known.md", "# Known\n");
        let (state, _assets) = opened(&s);

        let resolver = state.resolver();
        let doc = render::render(
            b"[[Known]] and [[Missing]]\n",
            RenderOpts {
                wiki_resolver: Some(&resolver),
                ..RenderOpts::default()
            },
        );

        assert!(
            doc.html.contains(r#"class="wikilink" href="#),
            "got {}",
            doc.html
        );
        assert!(
            doc.html
                .contains(r#"class="wikilink unresolved" data-target="Missing""#),
            "got {}",
            doc.html
        );
        assert!(doc.links[0].resolved);
        assert!(!doc.links[1].resolved, "unresolved is a normal state");
    }

    #[test]
    fn with_no_vault_open_every_wikilink_is_unresolved_rather_than_an_error() {
        let state = VaultState::new();
        let resolver = state.resolver();
        let doc = render::render(
            b"[[Anything]]\n",
            RenderOpts {
                wiki_resolver: Some(&resolver),
                ..RenderOpts::default()
            },
        );
        assert!(doc.html.contains("wikilink unresolved"));
    }

    #[test]
    fn the_backlinks_panel_answers_for_the_open_note() {
        let s = Sandbox::new("state-backlinks");
        s.note("Note.md", "# Note\n");
        s.note("a.md", "# A\n\n[[Note]]\n");
        s.note("b.md", "# B\n\n[[Note]]\n");
        let (state, _assets) = opened(&s);

        let back = state.backlinks_for("Note.md");
        assert_eq!(
            back.iter().map(|b| b.path.as_str()).collect::<Vec<_>>(),
            ["a.md", "b.md"]
        );
    }

    #[test]
    fn a_path_outside_the_vault_is_denied_not_answered() {
        let s = Sandbox::new("state-denied");
        s.note("ok.md", "#");
        s.outside("secret.md", "not yours");
        let (state, _assets) = opened(&s);

        assert!(state.resolve_path("ok.md").is_ok());
        assert_eq!(
            state.resolve_path("../outside/secret.md").unwrap_err().kind,
            IpcErrorKind::Denied
        );
    }

    #[test]
    fn commands_refuse_cleanly_when_no_vault_is_open() {
        let state = VaultState::new();
        assert_eq!(
            state.scan(&mut |_| true).unwrap_err().kind,
            IpcErrorKind::Invalid
        );
        assert_eq!(
            state
                .search(&Query::default(), &mut |_| true)
                .unwrap_err()
                .kind,
            IpcErrorKind::Invalid
        );
        assert!(state.backlinks_for("a.md").is_empty());
        assert!(!state.is_open());
    }

    #[test]
    fn relative_of_maps_an_absolute_document_path_back_into_the_vault() {
        let s = Sandbox::new("state-relative");
        let note = s.note("sub/note.md", "#");
        let (state, _assets) = opened(&s);

        assert_eq!(state.relative_of(&note).as_deref(), Some("sub/note.md"));
        assert_eq!(state.relative_of(Path::new("/etc/hosts")), None);
    }

    #[test]
    fn closing_forgets_the_vault() {
        let s = Sandbox::new("state-close");
        s.note("a.md", "#");
        let (state, _assets) = opened(&s);
        assert!(state.is_open());
        state.close();
        assert!(!state.is_open());
        assert!(state.resolve("a").is_none());
    }
}

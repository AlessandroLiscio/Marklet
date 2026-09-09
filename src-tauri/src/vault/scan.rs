//! Walking a vault, streaming.
//!
//! A vault is a directory a user pointed us at. It contains whatever they put
//! there — a `node_modules` a colleague committed, a 4 GB `target/`, a symlink
//! into `/etc`, a filename that is not valid UTF-8. None of those may produce a
//! panic, a hang, or a read outside the root.
//!
//! **The walk streams.** `walk` hands the caller a batch every [`BATCH`]
//! entries rather than returning a `Vec` at the end, because a 5 000-note vault
//! takes longer to enumerate than a user is willing to look at an empty sidebar.
//! The first batch is a painted tree; the rest arrive underneath it.
//!
//! **Symlinks are skipped entirely, not followed.** `walkdir` with
//! `follow_links(false)` still *yields* a symlink as an entry, and reading that
//! entry follows the link — so a link named `notes.md` pointing at
//! `~/.ssh/id_rsa` would be read and indexed as a note. Refusing them at the
//! walk is the only place where one check covers every later reader.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use crate::ipc::{IpcError, IpcErrorKind};

use super::err;

/// Directories that are never a user's notes, by name at any depth.
///
/// Dotted names are excluded separately, which already covers `.obsidian` and
/// `.git`; both are listed anyway because the list is also documentation of
/// what the walk deliberately refuses, and a reader should not have to derive
/// it from a `starts_with('.')` three lines away.
pub const IGNORED_NAMES: &[&str] = &[".git", "node_modules", "target", ".obsidian"];

/// Extensions that make a file a note. Matches the file-association list P6
/// registers on Windows, deliberately: a file Explorer opens with Marklet is a
/// file the vault should show.
pub const NOTE_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd"];

/// Entries per streamed batch.
///
/// Small enough that the first batch arrives in single-digit milliseconds on a
/// warm cache, large enough that a 5 000-note vault is ~20 IPC messages rather
/// than 5 000.
pub const BATCH: usize = 256;

/// One node of the folder tree.
///
/// `path` is vault-relative with forward slashes on every platform: it is an
/// identity that crosses the IPC boundary, gets stored in the index cache, and
/// is compared against wiki-link targets. One spelling, everywhere.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub dir: bool,
    /// 0 for a direct child of the root. The tree renders indentation from this
    /// rather than counting separators in `path`.
    pub depth: usize,
    pub size: u64,
    /// Milliseconds since the Unix epoch, 0 when the platform will not say.
    /// The index invalidates a cached note on this plus `size`.
    pub mtime_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ScanStats {
    pub dirs: usize,
    pub notes: usize,
    /// Entries the walk refused: symlinks, unreadable directories, and names
    /// that are not valid UTF-8 and so cannot cross the IPC boundary.
    pub skipped: usize,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

/// A directory the walk will not descend into.
pub fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || IGNORED_NAMES.contains(&name)
}

/// A file the vault treats as a note.
pub fn is_note(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| NOTE_EXTENSIONS.contains(&e.as_str()))
}

/// Canonicalizes a vault root, or explains why it is not one.
pub fn canonical_root(root: &Path) -> Result<PathBuf, IpcError> {
    let resolved = root.canonicalize().map_err(|e| {
        err(
            IpcErrorKind::NotFound,
            format!("could not open that folder: {e}"),
            Some(root),
        )
    })?;
    if !resolved.is_dir() {
        return Err(err(
            IpcErrorKind::Invalid,
            "that path is a file, not a folder",
            Some(&resolved),
        ));
    }
    Ok(resolved)
}

/// Resolves a vault-relative path to a real file inside the vault.
///
/// **Canonicalize, then check** — the same rule as `protocol.rs`, for the same
/// reason. Screening the string for `..` before resolving is a bypass: a
/// symlink, a `%2e%2e`, or a Windows `8.3` short name all survive it.
///
/// `root` must already be canonical; [`canonical_root`] is how it gets that way.
pub fn resolve_in(root: &Path, rel: &str) -> Result<PathBuf, IpcError> {
    let candidate = Path::new(rel);

    // Refused before touching the disk. `has_root()` as well as `is_absolute()`
    // because on Windows a leading slash with no drive letter is root-relative
    // and *not* absolute, and `join` there keeps the drive prefix while
    // replacing everything after it. A colon is refused for the same family of
    // reason: it is a drive letter or an NTFS alternate data stream, never a
    // note the user made.
    if rel.is_empty() || candidate.has_root() || candidate.is_absolute() || rel.contains(':') {
        return Err(err(
            IpcErrorKind::Denied,
            "that path is outside the vault",
            Some(candidate),
        ));
    }

    let resolved = root.join(candidate).canonicalize().map_err(|e| {
        err(
            IpcErrorKind::NotFound,
            format!("could not open that note: {e}"),
            Some(candidate),
        )
    })?;

    if !resolved.starts_with(root) {
        return Err(err(
            IpcErrorKind::Denied,
            "that path is outside the vault",
            Some(&resolved),
        ));
    }

    Ok(resolved)
}

/// Walks `root`, handing the caller a batch of entries at a time.
///
/// Returning `false` from `on_batch` cancels the walk; the stats say so. That
/// is how a second `open_vault` stops the first one instead of racing it.
///
/// Parents always precede their children, and siblings are sorted by name, so
/// the receiving side can build a tree by appending and never has to sort or
/// look ahead.
pub fn walk(
    root: &Path,
    batch_size: usize,
    on_batch: &mut dyn FnMut(&[Entry]) -> bool,
) -> ScanStats {
    let started = Instant::now();
    let mut stats = ScanStats::default();
    let mut buf: Vec<Entry> = Vec::with_capacity(batch_size.max(1));

    let walker = WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            // A symlink is never descended into and never yielded — see the
            // module docs. This is also what stops a cyclic link from hanging
            // the walk.
            if e.path_is_symlink() {
                return false;
            }
            let Some(name) = e.file_name().to_str() else {
                return false;
            };
            if e.file_type().is_dir() {
                !is_ignored_dir(name)
            } else {
                !name.starts_with('.')
            }
        });

    for entry in walker {
        let Ok(entry) = entry else {
            // An unreadable directory is ordinary — a permission-denied folder
            // inside a vault must not abort the whole scan.
            stats.skipped += 1;
            continue;
        };

        let dir = entry.file_type().is_dir();
        if !dir && !is_note(entry.path()) {
            continue;
        }

        let Some(rel) = relative(root, entry.path()) else {
            stats.skipped += 1;
            continue;
        };
        let Some(name) = entry.file_name().to_str() else {
            stats.skipped += 1;
            continue;
        };

        let meta = entry.metadata().ok();
        buf.push(Entry {
            path: rel,
            name: name.to_string(),
            dir,
            depth: entry.depth().saturating_sub(1),
            size: meta.as_ref().map_or(0, std::fs::Metadata::len),
            mtime_ms: meta.as_ref().map_or(0, mtime_ms),
        });

        if dir {
            stats.dirs += 1;
        } else {
            stats.notes += 1;
        }

        if buf.len() >= batch_size {
            if !on_batch(&buf) {
                stats.cancelled = true;
                break;
            }
            buf.clear();
        }
    }

    if !stats.cancelled && !buf.is_empty() && !on_batch(&buf) {
        stats.cancelled = true;
    }

    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    stats
}

/// Milliseconds since the Unix epoch, or 0 when the filesystem will not say.
///
/// 0 rather than an error: a missing mtime makes the index cache miss on that
/// one file forever, which is slow but correct. Refusing to scan the vault
/// because one file has no timestamp is neither.
pub fn mtime_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// Same, for a path that may not exist.
pub fn stat(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), mtime_ms(&meta)))
}

/// The vault-relative path of `path`, forward-slashed, or `None` if it is not
/// representable as UTF-8 (it could not cross the IPC boundary) or not inside
/// the root (it could not be trusted).
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?.to_str()?;
    Some(if std::path::MAIN_SEPARATOR == '/' {
        rel.to_string()
    } else {
        rel.replace(std::path::MAIN_SEPARATOR, "/")
    })
}

/// Reads a file's bytes into `buf`, reusing its allocation.
///
/// The search scans thousands of files in a row; a fresh `Vec` per file is
/// thousands of allocations and thousands of page faults for no benefit.
pub fn read_into(path: &Path, buf: &mut Vec<u8>) -> std::io::Result<()> {
    use std::io::Read;
    buf.clear();
    let mut file = std::fs::File::open(path)?;
    if let Ok(meta) = file.metadata() {
        buf.reserve(meta.len() as usize);
    }
    file.read_to_end(buf)?;
    Ok(())
}

/// Unix-epoch milliseconds, for stamping a cache.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::testing::Sandbox;

    fn all(root: &Path) -> Vec<Entry> {
        let mut out = Vec::new();
        walk(root, BATCH, &mut |b| {
            out.extend_from_slice(b);
            true
        });
        out
    }

    #[test]
    fn finds_notes_and_folders_and_nothing_else() {
        let s = Sandbox::new("scan-basic");
        s.note("a.md", "# A");
        s.note("sub/b.markdown", "# B");
        s.note("sub/c.txt", "not a note");
        s.note("image.png", "");

        let paths: Vec<_> = all(s.root()).iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, ["a.md", "sub", "sub/b.markdown"]);
    }

    #[test]
    fn skips_the_directories_a_vault_never_wants() {
        let s = Sandbox::new("scan-ignored");
        s.note("keep.md", "#");
        for dir in [".git", "node_modules", "target", ".obsidian", ".hidden"] {
            s.note(&format!("{dir}/x.md"), "#");
        }

        let paths: Vec<_> = all(s.root()).iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, ["keep.md"]);
    }

    #[test]
    fn skips_dotfiles() {
        let s = Sandbox::new("scan-dotfiles");
        s.note("keep.md", "#");
        s.note(".secret.md", "#");
        assert_eq!(all(s.root()).len(), 1);
    }

    #[test]
    fn parents_precede_children_and_siblings_are_sorted() {
        let s = Sandbox::new("scan-order");
        s.note("z/2.md", "#");
        s.note("z/1.md", "#");
        s.note("a.md", "#");

        let paths: Vec<_> = all(s.root()).iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, ["a.md", "z", "z/1.md", "z/2.md"]);
    }

    #[test]
    fn streams_in_batches_rather_than_collecting() {
        let s = Sandbox::new("scan-batches");
        for i in 0..10 {
            s.note(&format!("n{i}.md"), "#");
        }

        let mut batches = 0;
        let stats = walk(s.root(), 3, &mut |b| {
            assert!(b.len() <= 3);
            batches += 1;
            true
        });
        assert_eq!(stats.notes, 10);
        assert_eq!(batches, 4, "10 entries in batches of 3");
    }

    #[test]
    fn the_size_and_mtime_a_walk_reports_are_stable_and_match_a_fresh_stat() {
        // `size` and `mtime_ms` are not decoration: the index persists them as
        // its per-file cache key, so a walk that reported them differently from
        // one run to the next — or differently from a plain `stat` of the same
        // path — would make every warm build miss and the cache worthless,
        // silently and only on the filesystem where the two disagree.
        //
        // `entry.metadata()` is `walkdir`'s, taken during the walk;
        // `stat` is `std::fs::metadata`, taken after it. The walk yields no
        // symlinks (see `a_symlink_pointing_outside_the_vault_is_not_walked`),
        // which is what lets the two be required to agree: on a symlink they
        // would not, one following the link and one describing it.
        let s = Sandbox::new("scan-metadata-stable");
        for i in 0..64 {
            s.note(
                &format!("folder-{}/n{i:02}.md", i % 4),
                &format!("# {i}\n\n{}\n", "x".repeat(i)),
            );
        }

        let first = all(s.root());
        let second = all(s.root());
        assert_eq!(
            first, second,
            "two walks of an unchanged tree report the same entries"
        );

        for entry in first.iter().filter(|e| !e.dir) {
            let fresh = stat(&s.root().join(&entry.path)).expect("the note is still there");
            assert_eq!(
                (entry.size, entry.mtime_ms),
                fresh,
                "{} disagrees with a fresh stat",
                entry.path
            );
        }
    }

    #[test]
    fn a_false_from_the_callback_cancels() {
        let s = Sandbox::new("scan-cancel");
        for i in 0..10 {
            s.note(&format!("n{i}.md"), "#");
        }
        let stats = walk(s.root(), 2, &mut |_| false);
        assert!(stats.cancelled);
        assert_eq!(stats.notes, 2, "stopped after the first batch");
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_pointing_outside_the_vault_is_not_walked() {
        let s = Sandbox::new("scan-symlink");
        s.note("real.md", "#");
        let outside = s.outside("secret.md", "not yours");
        std::os::unix::fs::symlink(&outside, s.root().join("link.md")).unwrap();

        let paths: Vec<_> = all(s.root()).iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, ["real.md"], "the symlink must not be yielded");
    }

    #[test]
    fn resolve_in_rejects_traversal_to_a_file_that_exists() {
        // 403-equivalent, not 404, and that is the whole test: the target
        // exists, so canonicalization SUCCEEDS and the containment check is
        // what refuses it.
        let s = Sandbox::new("scan-traversal");
        s.note("ok.md", "#");
        s.outside("secret.md", "not yours");

        let err = resolve_in(s.root(), "../outside/secret.md").unwrap_err();
        assert_eq!(err.kind, IpcErrorKind::Denied);
        assert!(resolve_in(s.root(), "ok.md").is_ok());
    }

    #[test]
    fn resolve_in_rejects_absolute_and_root_relative_paths() {
        let s = Sandbox::new("scan-absolute");
        s.note("ok.md", "#");
        for bad in ["/etc/passwd", "C:/Windows/notepad.exe", ""] {
            assert_eq!(
                resolve_in(s.root(), bad).unwrap_err().kind,
                IpcErrorKind::Denied,
                "{bad} must be refused"
            );
        }
    }

    #[test]
    fn a_missing_note_is_not_found_rather_than_denied() {
        let s = Sandbox::new("scan-missing");
        s.note("ok.md", "#");
        assert_eq!(
            resolve_in(s.root(), "absent.md").unwrap_err().kind,
            IpcErrorKind::NotFound
        );
    }

    #[test]
    fn a_file_root_is_invalid() {
        let s = Sandbox::new("scan-file-root");
        let note = s.note("ok.md", "#");
        assert_eq!(
            canonical_root(&note).unwrap_err().kind,
            IpcErrorKind::Invalid
        );
    }
}

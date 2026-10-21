//! Creating a new, empty note.
//!
//! The one place Marklet makes a file that did not exist. Everything else in
//! the vault module only reads, so the rules live together here: a name is a
//! single path component, the folder must resolve inside the root, and an
//! existing file is never touched.

use std::path::Path;

use crate::ipc::{IpcError, IpcErrorKind};

use super::err;
use super::scan::{is_note, resolve_in, Entry};

/// Turns what the user typed into a file name, or says why it is not one.
///
/// A name without a Markdown extension gets `.md`, so `ideas` and `v1.2` both
/// become notes. Titles with a dot in them are kept whole.
pub fn note_file_name(typed: &str) -> Result<String, IpcError> {
    let name = typed.trim();
    let bad = |why: &str| Err(err(IpcErrorKind::Invalid, why, None));

    if name.is_empty() {
        return bad("give the note a name");
    }
    // Separators and the characters Windows forbids. The webview may pass a
    // path here, and a name that is more than one component would let a
    // create reach outside the folder it was asked about.
    if name.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
    {
        return bad("a name cannot contain / \\ : < > \" | ? * or control characters");
    }
    if name.starts_with('.') {
        return bad("a name cannot start with a dot: the explorer hides those files");
    }
    if name.ends_with('.') {
        return bad("a name cannot end with a dot");
    }

    // Windows treats `CON.md` as the console device, whatever the extension.
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.ends_with(|c: char| c.is_ascii_digit() && c != '0'));
    if reserved {
        return bad("Windows reserves that name");
    }

    // Only a name that already ends in a note extension is left alone. Anything
    // else gets `.md`, so `Mr.Smith` and `v1.2` stay titles rather than being
    // mistaken for a file type.
    let named = if is_note(Path::new(name)) {
        name.to_string()
    } else {
        format!("{name}.md")
    };
    Ok(named)
}

/// Creates an empty note named `typed` in `parent_rel` (vault-relative, empty
/// for the root) and describes it as the explorer's next row.
///
/// `create_new`, never `create`: it fails if the name is taken instead of
/// truncating whatever is there.
pub fn create_note(root: &Path, parent_rel: &str, typed: &str) -> Result<Entry, IpcError> {
    let file = note_file_name(typed)?;

    let parent = if parent_rel.is_empty() {
        root.to_path_buf()
    } else {
        resolve_in(root, parent_rel)?
    };
    if !parent.is_dir() {
        return Err(err(
            IpcErrorKind::Invalid,
            "that path is a file, not a folder",
            Some(&parent),
        ));
    }

    let target = parent.join(&file);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => err(
                IpcErrorKind::Invalid,
                format!("{file} already exists in that folder"),
                Some(&target),
            ),
            _ => err(
                IpcErrorKind::Io,
                format!("could not create the note: {e}"),
                Some(&target),
            ),
        })?;

    let path = if parent_rel.is_empty() {
        file.clone()
    } else {
        format!("{parent_rel}/{file}")
    };
    Ok(Entry {
        depth: path.matches('/').count(),
        path,
        name: file,
        dir: false,
        size: 0,
        mtime_ms: target.metadata().map_or(0, |m| super::scan::mtime_ms(&m)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("marklet-create-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("notes/deep")).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn a_bare_name_becomes_a_markdown_file() {
        assert_eq!(note_file_name("ideas").unwrap(), "ideas.md");
        assert_eq!(note_file_name("  ideas  ").unwrap(), "ideas.md");
        assert_eq!(note_file_name("v1.2 plan").unwrap(), "v1.2 plan.md");
        assert_eq!(note_file_name("Todo.MD").unwrap(), "Todo.MD");
        assert_eq!(note_file_name("a.markdown").unwrap(), "a.markdown");
        assert_eq!(note_file_name("Mr.Smith").unwrap(), "Mr.Smith.md");
        assert_eq!(note_file_name("pic.png").unwrap(), "pic.png.md");
        assert_eq!(note_file_name("console").unwrap(), "console.md");
    }

    #[test]
    fn names_that_are_not_one_component_are_refused() {
        for bad in [
            "", "  ", "a/b", "..\\x", "../x", "a:b", "x?", ".hidden", "dot.", "a\nb",
        ] {
            let e = note_file_name(bad).unwrap_err();
            assert_eq!(e.kind, IpcErrorKind::Invalid, "{bad:?}");
        }
    }

    #[test]
    fn creates_in_the_root_and_in_a_subfolder() {
        let root = sandbox("ok");
        let top = create_note(&root, "", "first").unwrap();
        assert_eq!((top.path.as_str(), top.depth), ("first.md", 0));
        let deep = create_note(&root, "notes/deep", "second").unwrap();
        assert_eq!(
            (deep.path.as_str(), deep.depth),
            ("notes/deep/second.md", 2)
        );
        assert!(root.join("notes/deep/second.md").is_file());
        assert_eq!(std::fs::read(root.join("first.md")).unwrap(), b"");
    }

    #[test]
    fn never_overwrites() {
        let root = sandbox("exists");
        std::fs::write(root.join("keep.md"), b"# precious").unwrap();
        let e = create_note(&root, "", "keep").unwrap_err();
        assert_eq!(e.kind, IpcErrorKind::Invalid);
        assert_eq!(std::fs::read(root.join("keep.md")).unwrap(), b"# precious");
    }

    #[test]
    fn refuses_a_folder_outside_the_root() {
        let root = sandbox("escape");
        for parent in ["..", "../x", "/etc", "notes/../../x"] {
            let e = create_note(&root, parent, "x").unwrap_err();
            assert!(
                matches!(e.kind, IpcErrorKind::Denied | IpcErrorKind::NotFound),
                "{parent}: {:?}",
                e.kind
            );
        }
    }

    #[test]
    fn a_file_is_not_a_parent() {
        let root = sandbox("file");
        std::fs::write(root.join("a.md"), b"").unwrap();
        assert!(create_note(&root, "a.md", "x").is_err());
    }
}

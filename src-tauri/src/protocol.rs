//! The `marklet://` asset scheme: local images, and nothing else.
//!
//! A markdown file can reference `![](./diagram.png)`, and that image has to
//! reach a webview which is otherwise granted no filesystem access at all. This
//! module is the entire bridge, which is why it is also the entire attack
//! surface for reading files off disk.
//!
//! **Canonicalize, then check.** Every path is resolved on the filesystem
//! *first* — following symlinks, collapsing `..` — and only then compared
//! against the allowed root. Screening the raw string for `".."` before
//! resolving is the classic bypass: `%2e%2e%2f`, a symlink pointing outward,
//! and `..%c0%af` all survive it, and none survive this.
//!
//! Chosen over Tauri's own asset protocol because it is smaller and because the
//! rejection lives in one readable function instead of in a scope-glob config.
//! See `.claude/skills/tauri-ipc/SKILL.md`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tauri::http::{Request, Response, StatusCode};

use crate::webpath::{guess_mime, percent_decode};

/// The one directory tree `marklet://` will serve, shared with the commands
/// that change it when a document or vault opens.
///
/// `None` means nothing is served — the state before a document is open. A
/// request arriving then is answered 403, not 404: "no root" and "not found in
/// the root" are different situations and only one of them is a bug.
#[derive(Default, Clone)]
pub struct AssetRoot(Arc<RwLock<Option<PathBuf>>>);

impl AssetRoot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Points the scheme at a directory. The path is canonicalized here so
    /// every later comparison is between two resolved paths.
    pub fn set(&self, dir: &Path) {
        let resolved = dir.canonicalize().ok();
        if let Ok(mut guard) = self.0.write() {
            *guard = resolved;
        }
    }

    pub fn get(&self) -> Option<PathBuf> {
        self.0.read().ok().and_then(|g| g.clone())
    }

    /// The URL prefix the renderer must prepend to relative image paths.
    ///
    /// Tauri rewrites custom schemes differently per platform: Windows serves
    /// them over `http://<scheme>.localhost/` because WebView2 will not accept
    /// a genuinely custom scheme, while WebKitGTK takes `<scheme>://localhost/`
    /// as written. Both spellings are in the CSP for the same reason.
    pub fn url_prefix() -> &'static str {
        if cfg!(windows) {
            "http://marklet.localhost/"
        } else {
            "marklet://localhost/"
        }
    }
}

/// Resolves a request path against the root, or explains why it will not.
///
/// Split out from the handler so it is testable without a running webview —
/// the traversal rejection is the part that must never silently regress.
pub fn resolve(root: &Path, request_path: &str) -> Result<PathBuf, StatusCode> {
    let decoded = percent_decode(request_path.trim_start_matches('/'));
    if decoded.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Absolute and root-relative paths never come from a relative markdown
    // reference, so they are refused before they touch the disk.
    //
    // `has_root()` rather than `is_absolute()` alone, and the difference is not
    // cosmetic: on Windows `Path::new("/etc/passwd").is_absolute()` is **false**
    // — a leading slash with no drive letter is root-relative, not absolute. Such
    // a path still escapes this directory, because `join` on Windows keeps the
    // prefix and replaces everything after it: `C:\…\vault` joined with
    // `/Windows/System32/config/SAM` is `C:\Windows\System32\config\SAM`.
    //
    // The containment check below would still refuse it, so this is defence in
    // depth rather than the only gate — but a request should be refused by the
    // check that describes it, and the two branches diverging by platform is
    // exactly how a reviewer ends up reasoning about the wrong one.
    let candidate = Path::new(&decoded);
    if candidate.has_root() || candidate.is_absolute() || decoded.contains(':') {
        return Err(StatusCode::FORBIDDEN);
    }

    let joined = root.join(candidate);

    // The canonicalization that makes the check meaningful. It fails for a path
    // that does not exist, which is a 404 rather than a 403: a missing image is
    // an ordinary state of a document being edited.
    let resolved = joined.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;

    if !resolved.starts_with(root) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !resolved.is_file() {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(resolved)
}

/// Serves one request. Registered on the builder in [`crate::run`].
pub fn handle(root: &AssetRoot, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(base) = root.get() else {
        return deny(StatusCode::FORBIDDEN, "no document is open");
    };

    let path = request.uri().path().to_string();

    match resolve(&base, &path) {
        Ok(file) => match std::fs::read(&file) {
            Ok(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", guess_mime(&file))
                // The webview is the only client and the file may change under
                // it while the document is open, so nothing is cached.
                .header("Cache-Control", "no-store")
                .body(bytes)
                .unwrap_or_else(|_| deny(StatusCode::INTERNAL_SERVER_ERROR, "response")),
            Err(_) => deny(StatusCode::NOT_FOUND, "unreadable"),
        },
        Err(status) => deny(status, "rejected"),
    }
}

fn deny(status: StatusCode, why: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .body(why.as_bytes().to_vec())
        .expect("static response is always valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A served root with a sibling directory that must stay unreachable.
    ///
    /// The sibling exists so the traversal test can escape to a file that
    /// really is there **on every platform**. Reaching for `/etc/passwd`
    /// instead made the test pass on Linux for the right reason and on Windows
    /// for the wrong one: the file is absent there, so canonicalization failed
    /// and the request 404'd without the containment check ever running. A test
    /// that stops exercising its own branch on half the target platforms is
    /// worse than no test, because it still reports green.
    fn sandbox() -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("marklet-protocol-{}", std::process::id()));
        let root = base.join("vault");
        let outside = base.join("outside");

        let _ = fs::create_dir_all(root.join("img"));
        let _ = fs::create_dir_all(&outside);
        fs::write(root.join("img/ok.png"), b"png").unwrap();
        fs::write(root.join("note.md"), b"# hi").unwrap();
        fs::write(outside.join("secret.txt"), b"not yours").unwrap();

        (
            root.canonicalize().unwrap(),
            outside.canonicalize().unwrap(),
        )
    }

    #[test]
    fn serves_a_file_inside_the_root() {
        let (root, _) = sandbox();
        assert!(resolve(&root, "/img/ok.png").is_ok());
    }

    #[test]
    fn rejects_traversal_to_a_file_that_really_exists() {
        // 403, not 404, and that distinction is the whole test: the target
        // exists, so canonicalization SUCCEEDS and the request is refused by
        // the `starts_with(root)` containment check. A 404 here would mean the
        // path simply did not resolve, and would prove nothing about containment.
        let (root, _) = sandbox();
        assert_eq!(
            resolve(&root, "/../outside/secret.txt").unwrap_err(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn rejects_percent_encoded_traversal() {
        // The whole reason decoding happens before the check. Screening the raw
        // string for ".." would have let this through.
        let (root, _) = sandbox();
        assert_eq!(
            resolve(&root, "/%2e%2e%2foutside%2fsecret.txt").unwrap_err(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn collapses_repeated_leading_slashes_rather_than_treating_them_as_absolute() {
        // `//img/ok.png` is not an absolute-path attack once the leading
        // separators are stripped: it becomes `img/ok.png`, resolved *inside*
        // the root, and is served. Worth asserting explicitly — the first
        // version of this test expected a rejection and was wrong about which
        // branch it exercised.
        let (root, _) = sandbox();
        assert!(resolve(&root, "//img/ok.png").is_ok());
    }

    #[test]
    fn rejects_a_percent_encoded_root_relative_path() {
        // This is what reaches the `has_root` branch. `%2F` survives the
        // leading-slash strip and only becomes `/` during decoding, so the
        // decoded path is root-relative — proof that the order is strip,
        // decode, *then* judge, and that judging earlier would miss it.
        //
        // `has_root()` rather than `is_absolute()`, because on Windows a bare
        // leading slash is root-relative and NOT absolute, and `join` there
        // keeps the drive prefix while replacing the rest — so this escapes the
        // served directory on both platforms and must be refused on both.
        let (root, _) = sandbox();
        assert_eq!(
            resolve(&root, "/%2Foutside%2Fsecret.txt").unwrap_err(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn rejects_a_windows_drive_letter() {
        let (root, _) = sandbox();
        assert_eq!(
            resolve(&root, "/C:/Windows/System32/drivers/etc/hosts").unwrap_err(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn a_missing_file_is_not_found_rather_than_forbidden() {
        // A document that references an image the author has not added yet is
        // ordinary. Answering 403 would make it look like a security event.
        let (root, _) = sandbox();
        assert_eq!(
            resolve(&root, "/img/absent.png").unwrap_err(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn rejects_a_directory() {
        let (root, _) = sandbox();
        assert_eq!(resolve(&root, "/img").unwrap_err(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn an_empty_path_is_a_bad_request() {
        let (root, _) = sandbox();
        assert_eq!(resolve(&root, "/").unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn url_prefix_matches_the_csp() {
        // If these two ever disagree, images fail to load with a CSP violation
        // in a console nobody is reading. tauri.conf.json carries both spellings.
        let prefix = AssetRoot::url_prefix();
        assert!(prefix.ends_with('/'), "prefix must be joinable: {prefix}");
        assert!(prefix.contains("marklet"));
    }
}

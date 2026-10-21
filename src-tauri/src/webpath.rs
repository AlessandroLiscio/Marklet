//! Web-path helpers shared by everything that turns a URL into a file.
//!
//! These live here rather than beside their first caller because two very
//! different code paths need the *same* answer from them:
//!
//! - [`export::html`](crate::export::html) inlines a local image into a
//!   standalone file,
//! - [`protocol`](crate::protocol) serves one to the webview over `marklet://`.
//!
//! `percent_decode` in particular decides which bytes become a filesystem path,
//! and a security-relevant decoder with two implementations is a decoder with
//! two behaviours. One of them will be the one nobody updated.

use std::path::{Path, PathBuf};

/// Content type from the file extension, for the handful of types Marklet
/// actually serves.
///
/// Extension sniffing rather than content sniffing, deliberately: the webview
/// must never be told a file is something other than what its name claims, and
/// `application/octet-stream` for anything unrecognised means an unexpected
/// file downloads instead of executing.
pub fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// `Path::canonicalize`, taught to follow symlinks that live inside WSL.
///
/// A file in a WSL distro is seen from Windows as `\\wsl.localhost\<distro>\...`
/// (older builds: `\\wsl$\...`). A symlink there usually has an absolute Linux
/// target such as `/home/me/repo/CLAUDE.md`, which Windows reads as
/// `C:\home\me\...` and cannot find: `canonicalize` fails with "path not
/// found" (os error 3) for a file that opens fine in Explorer, and the 9P share
/// does not let `read_link` see the target either.
///
/// For such a path only, and only on Windows, the distro is asked to resolve it
/// (`wsl.exe -d <distro> realpath`) and the answer is mapped back onto the
/// share. The result is the real file, so the callers' starts-with checks
/// compare resolved paths exactly as they do everywhere else.
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) => match wsl_realpath(path) {
            Some(real) => real.canonicalize().or(Err(e)),
            None => Err(e),
        },
    }
}

/// Splits `\\wsl.localhost\Ubuntu\home\me` (or `\\wsl$\...`, or the `//`
/// spelling) into the distro and its Linux path: `("Ubuntu", "/home/me")`.
#[cfg_attr(not(windows), allow(dead_code))]
fn split_wsl_share(path: &Path) -> Option<(String, String)> {
    let s = path.to_str()?;
    let rest = s.strip_prefix("\\\\").or_else(|| s.strip_prefix("//"))?;
    let rest = ["wsl.localhost", "wsl$"].iter().find_map(|host| {
        let head = rest.get(..host.len())?;
        head.eq_ignore_ascii_case(host)
            .then(|| rest[host.len()..].strip_prefix(['\\', '/']))
            .flatten()
    })?;
    let (distro, tail) = rest.split_once(['\\', '/']).unwrap_or((rest, ""));
    if distro.is_empty() {
        return None;
    }
    Some((distro.to_string(), format!("/{}", tail.replace('\\', "/"))))
}

/// The share path for a Linux path inside `distro`.
#[cfg_attr(not(windows), allow(dead_code))]
fn wsl_share_path(distro: &str, linux: &str) -> PathBuf {
    PathBuf::from(format!(
        "\\\\wsl.localhost\\{distro}{}",
        linux.replace('/', "\\")
    ))
}

#[cfg(windows)]
fn wsl_realpath(path: &Path) -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;

    let (distro, linux) = split_wsl_share(path)?;
    // Separate arguments after `--`, never a shell string: the path comes from
    // the command line and may hold anything a file name can.
    let out = std::process::Command::new("wsl.exe")
        .args(["-d", &distro, "--exec", "realpath", "-e", "--", &linux])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let real = String::from_utf8(out.stdout).ok()?;
    let real = real.trim_end_matches(['\n', '\r']);
    real.starts_with('/').then(|| wsl_share_path(&distro, real))
}

#[cfg(not(windows))]
fn wsl_realpath(_: &Path) -> Option<PathBuf> {
    None
}

/// Just enough percent-decoding for the `%20`-style escapes markdown image
/// paths pick up from spaces in filenames.
///
/// A malformed escape is left as literal text rather than guessed at. That
/// matters on the protocol path: a decoder that invents bytes for `%2` invents
/// a different filename than the one the author wrote.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn mime_is_case_insensitive() {
        assert_eq!(guess_mime(&PathBuf::from("a.PNG")), "image/png");
        assert_eq!(guess_mime(&PathBuf::from("a.JpEg")), "image/jpeg");
    }

    #[test]
    fn unknown_extension_is_octet_stream() {
        assert_eq!(
            guess_mime(&PathBuf::from("a.exe")),
            "application/octet-stream"
        );
        assert_eq!(guess_mime(&PathBuf::from("a")), "application/octet-stream");
    }

    #[test]
    fn decodes_escaped_spaces() {
        assert_eq!(percent_decode("my%20file.png"), "my file.png");
    }

    #[test]
    fn decodes_an_escaped_percent() {
        assert_eq!(percent_decode("100%25.png"), "100%.png");
    }

    #[test]
    fn leaves_a_malformed_escape_alone() {
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zz.png"), "a%zz.png");
    }

    #[test]
    fn decodes_an_escaped_traversal_so_the_caller_can_reject_it() {
        // The decoder does not judge; it decodes. Rejecting traversal is
        // protocol.rs's job, and it can only do that job on decoded bytes —
        // which is exactly why checking a raw string for ".." is a bypass.
        assert_eq!(percent_decode("%2e%2e%2fetc%2fpasswd"), "../etc/passwd");
    }

    #[test]
    fn splits_a_wsl_share_into_distro_and_linux_path() {
        let split = |p: &str| split_wsl_share(Path::new(p));
        let want = Some(("Ubuntu".to_string(), "/home/a/n.md".to_string()));
        assert_eq!(split(r"\\wsl.localhost\Ubuntu\home\a\n.md"), want);
        assert_eq!(split(r"\\WSL$\Ubuntu\home\a\n.md"), want);
        assert_eq!(split("//wsl.localhost/Ubuntu/home/a/n.md"), want);
        assert_eq!(
            split(r"\\wsl.localhost\Ubuntu"),
            Some(("Ubuntu".to_string(), "/".to_string()))
        );
    }

    #[test]
    fn other_paths_are_not_wsl_shares() {
        let split = |p: &str| split_wsl_share(Path::new(p));
        assert_eq!(split(r"C:\Users\a\n.md"), None);
        assert_eq!(split(r"\\server\share\wsl.localhost"), None);
        assert_eq!(split(r"\\wsl.localhostile\x\y"), None);
        assert_eq!(split(r"\\wsl.localhost"), None);
        assert_eq!(split("/home/a/n.md"), None);
    }

    #[test]
    fn maps_a_linux_path_back_onto_the_share() {
        assert_eq!(
            wsl_share_path("Ubuntu", "/home/a/n.md"),
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\a\n.md")
        );
    }

    #[test]
    fn a_missing_path_still_fails() {
        assert!(canonicalize(Path::new("/definitely/not/here.md")).is_err());
    }
}

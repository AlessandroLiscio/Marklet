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

use std::path::Path;

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
}

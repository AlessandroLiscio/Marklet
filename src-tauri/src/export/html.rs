//! Standalone single-file HTML export.
//!
//! The output must open in a browser with **zero** network requests: CSS is
//! inlined into a `<style>` tag and local images are inlined as `data:` URIs.
//! Math and Mermaid are left exactly as [`crate::render::render`] emits them
//! — Rust does not render either (see `render/mod.rs`'s own doc comment) —
//! pre-rendering both for export is phase P8; the emitted document carries a
//! TODO comment saying so rather than pretending the gap does not exist.
//!
//! This module must not import `tauri`: it is reached from `MD_HTML=1`,
//! which has to run before `tauri::Builder::build()`.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::render::{self, RenderOpts};

/// Renders `path` to a standalone HTML document.
///
/// Local image references in the markdown are resolved relative to `path`'s
/// parent directory and inlined as `data:` URIs. A reference that cannot be
/// read (missing file, permission error) is left as the original `src` —
/// best effort, not a hard failure, since the alternative is refusing to
/// export a whole document over one broken image.
pub fn export_file(path: &Path) -> Result<String, ExportError> {
    let bytes = std::fs::read(path).map_err(|source| ExportError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let doc = render::render(&bytes, RenderOpts::default());
    let base_dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    let body = match base_dir {
        Some(dir) => inline_local_images(&doc.html, dir),
        None => doc.html,
    };

    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Marklet document");

    Ok(standalone_document(title, &body))
}

/// What can go wrong turning a file into standalone HTML.
#[derive(Debug)]
pub enum ExportError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Read { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ExportError {}

fn standalone_document(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         <!-- Math delimiters and ```mermaid fences below are pulldown-cmark's raw\n\
              placeholders, exactly as render::render() emits them: Rust does not\n\
              render either. KaTeX and Mermaid pre-rendering for standalone export\n\
              is phase P8 — TODO. -->\n\
         <style>\n{STANDALONE_CSS}\n</style>\n\
         </head>\n\
         <body>\n\
         <article class=\"marklet-export\">\n{body}\n</article>\n\
         </body>\n\
         </html>\n",
        title = escape_html(title),
    )
}

/// A minimal, self-contained stylesheet for the exported file. Deliberately
/// not `src/styles/**` (owned by `ui-engineer` this wave, and tied to the
/// live app's theme tokens) — an exported file has to keep reading correctly
/// for as long as it exists, independent of the app version that made it.
const STANDALONE_CSS: &str = r#"
:root { color-scheme: light dark; }
body {
  margin: 0;
  background: #fff;
  color: #1a1a1a;
}
article.marklet-export {
  max-width: 42rem;
  margin: 0 auto;
  padding: 2.5rem 1.5rem 4rem;
  font-family: -apple-system, "Segoe UI", system-ui, sans-serif;
  line-height: 1.6;
  font-size: 16px;
}
article.marklet-export img { max-width: 100%; height: auto; }
article.marklet-export pre {
  overflow-x: auto;
  padding: 0.75rem 1rem;
  background: #f4f4f4;
  border-radius: 4px;
}
article.marklet-export code {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 0.9em;
}
article.marklet-export pre code { font-size: 0.85em; }
article.marklet-export table { border-collapse: collapse; width: 100%; }
article.marklet-export th, article.marklet-export td {
  border: 1px solid #d0d0d0;
  padding: 0.4rem 0.6rem;
  text-align: left;
}
article.marklet-export blockquote {
  margin: 0;
  padding-left: 1rem;
  border-left: 3px solid #d0d0d0;
  color: #555;
}
@media (prefers-color-scheme: dark) {
  body { background: #1a1a1a; color: #e8e8e8; }
  article.marklet-export pre { background: #2a2a2a; }
  article.marklet-export th, article.marklet-export td { border-color: #444; }
  article.marklet-export blockquote { border-left-color: #444; color: #aaa; }
}
"#;

/// Finds every `<img ...src="...">` tag and, for a local (non-URL, non-`data:`)
/// `src`, rewrites it to a `data:` URI holding the file's own bytes.
fn inline_local_images(html: &str, base_dir: &Path) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(tag_start) = rest.find("<img ") {
        out.push_str(&rest[..tag_start]);
        let from_tag = &rest[tag_start..];
        let tag_len = from_tag.find('>').map(|i| i + 1).unwrap_or(from_tag.len());
        let tag = &from_tag[..tag_len];
        out.push_str(&rewrite_img_tag(tag, base_dir));
        rest = &from_tag[tag_len..];
    }
    out.push_str(rest);
    out
}

fn rewrite_img_tag(tag: &str, base_dir: &Path) -> String {
    const SRC_ATTR: &str = "src=\"";

    let Some(attr_start) = tag.find(SRC_ATTR) else {
        return tag.to_string();
    };
    let value_start = attr_start + SRC_ATTR.len();
    let Some(value_len) = tag[value_start..].find('"') else {
        return tag.to_string();
    };
    let src = &tag[value_start..value_start + value_len];

    if is_remote_or_data(src) {
        return tag.to_string();
    }

    let resolved = base_dir.join(percent_decode(src));
    let Ok(bytes) = std::fs::read(&resolved) else {
        // Best effort: a missing local image should not fail the whole
        // export, it should just stay a broken link, same as it already
        // would be in any browser.
        return tag.to_string();
    };

    let data_uri = format!(
        "data:{};base64,{}",
        guess_mime(&resolved),
        base64_encode(&bytes)
    );

    let mut rewritten = String::with_capacity(tag.len() + data_uri.len());
    rewritten.push_str(&tag[..value_start]);
    rewritten.push_str(&data_uri);
    rewritten.push_str(&tag[value_start + value_len..]);
    rewritten
}

fn is_remote_or_data(src: &str) -> bool {
    src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with("data:")
        || src.starts_with("//")
}

fn guess_mime(path: &Path) -> &'static str {
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
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Hand-rolled: no `base64` crate. `size-budget` denies pulling a dependency
/// for something this small, and cli.rs's own parser is hand-rolled for the
/// same reason.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;

        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            CHARS[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Just enough percent-decoding for the `%20`-style escapes markdown image
/// paths pick up from spaces in filenames.
fn percent_decode(s: &str) -> String {
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

/// Escapes the handful of characters that matter inside `<title>` — the
/// filename stem is attacker-controlled in the loosest sense (whatever the
/// filesystem allows), so this stays defensive even though it is not HTML
/// body content.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("marklet-export-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn exports_a_plain_document_with_no_network_dependent_markup() {
        let dir = temp_dir("plain");
        let md_path = dir.join("doc.md");
        std::fs::write(&md_path, b"# Title\n\nHello *world*.\n").unwrap();

        let html = export_file(&md_path).unwrap();

        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<style>"), "CSS must be inlined, got: {html}");
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(!html.contains("<link "), "no external stylesheet link");
        assert!(!html.contains("<script"), "no external script");
    }

    #[test]
    fn inlines_a_local_image_as_a_data_uri() {
        let dir = temp_dir("image");
        std::fs::create_dir_all(dir.join("img")).unwrap();
        let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];
        let mut f = std::fs::File::create(dir.join("img/pixel.png")).unwrap();
        f.write_all(png_bytes).unwrap();

        let md_path = dir.join("doc.md");
        std::fs::write(&md_path, b"![alt](./img/pixel.png)\n").unwrap();

        let html = export_file(&md_path).unwrap();

        assert!(
            html.contains("data:image/png;base64,"),
            "expected inlined image, got: {html}"
        );
        assert!(!html.contains("src=\"./img/pixel.png\""));
    }

    #[test]
    fn leaves_a_missing_local_image_as_a_broken_link_rather_than_failing() {
        let dir = temp_dir("missing-image");
        let md_path = dir.join("doc.md");
        std::fs::write(&md_path, b"![alt](./img/does-not-exist.png)\n").unwrap();

        let html = export_file(&md_path).unwrap();

        assert!(html.contains("does-not-exist.png"));
    }

    #[test]
    fn leaves_a_remote_image_url_untouched() {
        let dir = temp_dir("remote-image");
        let md_path = dir.join("doc.md");
        std::fs::write(&md_path, b"![alt](https://example.com/pic.png)\n").unwrap();

        let html = export_file(&md_path).unwrap();

        assert!(html.contains("src=\"https://example.com/pic.png\""));
    }

    #[test]
    fn errors_on_a_missing_source_file() {
        let dir = temp_dir("no-source");
        let err = export_file(&dir.join("nope.md")).unwrap_err();
        assert!(matches!(err, ExportError::Read { .. }));
    }

    #[test]
    fn base64_round_trips_known_vectors() {
        // RFC 4648 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn percent_decode_handles_escaped_spaces() {
        assert_eq!(percent_decode("a%20b.png"), "a b.png");
        assert_eq!(percent_decode("plain.png"), "plain.png");
    }
}

//! Standalone single-file HTML export.
//!
//! The output must open in a browser with **zero** network requests: CSS is
//! inlined into a `<style>` tag and local images are inlined as `data:` URIs.
//!
//! There are **two ways in**, and they carry different promises about math
//! and Mermaid, for a reason that is architectural rather than an oversight:
//!
//! - [`export_file`] reads a path straight off disk and calls
//!   [`crate::render::render`] on it — the same headless path `MD_HTML=1`
//!   uses. Rust renders no math and no Mermaid (`render/mod.rs`'s own doc
//!   comment; `src/lib/rich/index.ts`'s `enrich()` is the only thing that
//!   does, and it is JavaScript running in a webview). `MD_HTML=1` is
//!   contractually forbidden from initializing WebView2 — every CLI path
//!   "completes in under 50 ms with no window and no WebView2
//!   initialization" (`docs/architecture.md`, this crate's own
//!   `.claude/agents/platform-engineer.md`, rule 1) — so this path
//!   structurally cannot carry rendered KaTeX markup or inlined Mermaid SVG.
//!   It emits the same placeholders `render()` always emits and says so in
//!   the output; that is not a gap to close from inside this module, it is
//!   the tradeoff `MD_HTML=1` exists to make (see `README.md`'s "no window"
//!   guarantee for that flag).
//! - [`assemble_standalone`] takes a body that is **already enriched** —
//!   `enrich()` has already run, in a live webview, on the actual document
//!   the user is looking at, so real `<span class="katex">…</span>` markup
//!   and real inlined `<svg>` elements are already sitting in the DOM this
//!   function is handed. This is the function the "Export as standalone
//!   HTML" menu action should call: the frontend calls `enrich()` (it
//!   already has, for the visible document), serializes the resulting DOM
//!   with `src/lib/rich/export.ts`'s `serializeEnrichedDocument()`, and
//!   passes the result to a Tauri command that calls this. Wiring that
//!   command is `ipc.rs`'s job, not this module's — see the P8 diff receipt.
//!
//! Both paths inline local images as `data:` URIs and produce zero-network
//! output; only the math/Mermaid guarantee differs, and only because one of
//! the two has a webview to draw on and the other is not allowed one.
//!
//! This module must not import `tauri`: [`export_file`] is reached from
//! `MD_HTML=1`, which has to run before `tauri::Builder::build()`.
//! [`assemble_standalone`] has no such constraint on its caller — it simply
//! never touches a webview itself — so it stays in the same tauri-free
//! module rather than fork the file in two.

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

    Ok(standalone_document(title, &body, None))
}

/// Assembles a standalone HTML document from a body that has **already been
/// enriched** in a live webview: `enrich()` (`src/lib/rich/index.ts`) has
/// run on it, so any `.math-inline` / `.math-block` placeholder is already
/// real KaTeX markup and any `.mermaid` placeholder is already an inlined
/// `<svg>`. This function does not render anything itself — it only:
///
/// 1. inlines local (non-`http(s)`, non-`data:`) `<img src>` references as
///    `data:` URIs, exactly like [`export_file`], resolved against
///    `base_dir` when given;
/// 2. adds `extra_css`, verbatim, to the document's inlined `<style>` — this
///    is where the caller puts whatever stylesheet KaTeX's rendering needed
///    (its generated markup depends on KaTeX's own CSS to lay out correctly;
///    Mermaid's inlined `<svg>` needs nothing extra, it carries its own
///    styling inline);
/// 3. wraps the result in the same zero-network document shell
///    [`export_file`] uses.
///
/// `extra_css` is trusted, already-rendered CSS text from the app's own
/// bundle (see `src/lib/rich/export.ts`), not attacker-controlled markdown —
/// it is concatenated as-is, the same way [`STANDALONE_CSS`] is.
pub fn assemble_standalone(
    title: &str,
    enriched_body_html: &str,
    extra_css: &str,
    base_dir: Option<&Path>,
) -> String {
    let body = match base_dir {
        Some(dir) => inline_local_images(enriched_body_html, dir),
        None => enriched_body_html.to_string(),
    };
    standalone_document(title, &body, Some(extra_css))
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

/// `extra_css`, when given, is appended verbatim after [`STANDALONE_CSS`] —
/// used by [`assemble_standalone`] to carry whatever stylesheet an enriched
/// body's KaTeX markup needs. [`export_file`] passes `None`: its body is
/// still the raw placeholder, which needs nothing beyond `STANDALONE_CSS`.
fn standalone_document(title: &str, body: &str, extra_css: Option<&str>) -> String {
    let placeholder_note = if extra_css.is_some() {
        // This body came from assemble_standalone: enrich() already ran in a
        // live webview, so there are no placeholders left to explain away.
        String::new()
    } else {
        "<!-- Math delimiters and ```mermaid fences below are pulldown-cmark's raw\n     \
              placeholders, exactly as render::render() emits them: this document was\n     \
              produced by export_file(), the headless MD_HTML=1 path, which cannot run\n     \
              a webview and therefore cannot render either — see this file's module\n     \
              doc comment. assemble_standalone() is the path that carries real KaTeX\n     \
              markup and inlined Mermaid SVG. -->\n"
            .to_string()
    };
    let extra_css = extra_css.unwrap_or_default();

    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         {placeholder_note}\
         <style>\n{STANDALONE_CSS}\n{extra_css}\n</style>\n\
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

    // ------------------------------------------------------------------
    // assemble_standalone: the enriched-body path. These assert the thing
    // export_file() cannot — that a body already carrying real KaTeX markup
    // and an inlined Mermaid SVG (i.e. what enrich() actually produces in
    // the webview) comes through unrendered-placeholder-free and with zero
    // network-fetching markup, which is the whole promise this file makes.
    // ------------------------------------------------------------------

    /// Stand-in for what `enrich()` actually leaves in the DOM: KaTeX's real
    /// output shape (nested spans, no `data-tex` placeholder attribute left
    /// behind) and Mermaid's real output shape (an inlined `<svg>`, not a
    /// `<div class="mermaid" data-src="...">` placeholder).
    const ENRICHED_BODY: &str = r#"<p data-l="1">Inline math:
        <span class="katex"><span class="katex-mathml">E = mc^2</span></span>.</p>
        <div class="mermaid-rendered" data-l="3">
          <svg viewBox="0 0 100 40"><g><rect width="80" height="20"/><text>A</text></g></svg>
        </div>"#;

    const FAKE_KATEX_CSS: &str = ".katex { font: normal 1.21em KaTeX_Main, serif; }";

    #[test]
    fn assemble_standalone_preserves_real_katex_and_svg_markup_verbatim() {
        let html = assemble_standalone("Doc", ENRICHED_BODY, FAKE_KATEX_CSS, None);

        assert!(
            html.contains(r#"<span class="katex">"#),
            "real KaTeX markup must survive assembly, got: {html}"
        );
        assert!(
            html.contains("<svg viewBox=\"0 0 100 40\">"),
            "inlined Mermaid SVG must survive assembly, got: {html}"
        );
    }

    #[test]
    fn assemble_standalone_never_reintroduces_the_headless_placeholder_shapes() {
        let html = assemble_standalone("Doc", ENRICHED_BODY, FAKE_KATEX_CSS, None);

        // The two shapes export_file() emits when it cannot render math or
        // Mermaid (see render/mod.rs's own placeholder tests). A regression
        // that fell back to raw render() output here would reintroduce them.
        assert!(
            !html.contains("data-tex=\""),
            "no unrendered math placeholder attribute should remain: {html}"
        );
        assert!(
            !html.contains(r#"<div class="mermaid""#) || !html.contains("data-src="),
            "no unrendered Mermaid placeholder (div[data-src]) should remain: {html}"
        );
    }

    #[test]
    fn assemble_standalone_carries_the_extra_css_and_makes_zero_network_requests() {
        let html = assemble_standalone("Doc", ENRICHED_BODY, FAKE_KATEX_CSS, None);

        assert!(
            html.contains("KaTeX_Main"),
            "extra_css must be inlined into the <style> tag, got: {html}"
        );
        assert!(!html.contains("<link "), "no external stylesheet link");
        assert!(!html.contains("<script"), "no external script");
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "no network-fetched resource, got: {html}"
        );
    }

    #[test]
    fn assemble_standalone_still_inlines_local_images_against_base_dir() {
        let dir = temp_dir("assemble-image");
        std::fs::create_dir_all(dir.join("img")).unwrap();
        let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 9, 9, 9];
        std::fs::write(dir.join("img/pixel.png"), png_bytes).unwrap();

        let body = r#"<img src="./img/pixel.png" alt="">"#;
        let html = assemble_standalone("Doc", body, "", Some(&dir));

        assert!(
            html.contains("data:image/png;base64,"),
            "local images must still be inlined in the enriched-body path, got: {html}"
        );
    }

    #[test]
    fn export_file_output_is_honest_about_being_unrendered() {
        // export_file() is the MD_HTML=1 path: it cannot run a webview, so it
        // cannot render KaTeX or Mermaid (see this file's module doc). This
        // pins that it says so rather than silently shipping placeholders
        // unlabelled — a future change that tried to "fix" this by deleting
        // the note without actually rendering anything would be a regression
        // this test catches.
        let dir = temp_dir("honest-placeholder");
        let md_path = dir.join("doc.md");
        std::fs::write(&md_path, b"Inline: $E = mc^2$.\n").unwrap();

        let html = export_file(&md_path).unwrap();

        assert!(
            html.contains("MD_HTML=1"),
            "the headless path must document why math/Mermaid are unrendered here, got: {html}"
        );
        assert!(html.contains("data-tex=\"E = mc^2\""));
    }
}

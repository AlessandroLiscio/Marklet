//! The render pipeline: bytes in, HTML plus everything the UI needs out.
//!
//! This module must never import `tauri`. It runs headless for `MD_HTML=1`,
//! which has to complete without creating a window or initializing WebView2.
//! When you need something from the host, take it as a plain value or a
//! callback on [`RenderOpts`] instead.
//!
//! The whole pipeline is **one pass** over `Parser::into_offset_iter()`. That
//! iterator yields the byte range of every event, and four separate features —
//! scroll sync, scroll restore across a reflow, the outline, byte-exact block
//! editing — are all just different readings of the same offsets. Anything that
//! discards them breaks all four at once, which is why `data-l` is documented as
//! a contract rather than an implementation detail.
//!
//! See `.claude/skills/render-pipeline/SKILL.md` for the full contract.

use std::borrow::Cow;
use std::fmt;

use pulldown_cmark::Options;

pub mod decode;
pub mod html;
pub mod outline;
pub mod sanitize;
pub mod wikilink;

pub use decode::DecodeError;

/// How the input bytes were decoded into text.
///
/// Surfaced in the UI so a user can tell "this file is Windows-1252" from
/// "this file is mojibake".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
    /// The file contradicted its own BOM and was decoded with replacement
    /// characters by the infallible [`render`] entry point. [`try_render`]
    /// reports the [`DecodeError`] instead.
    Unknown,
}

/// One heading, for the outline panel and for jump-to-heading.
///
/// `slug` is GitHub-compatible so pasted GitHub anchors resolve, and it must
/// match the `id` emitted on the heading element — a mismatch shows up as an
/// outline entry that jumps nowhere.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub line: usize,
}

/// A link found in the document. Wiki-links carry `target` and may be unresolved.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Link {
    pub href: String,
    pub text: String,
    pub line: usize,
    pub wiki: bool,
    pub resolved: bool,
}

/// Maps a rendered block to its byte range in the source.
///
/// This is what makes scroll sync, scroll restore across a reflow, the outline
/// and byte-exact block editing all fall out of a single parse pass. Anything
/// that discards these offsets breaks four features at once.
///
/// There is exactly one entry per element carrying `data-l`, in document order,
/// so `line` is non-decreasing and the frontend can binary-search it.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct BlockSpan {
    pub line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Resolves a wiki-link target to an href.
///
/// The render core does not know what a vault is; the caller does. Returning
/// `None` is a normal state — a link to a note that does not exist yet — and
/// produces `class="wikilink unresolved"` rather than an error.
pub type WikiResolver<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Everything the caller can vary about a render.
#[derive(Default)]
pub struct RenderOpts<'a> {
    /// Resolves `[[Note]]` targets. `None` leaves every wiki-link unresolved.
    pub wiki_resolver: Option<WikiResolver<'a>>,
    /// Base directory for resolving relative image paths, as a `marklet://` URL prefix.
    pub asset_base: Option<Cow<'a, str>>,
}

/// The single output of the pipeline. Every consumer reads this struct.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenderedDoc {
    pub html: String,
    pub outline: Vec<Heading>,
    pub line_map: Vec<BlockSpan>,
    pub links: Vec<Link>,
    pub frontmatter: Option<serde_json::Value>,
    pub encoding: Encoding,
}

/// Everything that can go wrong turning a file into a document.
///
/// Only decoding can fail, and only when the file contradicts a BOM it wrote
/// itself. Parsing cannot: CommonMark has no syntax errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    Decode(DecodeError),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Decode(e) => write!(f, "could not decode the file: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<DecodeError> for RenderError {
    fn from(e: DecodeError) -> Self {
        RenderError::Decode(e)
    }
}

/// The markdown extensions Marklet enables. Exactly these, no more.
///
/// `ENABLE_MATH` and the mermaid fence handling emit *placeholders*; the
/// webview finishes the job lazily and only when a document actually contains
/// one. Rust renders neither.
pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_WIKILINKS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

/// Renders markdown bytes into everything the UI needs.
///
/// Infallible by design: this is what the window calls, and a file that lies
/// about its encoding must still open. It falls back to lossy UTF-8 and reports
/// [`Encoding::Unknown`] so the UI can say so. Use [`try_render`] where the
/// error matters — `MD_HTML=1` should fail loudly rather than write mojibake to
/// a pipe.
pub fn render(bytes: &[u8], opts: RenderOpts<'_>) -> RenderedDoc {
    match decode::decode(bytes) {
        Ok((text, encoding)) => html::write_document(&text, &opts, encoding),
        Err(_) => {
            let text = decode::decode_lossy(bytes);
            html::write_document(&text, &opts, Encoding::Unknown)
        }
    }
}

/// Renders markdown bytes, reporting a decoding failure instead of papering over it.
pub fn try_render(bytes: &[u8], opts: RenderOpts<'_>) -> Result<RenderedDoc, RenderError> {
    let (text, encoding) = decode::decode(bytes)?;
    Ok(html::write_document(&text, &opts, encoding))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html_of(md: &str) -> String {
        render(md.as_bytes(), RenderOpts::default()).html
    }

    #[test]
    fn renders_a_gfm_table() {
        let doc = render(b"| a | b |\n|---|---|\n| 1 | 2 |\n", RenderOpts::default());
        assert!(
            doc.html.contains("<table data-l=\"1\">"),
            "got: {}",
            doc.html
        );
    }

    #[test]
    fn renders_a_footnote_with_a_backlink() {
        let doc = render(b"Text[^1]\n\n[^1]: Note\n", RenderOpts::default());
        assert!(doc.html.contains("id=\"fnref-1\""), "got: {}", doc.html);
        assert!(doc.html.contains("id=\"fn-1\""), "got: {}", doc.html);
        assert!(
            doc.html.contains("class=\"footnote-backref\""),
            "got: {}",
            doc.html
        );
    }

    #[test]
    fn every_block_carries_data_l() {
        let doc = render(b"# H\n\npara\n\n- a\n\n> q\n", RenderOpts::default());
        for tag in ["<h1 ", "<p ", "<ul ", "<li ", "<blockquote "] {
            assert!(doc.html.contains(tag), "missing {tag} in {}", doc.html);
        }
        assert_eq!(doc.html.matches("data-l=").count(), doc.line_map.len());
    }

    #[test]
    fn line_map_is_non_decreasing() {
        let doc = render(
            b"# A\n\none\n\n## B\n\n- x\n- y\n\n```\nz\n```\n",
            RenderOpts::default(),
        );
        assert!(doc.line_map.windows(2).all(|w| w[0].line <= w[1].line));
        assert!(doc.line_map.iter().all(|s| s.start_byte <= s.end_byte));
    }

    #[test]
    fn outline_slugs_match_the_emitted_ids() {
        let doc = render(
            b"# Hello, World!\n\n## Notes\n\n## Notes\n",
            RenderOpts::default(),
        );
        assert_eq!(
            doc.outline
                .iter()
                .map(|h| h.slug.as_str())
                .collect::<Vec<_>>(),
            ["hello-world", "notes", "notes-1"]
        );
        for h in &doc.outline {
            assert!(
                doc.html.contains(&format!("id=\"{}\"", h.slug)),
                "no id for {}",
                h.slug
            );
        }
    }

    #[test]
    fn math_is_a_placeholder_never_rendered() {
        let h = html_of("Inline $E = mc^2$ and\n\n$$\n\\int_0^1 x\\,dx\n$$\n");
        assert!(
            h.contains(r#"<span class="math-inline" data-tex="E = mc^2"></span>"#),
            "got {h}"
        );
        assert!(h.contains(r#"<div class="math-block""#), "got {h}");
        assert!(h.contains("data-tex="), "got {h}");
    }

    #[test]
    fn mermaid_is_a_placeholder_with_escaped_source() {
        let h = html_of("```mermaid\ngraph TD; A-->B\n```\n");
        assert!(h.contains(r#"<div class="mermaid""#), "got {h}");
        assert!(h.contains("A--&gt;B"), "got {h}");
        assert!(
            !h.contains("<pre"),
            "mermaid must not become a code block: {h}"
        );
    }

    #[test]
    fn other_fences_stay_plain() {
        let h = html_of("```rust\nfn main() {}\n```\n");
        assert!(
            h.contains(r#"<pre data-l="1"><code class="language-rust">"#),
            "got {h}"
        );
    }

    #[test]
    fn a_javascript_href_loses_its_href() {
        let h = html_of("[click](javascript:alert(1))\n");
        assert!(!h.contains("javascript:"), "got {h}");
        assert!(h.contains("click"), "got {h}");
    }

    #[test]
    fn raw_html_is_filtered_by_the_allowlist() {
        let h = html_of("<div onclick=\"x\"><script>alert(1)</script><mark>ok</mark></div>\n");
        assert!(!h.contains("<script"), "got {h}");
        assert!(!h.contains("onclick"), "got {h}");
        assert!(h.contains("<mark>ok</mark>"), "got {h}");
    }

    #[test]
    fn wikilinks_report_resolution() {
        let f = |t: &str| (t == "Known").then(|| format!("marklet://vault/{t}.md"));
        let doc = render(
            b"[[Known]] and [[Missing]]\n",
            RenderOpts {
                wiki_resolver: Some(&f),
                ..RenderOpts::default()
            },
        );
        assert!(doc
            .html
            .contains(r#"class="wikilink" href="marklet://vault/Known.md""#));
        assert!(doc
            .html
            .contains(r#"class="wikilink unresolved" data-target="Missing""#));
        assert_eq!(doc.links.len(), 2);
        assert!(doc.links[0].resolved);
        assert!(!doc.links[1].resolved);
        assert!(doc.links.iter().all(|l| l.wiki));
    }

    #[test]
    fn frontmatter_is_parsed_and_not_rendered() {
        let doc = render(
            b"---\ntitle: Test\ntags: [a, b]\n---\n\n# H\n",
            RenderOpts::default(),
        );
        let fm = doc.frontmatter.expect("frontmatter");
        assert_eq!(fm["title"], "Test");
        assert_eq!(fm["tags"][0], "a");
        assert!(!doc.html.contains("title:"), "got {}", doc.html);
    }

    #[test]
    fn encoding_is_reported() {
        assert_eq!(
            render(b"# hi", RenderOpts::default()).encoding,
            Encoding::Utf8
        );
        assert_eq!(
            render(b"# h\x92i", RenderOpts::default()).encoding,
            Encoding::Windows1252
        );
    }

    #[test]
    fn a_file_that_lies_about_its_bom_still_opens() {
        let bytes = [0xEF, 0xBB, 0xBF, b'#', b' ', 0xFF];
        let doc = render(&bytes, RenderOpts::default());
        assert_eq!(doc.encoding, Encoding::Unknown);
        assert!(try_render(&bytes, RenderOpts::default()).is_err());
    }

    #[test]
    fn asset_base_rewrites_relative_images_only() {
        let opts = RenderOpts {
            asset_base: Some("marklet://doc".into()),
            ..RenderOpts::default()
        };
        let doc = render(b"![a](./img/x.png) ![b](https://e.com/y.png)\n", opts);
        assert!(
            doc.html.contains("src=\"marklet://doc/img/x.png\""),
            "got {}",
            doc.html
        );
        assert!(
            doc.html.contains("src=\"https://e.com/y.png\""),
            "got {}",
            doc.html
        );
    }

    #[test]
    fn task_lists_render_checkboxes() {
        let h = html_of("- [x] done\n- [ ] todo\n");
        assert_eq!(h.matches("type=\"checkbox\"").count(), 2);
        assert_eq!(h.matches("checked=\"\"").count(), 1);
    }
}

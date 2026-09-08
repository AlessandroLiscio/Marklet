//! The render pipeline: bytes in, HTML plus everything the UI needs out.
//!
//! This module must never import `tauri`. It runs headless for `MD_HTML=1`,
//! which has to complete without creating a window or initializing WebView2.
//! When you need something from the host, take it as a plain value or a
//! callback on [`RenderOpts`] instead.
//!
//! See `.claude/skills/render-pipeline/SKILL.md` for the full contract.

use std::borrow::Cow;

use pulldown_cmark::{html, Options, Parser};

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
/// P0 baseline: decodes as UTF-8 and emits plain CommonMark+GFM HTML so the
/// app shell has something real to display. Phase P1 replaces the body with the
/// offset-aware writer, the sanitizer, the outline builder and wiki-link
/// resolution — the signature and the types above are the contract it must keep.
pub fn render(bytes: &[u8], _opts: RenderOpts<'_>) -> RenderedDoc {
    let text = String::from_utf8_lossy(bytes);
    let parser = Parser::new_ext(&text, options());

    let mut out = String::with_capacity(text.len() * 3 / 2);
    html::push_html(&mut out, parser);

    RenderedDoc {
        html: out,
        outline: Vec::new(),
        line_map: Vec::new(),
        links: Vec::new(),
        frontmatter: None,
        encoding: Encoding::Utf8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_gfm_table() {
        let doc = render(b"| a | b |\n|---|---|\n| 1 | 2 |\n", RenderOpts::default());
        assert!(doc.html.contains("<table>"), "got: {}", doc.html);
    }

    #[test]
    fn renders_a_footnote() {
        let doc = render(b"Text[^1]\n\n[^1]: Note\n", RenderOpts::default());
        assert!(doc.html.contains("footnote"), "got: {}", doc.html);
    }
}

//! The HTML writer: one pass over `Parser::into_offset_iter()`.
//!
//! Everything Marklet knows about a document is produced here, in a single
//! traversal, because the parser is a pull parser with no AST and that is the
//! only reason a 5 MB file renders in under 120 ms. Collecting the events into a
//! `Vec` to make a second pass convenient would throw that away — the temptation
//! is real, so the code below buffers only two bounded things: the body of the
//! heading currently being written (its `id` depends on its own text, which
//! arrives after the opening tag) and the text of a code fence that turned out
//! to be `mermaid`.
//!
//! `data-l` is emitted on every block-level opening tag and mirrored into
//! `line_map`. Five frontend modules read it; see
//! `.claude/skills/render-pipeline/SKILL.md`.

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, LinkType, Parser, Tag, TagEnd,
};

use super::outline::Slugger;
use super::sanitize::{escape_attr, escape_html, is_safe_url, Sanitizer};
use super::{wikilink, BlockSpan, Encoding, Heading, Link, RenderOpts, RenderedDoc};

/// Byte offset to 1-based line number.
///
/// Built once per document and binary-searched, which is ~17 comparisons on a
/// 125 000-line file. The obvious alternative — counting newlines from the last
/// query — is faster in the common case and quadratic in the pathological one,
/// and this is not the place to gamble on input shape.
pub struct LineIndex {
    /// Byte offset of the first character of each line. Always starts with 0.
    starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut starts = Vec::with_capacity(bytes.len() / 32 + 1);
        starts.push(0);
        for (i, &b) in bytes.iter().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts }
    }

    /// The 1-based line containing `byte`.
    pub fn line_of(&self, byte: usize) -> usize {
        match self.starts.binary_search(&byte) {
            Ok(i) => i + 1,
            Err(i) => i, // `i` is the count of line starts at or before `byte`
        }
    }

    pub fn len(&self) -> usize {
        self.starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }
}

/// Where `Event::Text` currently goes.
///
/// Only one of these can be active at a time — a metadata block cannot contain a
/// mermaid fence, an image alt cannot contain a code block — so this is an enum
/// rather than a stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextMode {
    /// Escaped into the output, and into any open plain-text captures.
    Normal,
    /// Into `buf`, verbatim: the frontmatter, which is parsed, not rendered.
    Metadata,
    /// Into `buf`, verbatim: mermaid source, which becomes a `data-src`.
    Mermaid,
    /// Into `buf`, verbatim: an image's alt text, which becomes an attribute.
    AltText,
}

enum TableState {
    Head,
    Body,
}

struct PendingHeading {
    level: HeadingLevel,
    id: Option<String>,
    classes: Vec<String>,
    line: usize,
}

struct PendingLink {
    href: Option<String>,
    line: usize,
    wiki: bool,
}

struct PendingImage {
    dest: String,
    title: String,
}

/// A paragraph whose opening tag has not been written yet.
struct PendingPara {
    line: usize,
    range: Range<usize>,
}

struct Writer<'o> {
    lines: LineIndex,
    opts: &'o RenderOpts<'o>,

    out: String,
    /// Holds the real output while a heading body is being buffered into `out`.
    held: Option<String>,
    /// Plain-text sinks: one per open heading or link.
    captures: Vec<String>,
    /// Scratch for whichever non-`Normal` [`TextMode`] is active.
    buf: String,
    mode: TextMode,
    end_newline: bool,

    outline: Vec<Heading>,
    line_map: Vec<BlockSpan>,
    links: Vec<Link>,
    frontmatter: Option<serde_json::Value>,

    slugger: Slugger,
    sanitizer: Sanitizer,
    /// Footnote label to its 1-based number, shared by references and definitions.
    numbers: HashMap<String, usize>,

    table_alignments: Vec<Alignment>,
    table_state: TableState,
    table_cell_index: usize,

    heading: Option<PendingHeading>,
    links_open: Vec<PendingLink>,
    image: Option<PendingImage>,
    /// A paragraph seen but not yet written; see [`Writer::flush_paragraph`].
    pending_para: Option<PendingPara>,
    /// Whether a `<p>` is currently open in the output.
    para_open: bool,
    /// Labels of open footnote definitions, for the backlink on close.
    footnote_labels: Vec<String>,
    /// `Some(line)` while inside a ```mermaid fence.
    mermaid_line: Option<usize>,
}

/// Renders `text` into everything the UI needs.
pub fn write_document(text: &str, opts: &RenderOpts<'_>, encoding: Encoding) -> RenderedDoc {
    let lines = LineIndex::new(text);
    let mut w = Writer {
        // A rendered document runs roughly 1.5x its source. Guessing once beats
        // a dozen reallocations of a multi-megabyte string.
        out: String::with_capacity(text.len() * 3 / 2),
        line_map: Vec::with_capacity(lines.len() / 4 + 8),
        lines,
        opts,
        held: None,
        captures: Vec::new(),
        buf: String::new(),
        mode: TextMode::Normal,
        end_newline: true,
        outline: Vec::new(),
        links: Vec::new(),
        frontmatter: None,
        slugger: Slugger::new(),
        sanitizer: Sanitizer::new(),
        numbers: HashMap::new(),
        table_alignments: Vec::new(),
        table_state: TableState::Head,
        table_cell_index: 0,
        heading: None,
        links_open: Vec::new(),
        image: None,
        pending_para: None,
        para_open: false,
        footnote_labels: Vec::new(),
        mermaid_line: None,
    };

    let parser = Parser::new_ext(text, super::options());
    for (event, range) in parser.into_offset_iter() {
        w.event(event, range);
    }
    w.sanitizer.finish(&mut w.out);

    RenderedDoc {
        html: w.out,
        outline: w.outline,
        line_map: w.line_map,
        links: w.links,
        frontmatter: w.frontmatter,
        encoding,
    }
}

impl Writer<'_> {
    // ---- output plumbing -------------------------------------------------

    fn push(&mut self, s: &str) {
        self.out.push_str(s);
        if !s.is_empty() {
            self.end_newline = s.ends_with('\n');
        }
    }

    /// Recomputes `end_newline` after something was written straight into `out`.
    fn sync_newline(&mut self) {
        self.end_newline = self.out.ends_with('\n');
    }

    /// Starts a block on its own line without emitting a blank one.
    fn block_break(&mut self) {
        if !self.end_newline {
            self.push("\n");
        }
    }

    /// Records a block and returns its 1-based line, for `data-l`.
    ///
    /// Every element that carries `data-l` calls this exactly once, so
    /// `line_map` has one entry per `data-l` in the document, in order.
    fn block(&mut self, range: &Range<usize>) -> usize {
        let line = self.lines.line_of(range.start);
        self.line_map.push(BlockSpan {
            line,
            start_byte: range.start,
            end_byte: range.end,
        });
        line
    }

    /// Closes out any sanitizer state that a block left dangling.
    fn end_block(&mut self) {
        let mut s = String::new();
        self.sanitizer.finish(&mut s);
        self.push(&s);
    }

    /// Writes the `<p>` that [`Tag::Paragraph`] deferred, if it is still owed.
    fn flush_paragraph(&mut self) {
        let Some(p) = self.pending_para.take() else {
            return;
        };
        self.line_map.push(BlockSpan {
            line: p.line,
            start_byte: p.range.start,
            end_byte: p.range.end,
        });
        self.block_break();
        self.out.push_str("<p");
        self.data_l(p.line);
        self.push(">");
        self.para_open = true;
    }

    fn open_block(&mut self, tag: &str, range: &Range<usize>) {
        let line = self.block(range);
        self.block_break();
        self.out.push('<');
        self.out.push_str(tag);
        self.data_l(line);
        self.push(">");
    }

    fn data_l(&mut self, line: usize) {
        self.out.push_str(" data-l=\"");
        self.out.push_str(itoa(line).as_str());
        self.out.push('"');
    }

    // ---- events ----------------------------------------------------------

    fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        // Inside an opaque element the only events that matter are the ones
        // that can close it. Everything else is the payload — including the
        // `Text` event that `pulldown-cmark` produces for the body of an inline
        // `<script>` — and is dropped rather than written out as prose.
        if self.sanitizer.is_skipping()
            && !matches!(
                event,
                Event::Html(_)
                    | Event::InlineHtml(_)
                    | Event::End(TagEnd::HtmlBlock)
                    | Event::End(TagEnd::Paragraph)
            )
        {
            return;
        }

        // The enclosing `<p>` is written lazily: `pulldown-cmark` wraps `$$…$$`
        // in a paragraph, and a `<div class="math-block">` inside a `<p>` is
        // invalid HTML that browsers repair by closing the paragraph early —
        // which desynchronises every `data-l` the frontend indexes. Deferring
        // the opening tag by exactly one event is enough to know whether the
        // paragraph has any inline content to hold.
        if !matches!(
            event,
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) | Event::DisplayMath(_)
        ) {
            self.flush_paragraph();
        }

        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.text(&t),
            Event::Code(t) => {
                if self.mode == TextMode::Normal {
                    self.out.push_str("<code>");
                    escape_html(&t, &mut self.out);
                    self.push("</code>");
                    for c in &mut self.captures {
                        c.push_str(&t);
                    }
                } else {
                    self.text(&t);
                }
            }
            // Rust renders no math. KaTeX runs in the webview, lazily, and only
            // when a document actually contains a placeholder.
            Event::InlineMath(t) => {
                self.out.push_str("<span class=\"math-inline\" data-tex=\"");
                escape_attr(&t, &mut self.out);
                self.push("\"></span>");
            }
            Event::DisplayMath(t) => {
                // Leave the paragraph, emit the block, and arm a fresh one in
                // case inline content follows on the same paragraph.
                if self.para_open {
                    self.push("</p>\n");
                    self.para_open = false;
                }
                self.pending_para = None;

                let line = self.block(&range);
                self.block_break();
                self.out.push_str("<div class=\"math-block\"");
                self.data_l(line);
                self.out.push_str(" data-tex=\"");
                escape_attr(&t, &mut self.out);
                self.push("\"></div>\n");
                self.pending_para = Some(PendingPara {
                    line: self.lines.line_of(range.end),
                    range,
                });
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                self.sanitizer.push(&h, &mut self.out);
                self.sync_newline();
            }
            Event::SoftBreak => {
                if self.mode == TextMode::Normal {
                    self.push("\n");
                    for c in &mut self.captures {
                        c.push(' ');
                    }
                } else {
                    self.buf.push('\n');
                }
            }
            Event::HardBreak => {
                if self.mode == TextMode::Normal {
                    self.push("<br />\n");
                    for c in &mut self.captures {
                        c.push(' ');
                    }
                } else {
                    self.buf.push('\n');
                }
            }
            Event::Rule => {
                let line = self.block(&range);
                self.block_break();
                self.out.push_str("<hr");
                self.data_l(line);
                self.push(" />\n");
            }
            Event::FootnoteReference(name) => {
                let n = self.number_for(&name);
                self.push("<sup class=\"footnote-ref\" id=\"fnref-");
                let mut id = String::new();
                escape_attr(&name, &mut id);
                self.push(&id);
                self.push("\"><a href=\"#fn-");
                self.push(&id);
                self.push("\">");
                self.push(itoa(n).as_str());
                self.push("</a></sup>");
            }
            Event::TaskListMarker(checked) => {
                self.push(if checked {
                    "<input class=\"task\" type=\"checkbox\" disabled=\"\" checked=\"\" />"
                } else {
                    "<input class=\"task\" type=\"checkbox\" disabled=\"\" />"
                });
            }
        }
    }

    fn text(&mut self, t: &str) {
        match self.mode {
            TextMode::Normal => {
                // Straight into `out`: this is the hottest path in the writer,
                // once per text run, and a scratch `String` here is a malloc per
                // run — about a million of them on a 5 MB document.
                escape_html(t, &mut self.out);
                self.end_newline = t.ends_with('\n');
                for c in &mut self.captures {
                    c.push_str(t);
                }
            }
            _ => self.buf.push_str(t),
        }
    }

    fn number_for(&mut self, name: &str) -> usize {
        let next = self.numbers.len() + 1;
        *self.numbers.entry(name.to_string()).or_insert(next)
    }

    // ---- start tags ------------------------------------------------------

    fn start(&mut self, tag: Tag<'_>, range: Range<usize>) {
        match tag {
            Tag::Paragraph => {
                self.pending_para = Some(PendingPara {
                    line: self.lines.line_of(range.start),
                    range,
                })
            }

            Tag::Heading {
                level,
                id,
                classes,
                attrs: _,
            } => {
                // `attrs` is deliberately dropped. `ENABLE_HEADING_ATTRIBUTES`
                // lets a heading carry `{#id .class name=value}`, and `value`
                // comes out of an untrusted file — honouring arbitrary
                // attributes here would hand every `on*` handler a way past the
                // raw-HTML sanitizer, which is the one place they are checked.
                let line = self.block(&range);
                self.block_break();
                self.heading = Some(PendingHeading {
                    level,
                    id: id.map(|i| i.to_string()),
                    classes: classes.iter().map(|c| c.to_string()).collect(),
                    line,
                });
                // Buffer the body: the `id` is a slug of text that has not
                // arrived yet, and the opening tag has to carry it.
                self.held = Some(std::mem::take(&mut self.out));
                self.captures.push(String::new());
                self.end_newline = true;
            }

            Tag::BlockQuote(kind) => {
                let line = self.block(&range);
                self.block_break();
                self.out.push_str("<blockquote");
                if let Some(k) = kind {
                    self.out.push_str(match k {
                        BlockQuoteKind::Note => " class=\"alert alert-note\"",
                        BlockQuoteKind::Tip => " class=\"alert alert-tip\"",
                        BlockQuoteKind::Important => " class=\"alert alert-important\"",
                        BlockQuoteKind::Warning => " class=\"alert alert-warning\"",
                        BlockQuoteKind::Caution => " class=\"alert alert-caution\"",
                    });
                }
                self.data_l(line);
                self.push(">\n");
            }

            Tag::CodeBlock(kind) => {
                let line = self.block(&range);
                self.block_break();
                let lang = match &kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or(""),
                    CodeBlockKind::Indented => "",
                };
                if lang.eq_ignore_ascii_case("mermaid") {
                    // Never highlighted, never rendered here: the source goes
                    // into `data-src` and the webview loads Mermaid lazily, only
                    // for documents that actually contain a diagram.
                    self.mermaid_line = Some(line);
                    self.buf.clear();
                    self.mode = TextMode::Mermaid;
                } else {
                    self.out.push_str("<pre");
                    self.data_l(line);
                    self.out.push_str("><code");
                    if !lang.is_empty() {
                        self.out.push_str(" class=\"language-");
                        let mut s = String::new();
                        escape_attr(lang, &mut s);
                        self.out.push_str(&s);
                        self.out.push('"');
                    }
                    self.push(">");
                }
            }

            // The block itself has no element of its own; its contents go
            // through the sanitizer as they arrive.
            Tag::HtmlBlock => {}

            Tag::List(start) => {
                let line = self.block(&range);
                self.block_break();
                match start {
                    None => self.out.push_str("<ul"),
                    Some(1) => self.out.push_str("<ol"),
                    Some(n) => {
                        self.out.push_str("<ol start=\"");
                        self.out.push_str(itoa(n as usize).as_str());
                        self.out.push('"');
                    }
                }
                self.data_l(line);
                self.push(">\n");
            }
            Tag::Item => self.open_block("li", &range),

            Tag::FootnoteDefinition(name) => {
                let line = self.block(&range);
                let n = self.number_for(&name);
                let mut id = String::new();
                escape_attr(&name, &mut id);
                self.block_break();
                self.out
                    .push_str("<div class=\"footnote-definition\" id=\"fn-");
                self.out.push_str(&id);
                self.out.push('"');
                self.footnote_labels.push(id);
                self.data_l(line);
                self.out.push_str("><sup class=\"footnote-label\">");
                self.out.push_str(itoa(n).as_str());
                self.push("</sup>");
            }

            Tag::Table(alignments) => {
                self.table_alignments = alignments;
                self.open_block("table", &range);
                self.push("\n");
            }
            Tag::TableHead => {
                self.table_state = TableState::Head;
                self.table_cell_index = 0;
                let line = self.block(&range);
                self.out.push_str("<thead><tr");
                self.data_l(line);
                self.push(">");
            }
            Tag::TableRow => {
                self.table_cell_index = 0;
                self.open_block("tr", &range);
            }
            Tag::TableCell => {
                self.push(match self.table_state {
                    TableState::Head => "<th",
                    TableState::Body => "<td",
                });
                self.push(match self.table_alignments.get(self.table_cell_index) {
                    Some(Alignment::Left) => " style=\"text-align: left\">",
                    Some(Alignment::Center) => " style=\"text-align: center\">",
                    Some(Alignment::Right) => " style=\"text-align: right\">",
                    _ => ">",
                });
            }

            Tag::Emphasis => self.push("<em>"),
            Tag::Strong => self.push("<strong>"),
            Tag::Strikethrough => self.push("<del>"),
            Tag::Superscript => self.push("<sup>"),
            Tag::Subscript => self.push("<sub>"),

            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            } => self.start_link(link_type, &dest_url, &title, &range),

            Tag::Image {
                dest_url, title, ..
            } => {
                self.image = Some(PendingImage {
                    dest: dest_url.to_string(),
                    title: title.to_string(),
                });
                self.buf.clear();
                self.mode = TextMode::AltText;
            }

            Tag::MetadataBlock(_) => {
                self.buf.clear();
                self.mode = TextMode::Metadata;
            }

            // Not enabled in `options()`, but the match must stay total so that
            // turning one on is a compile error here rather than silent output.
            Tag::DefinitionList => self.open_block("dl", &range),
            Tag::DefinitionListTitle => self.open_block("dt", &range),
            Tag::DefinitionListDefinition => self.open_block("dd", &range),
        }
    }

    fn start_link(&mut self, link_type: LinkType, dest: &str, title: &str, range: &Range<usize>) {
        let line = self.lines.line_of(range.start);

        if matches!(link_type, LinkType::WikiLink { .. }) {
            let link = wikilink::resolve(dest, self.opts);
            let mut s = String::new();
            wikilink::open_tag(&link, &mut s);
            self.push(&s);
            self.links_open.push(PendingLink {
                href: link.href,
                line,
                wiki: true,
            });
            self.captures.push(String::new());
            return;
        }

        let href = match link_type {
            LinkType::Email => Some(format!("mailto:{dest}")),
            _ if is_safe_url(dest) => Some(dest.to_string()),
            // A rejected destination loses its `href` and keeps its text: the
            // words the author wrote still read, the payload is simply not
            // reachable. Removing the whole element would make a document look
            // corrupted instead of defused.
            _ => None,
        };

        self.push("<a");
        if let Some(h) = &href {
            let mut s = String::new();
            escape_attr(h, &mut s);
            self.push(" href=\"");
            self.push(&s);
            self.push("\"");
        } else {
            self.push(" class=\"blocked\"");
        }
        if !title.is_empty() {
            let mut s = String::new();
            escape_attr(title, &mut s);
            self.push(" title=\"");
            self.push(&s);
            self.push("\"");
        }
        self.push(">");

        self.links_open.push(PendingLink {
            href,
            line,
            wiki: false,
        });
        self.captures.push(String::new());
    }

    // ---- end tags --------------------------------------------------------

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.end_block();
                if self.para_open {
                    self.push("</p>\n");
                    self.para_open = false;
                } else {
                    // Nothing was ever written into it — a paragraph that held
                    // only block math, or nothing at all.
                    self.pending_para = None;
                }
            }

            TagEnd::Heading(_) => self.finish_heading(),

            TagEnd::BlockQuote(_) => self.push("</blockquote>\n"),

            TagEnd::CodeBlock => {
                if let Some(line) = self.mermaid_line.take() {
                    self.mode = TextMode::Normal;
                    let src = std::mem::take(&mut self.buf);
                    self.out.push_str("<div class=\"mermaid\"");
                    self.data_l(line);
                    self.out.push_str(" data-src=\"");
                    let mut s = String::new();
                    escape_attr(&src, &mut s);
                    self.out.push_str(&s);
                    self.push("\"></div>\n");
                } else {
                    self.push("</code></pre>\n");
                }
            }

            TagEnd::HtmlBlock => self.end_block(),

            TagEnd::List(ordered) => self.push(if ordered { "</ol>\n" } else { "</ul>\n" }),
            TagEnd::Item => self.push("</li>\n"),

            TagEnd::FootnoteDefinition => {
                // The backlink is the half of a footnote that usually breaks:
                // without it a reader who followed a reference has no way back.
                // `TagEnd` carries no label, so the label is kept on a stack —
                // footnote definitions do not nest, but a stack costs nothing
                // and searching the output backwards would be quadratic.
                let label = self.footnote_labels.pop().unwrap_or_default();
                self.push("<a class=\"footnote-backref\" href=\"#fnref-");
                self.push(&label);
                self.push("\">\u{21A9}</a></div>\n");
            }

            TagEnd::Table => self.push("</tbody></table>\n"),
            TagEnd::TableHead => {
                self.push("</tr></thead><tbody>\n");
                self.table_state = TableState::Body;
            }
            TagEnd::TableRow => self.push("</tr>\n"),
            TagEnd::TableCell => {
                self.push(match self.table_state {
                    TableState::Head => "</th>",
                    TableState::Body => "</td>",
                });
                self.table_cell_index += 1;
            }

            TagEnd::Emphasis => self.push("</em>"),
            TagEnd::Strong => self.push("</strong>"),
            TagEnd::Strikethrough => self.push("</del>"),
            TagEnd::Superscript => self.push("</sup>"),
            TagEnd::Subscript => self.push("</sub>"),

            TagEnd::Link => self.finish_link(),
            TagEnd::Image => self.finish_image(),

            TagEnd::MetadataBlock(_) => {
                self.mode = TextMode::Normal;
                let raw = std::mem::take(&mut self.buf);
                self.frontmatter = parse_frontmatter(&raw);
            }

            TagEnd::DefinitionList => self.push("</dl>\n"),
            TagEnd::DefinitionListTitle => self.push("</dt>\n"),
            TagEnd::DefinitionListDefinition => self.push("</dd>\n"),
        }
    }

    fn finish_heading(&mut self) {
        let Some(pending) = self.heading.take() else {
            return;
        };
        let body = match self.held.take() {
            Some(real) => std::mem::replace(&mut self.out, real),
            None => std::mem::take(&mut self.out),
        };
        let text = self.captures.pop().unwrap_or_default();
        let text = text.trim().to_string();

        // One call decides both the emitted `id` and the outline's `slug`, so
        // they cannot drift apart. An explicit `{#id}` wins but still takes part
        // in de-duplication, or two of them would silently collide.
        let slug = match pending.id.as_deref() {
            Some(id) if !id.is_empty() => self.slugger.unique(id.to_string()),
            _ => self.slugger.slug(&text),
        };

        let level = pending.level as u8;
        self.out.push_str("<h");
        self.out.push_str(itoa(level as usize).as_str());
        self.out.push_str(" id=\"");
        let mut s = String::new();
        escape_attr(&slug, &mut s);
        self.out.push_str(&s);
        self.out.push('"');
        if !pending.classes.is_empty() {
            self.out.push_str(" class=\"");
            let joined = pending.classes.join(" ");
            let mut c = String::new();
            escape_attr(&joined, &mut c);
            self.out.push_str(&c);
            self.out.push('"');
        }
        self.data_l(pending.line);
        self.out.push('>');
        self.out.push_str(&body);
        self.out.push_str("</h");
        self.out.push_str(itoa(level as usize).as_str());
        self.push(">\n");

        self.outline.push(Heading {
            level,
            text,
            slug,
            line: pending.line,
        });
    }

    fn finish_link(&mut self) {
        self.push("</a>");
        let text = self.captures.pop().unwrap_or_default();
        if let Some(p) = self.links_open.pop() {
            self.links.push(Link {
                href: p.href.clone().unwrap_or_default(),
                text,
                line: p.line,
                wiki: p.wiki,
                resolved: p.href.is_some(),
            });
        }
    }

    fn finish_image(&mut self) {
        self.mode = TextMode::Normal;
        let alt = std::mem::take(&mut self.buf);
        let Some(img) = self.image.take() else {
            return;
        };

        let src = self.resolve_asset(&img.dest);
        self.push("<img");
        if let Some(s) = &src {
            let mut e = String::new();
            escape_attr(s, &mut e);
            self.push(" src=\"");
            self.push(&e);
            self.push("\"");
        }
        let mut a = String::new();
        escape_attr(&alt, &mut a);
        self.push(" alt=\"");
        self.push(&a);
        self.push("\"");
        if !img.title.is_empty() {
            let mut t = String::new();
            escape_attr(&img.title, &mut t);
            self.push(" title=\"");
            self.push(&t);
            self.push("\"");
        }
        self.push(" />");

        for c in &mut self.captures {
            c.push_str(&alt);
        }
    }

    /// Applies `asset_base` to a relative image destination.
    ///
    /// Absolute URLs and fragments are left alone; anything that fails the URL
    /// check loses its `src` entirely and renders as its alt text.
    fn resolve_asset(&self, dest: &str) -> Option<String> {
        if !is_safe_url(dest) {
            return None;
        }
        let relative = !dest.contains(':') && !dest.starts_with('/') && !dest.starts_with('#');
        match (&self.opts.asset_base, relative) {
            (Some(base), true) => {
                let base = base.as_ref();
                let sep = if base.ends_with('/') { "" } else { "/" };
                let dest = dest.strip_prefix("./").unwrap_or(dest);
                Some(format!("{base}{sep}{dest}"))
            }
            _ => Some(dest.to_string()),
        }
    }
}

/// A tiny integer formatter, so `data-l` does not allocate a `String` per block.
fn itoa(mut n: usize) -> ArrayStr {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    ArrayStr { buf, start: i }
}

struct ArrayStr {
    buf: [u8; 20],
    start: usize,
}

impl ArrayStr {
    fn as_str(&self) -> &str {
        // Every byte written is an ASCII digit, so this cannot fail.
        std::str::from_utf8(&self.buf[self.start..]).unwrap_or("0")
    }
}

/// Parses the flat subset of YAML that markdown frontmatter actually uses.
///
/// Supported: `key: scalar`, `key: [a, b]`, and a block sequence of `- item`
/// under a bare `key:`. Scalars are typed as bool, integer, float or string.
///
/// Deliberately not supported: nested mappings, anchors, multi-line scalars,
/// flow mappings. A real YAML parser is 200–400 KB for metadata that is, in
/// practice, six lines of `title:` and `tags:`. Anything unrecognised is
/// skipped rather than guessed at, and a block that yields nothing produces
/// `None` rather than an empty object.
pub fn parse_frontmatter(raw: &str) -> Option<serde_json::Value> {
    use serde_json::{Map, Value};

    let mut map = Map::new();
    let mut pending_key: Option<String> = None;
    let mut pending_seq: Vec<Value> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed == "---" {
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            if pending_key.is_some() {
                pending_seq.push(scalar(item.trim()));
                continue;
            }
        }

        if let Some(key) = pending_key.take() {
            map.insert(key, Value::Array(std::mem::take(&mut pending_seq)));
        }

        let Some((key, rest)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let rest = rest.trim();

        if rest.is_empty() {
            pending_key = Some(key.to_string());
        } else if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            let items = inner
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(scalar)
                .collect();
            map.insert(key.to_string(), Value::Array(items));
        } else {
            map.insert(key.to_string(), scalar(rest));
        }
    }

    if let Some(key) = pending_key {
        map.insert(key, Value::Array(pending_seq));
    }

    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

fn scalar(s: &str) -> serde_json::Value {
    use serde_json::Value;

    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        return Value::String(s[1..s.len() - 1].to_string());
    }
    match s {
        "true" | "yes" => return Value::Bool(true),
        "false" | "no" => return Value::Bool(false),
        "null" | "~" | "" => return Value::Null,
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::from(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() {
            return Value::from(f);
        }
    }
    Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_is_one_based() {
        let idx = LineIndex::new("a\nbb\n\nccc");
        assert_eq!(idx.line_of(0), 1);
        assert_eq!(idx.line_of(1), 1);
        assert_eq!(idx.line_of(2), 2);
        assert_eq!(idx.line_of(5), 3);
        assert_eq!(idx.line_of(6), 4);
    }

    #[test]
    fn itoa_matches_std() {
        for n in [0usize, 1, 9, 10, 99, 100, 12345, usize::MAX] {
            assert_eq!(itoa(n).as_str(), n.to_string());
        }
    }

    #[test]
    fn frontmatter_subset() {
        let v = parse_frontmatter("title: Kitchen sink\ntags: [fixture, smoke]\ndraft: false\n")
            .expect("some frontmatter");
        assert_eq!(v["title"], "Kitchen sink");
        assert_eq!(v["tags"][1], "smoke");
        assert_eq!(v["draft"], false);
    }

    #[test]
    fn frontmatter_block_sequence() {
        let v = parse_frontmatter("tags:\n  - one\n  - two\nn: 3\n").expect("some frontmatter");
        assert_eq!(v["tags"][0], "one");
        assert_eq!(v["n"], 3);
    }

    #[test]
    fn empty_frontmatter_is_none() {
        assert!(parse_frontmatter("\n\n").is_none());
    }
}

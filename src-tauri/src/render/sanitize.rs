//! The allowlist filter for raw HTML, and the URL check every href and src passes.
//!
//! A `.md` file from the internet is untrusted input and it ends up as
//! `innerHTML` in a process that can reach the filesystem. This module is the
//! first of the four layers described in `docs/architecture.md`, and it is an
//! **allowlist**: an unknown tag is dropped because it is unknown, not because
//! it appeared on a list of bad ones. Blocklists lose to the payload nobody
//! thought of; allowlists lose only to a mistake in the list itself, which is
//! eight names long and sitting right here.
//!
//! `ammonia` would do this too, and it pulls `html5ever` for roughly +1.2 MB —
//! 40% of the entire lite installer ceiling. It is denied by name in
//! `deny.toml`, so it cannot come back transitively either.

/// The only tags that survive. Adding one means adding fixtures for it,
/// including adversarial ones.
pub const ALLOWED_TAGS: &[&str] = &[
    "br", "img", "details", "summary", "sub", "sup", "kbd", "mark",
];

/// The only attributes that survive, on any allowed tag.
///
/// `href` is deliberately absent: `a` is not an allowed raw tag, so a raw
/// `<a href="javascript:…">` loses both the tag and the attribute. Markdown's
/// own links go through [`is_safe_url`] in the writer instead.
pub const ALLOWED_ATTRS: &[&str] = &["src", "alt", "title", "width", "height", "open"];

/// Allowed tags that never have a closing tag.
const VOID_TAGS: &[&str] = &["br", "img"];

/// Tags whose *content* is code rather than prose, so dropping the tag alone
/// would leave the payload visible as text. Their children are swallowed too.
const OPAQUE_TAGS: &[&str] = &["script", "style"];

/// URL schemes that may appear in a `src`, an `href` or an image destination.
///
/// `data:` is handled separately because only `data:image/*` is allowed.
const SAFE_SCHEMES: &[&str] = &["http", "https", "mailto", "marklet", "ftp", "ftps", "tel"];

/// Appends `s` to `out` with the three characters that can end a text node escaped.
///
/// Copies the runs between escapes as slices rather than character by character.
/// This is the writer's hottest function — it runs once per text event, which is
/// hundreds of thousands of times on a large document — and prose contains
/// almost none of these three bytes, so the common case is one `push_str` of the
/// whole string.
pub fn escape_html(s: &str, out: &mut String) {
    out.reserve(s.len());
    let mut last = 0;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        let replacement = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            _ => continue,
        };
        // All three are ASCII, so `i` is always a char boundary.
        out.push_str(&s[last..i]);
        out.push_str(replacement);
        last = i + 1;
    }
    out.push_str(&s[last..]);
}

/// Appends `s` to `out` escaped for use inside a double-quoted attribute value.
///
/// Single quotes are escaped as well even though we always emit double quotes:
/// the value may be read back out of the DOM and re-inserted elsewhere, and one
/// escaping rule for both quote styles is one fewer thing to get wrong.
pub fn escape_attr(s: &str, out: &mut String) {
    out.reserve(s.len());
    let mut last = 0;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        let replacement = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&s[last..i]);
        out.push_str(replacement);
        last = i + 1;
    }
    out.push_str(&s[last..]);
}

/// True when `url` may be emitted as a destination.
///
/// Relative URLs pass. Absolute ones pass only with a known-safe scheme, plus
/// `data:image/*` so that inline images keep working.
///
/// The scheme is read from a normalised copy with ASCII whitespace and control
/// characters removed, because `java\nscript:` and `java\tscript:` are both
/// live URLs in a browser. Any `&` in that prefix is an immediate reject: no
/// real scheme contains one, and its only use there is to smuggle a character
/// reference such as `java&#115;cript:` past a naive prefix comparison. That is
/// cheaper and more certain than shipping an entity decoder for the purpose.
pub fn is_safe_url(url: &str) -> bool {
    let mut prefix = String::new();
    let mut has_scheme = false;

    for c in url.chars() {
        if c.is_ascii_whitespace() || (c as u32) < 0x20 || c as u32 == 0x7F {
            continue;
        }
        match c {
            ':' => {
                has_scheme = true;
                break;
            }
            // A '/', '?' or '#' before any ':' means the URL is relative.
            '/' | '?' | '#' => break,
            _ => prefix.extend(c.to_lowercase()),
        }
    }

    if prefix.contains('&') {
        return false;
    }
    if !has_scheme {
        return true;
    }
    if SAFE_SCHEMES.contains(&prefix.as_str()) {
        return true;
    }
    if prefix == "data" {
        let rest: String = url
            .chars()
            .skip_while(|&c| c != ':')
            .skip(1)
            .filter(|c| !c.is_ascii_whitespace())
            .flat_map(char::to_lowercase)
            .take(6)
            .collect();
        return rest == "image/";
    }
    false
}

/// Filters a stream of raw HTML fragments against the allowlist.
///
/// It is a struct rather than a function because `pulldown-cmark` delivers an
/// HTML block one **line** at a time, so `<img\n  src="x">` arrives as two
/// events. State that spans events — a half-read tag, an unterminated comment,
/// the inside of a `<script>` — has to live somewhere, and a stateless filter
/// would be trivially defeated by putting a newline in the middle of a tag.
#[derive(Debug, Default)]
pub struct Sanitizer {
    /// Text of a tag or comment that had not closed when the fragment ended.
    carry: String,
    /// Set while inside an opaque element; holds the tag name being waited on.
    skip: Option<&'static str>,
    /// Set while inside an unterminated `<!-- … -->`.
    in_comment: bool,
}

impl Sanitizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters one fragment, appending what survives to `out`.
    pub fn push(&mut self, fragment: &str, out: &mut String) {
        let input = if self.carry.is_empty() {
            fragment.to_string()
        } else {
            let mut s = std::mem::take(&mut self.carry);
            s.push_str(fragment);
            s
        };
        self.scan(&input, out);
    }

    /// True while the filter is inside an opaque element or a comment.
    ///
    /// The writer has to ask, because an inline `<script>` does not deliver its
    /// body through this filter at all: `pulldown-cmark` splits
    /// `<script>alert(1)</script>` into two `InlineHtml` events with an ordinary
    /// `Text` event between them, and that `Text` would otherwise be written
    /// straight out as visible prose.
    pub fn is_skipping(&self) -> bool {
        self.skip.is_some() || self.in_comment
    }

    /// Flushes anything still buffered. An unterminated tag is text, escaped.
    ///
    /// Also called at the end of every block, which bounds the blast radius of
    /// an unclosed `<script>` or `<!--` to the block it opened in. Letting it
    /// run to the end of the file would be safe but would silently blank the
    /// rest of the document, and a viewer that shows nothing is indistinguishable
    /// from one that crashed.
    pub fn finish(&mut self, out: &mut String) {
        let carry = std::mem::take(&mut self.carry);
        if !carry.is_empty() && self.skip.is_none() {
            escape_html(&carry, out);
        }
        self.in_comment = false;
        self.skip = None;
    }

    fn scan(&mut self, input: &str, out: &mut String) {
        let bytes = input.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if self.in_comment {
                match find(bytes, i, b"-->") {
                    Some(end) => {
                        self.in_comment = false;
                        i = end + 3;
                    }
                    None => return, // whole rest is comment; drop it
                }
                continue;
            }

            if let Some(name) = self.skip {
                // Inside <script>/<style>: swallow until the matching close.
                match find_close(bytes, i, name) {
                    Some((start, end)) => {
                        let _ = start;
                        self.skip = None;
                        i = end;
                    }
                    None => return, // drop the rest of the fragment
                }
                continue;
            }

            let Some(lt) = memchr_lt(bytes, i) else {
                out.push_str(&input[i..]);
                return;
            };

            out.push_str(&input[i..lt]);

            if input[lt..].starts_with("<!--") {
                self.in_comment = true;
                i = lt + 4;
                continue;
            }

            match parse_tag(input, lt) {
                TagScan::Incomplete => {
                    // The tag runs past the end of this fragment. Hold it for
                    // the next one rather than guessing at its shape.
                    self.carry.push_str(&input[lt..]);
                    return;
                }
                TagScan::NotATag => {
                    out.push_str("&lt;");
                    i = lt + 1;
                }
                TagScan::Tag { tag, end } => {
                    self.emit(&tag, out);
                    i = end;
                }
            }
        }
    }

    fn emit(&mut self, tag: &ParsedTag, out: &mut String) {
        if tag.closing {
            if ALLOWED_TAGS.contains(&tag.name.as_str()) && !VOID_TAGS.contains(&tag.name.as_str())
            {
                out.push_str("</");
                out.push_str(&tag.name);
                out.push('>');
            }
            return;
        }

        if let Some(opaque) = OPAQUE_TAGS.iter().find(|t| **t == tag.name) {
            if !tag.self_closing {
                self.skip = Some(opaque);
            }
            return;
        }

        if !ALLOWED_TAGS.contains(&tag.name.as_str()) {
            return;
        }

        out.push('<');
        out.push_str(&tag.name);
        for (name, value) in &tag.attrs {
            if !ALLOWED_ATTRS.contains(&name.as_str()) {
                continue;
            }
            match value {
                Some(v) => {
                    if (name == "src" || name == "href") && !is_safe_url(v) {
                        continue;
                    }
                    out.push(' ');
                    out.push_str(name);
                    out.push_str("=\"");
                    escape_attr(v, out);
                    out.push('"');
                }
                // Boolean attribute, e.g. `open` on `<details>`.
                None => {
                    out.push(' ');
                    out.push_str(name);
                    out.push_str("=\"\"");
                }
            }
        }
        if VOID_TAGS.contains(&tag.name.as_str()) {
            out.push_str(" />");
        } else {
            out.push('>');
        }
    }
}

struct ParsedTag {
    name: String,
    closing: bool,
    self_closing: bool,
    attrs: Vec<(String, Option<String>)>,
}

enum TagScan {
    Tag {
        tag: ParsedTag,
        end: usize,
    },
    /// A `<` that does not begin a tag, e.g. `a < b`.
    NotATag,
    /// A tag that has not closed before the end of the fragment.
    Incomplete,
}

fn memchr_lt(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|&b| b == b'<')
        .map(|p| p + from)
}

fn find(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= bytes.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Finds `</name…>` from `from`, returning `(start, end_exclusive)`.
fn find_close(bytes: &[u8], from: usize, name: &str) -> Option<(usize, usize)> {
    let mut i = from;
    while let Some(lt) = memchr_lt(bytes, i) {
        let after = lt + 1;
        if bytes.get(after) == Some(&b'/') {
            let start = after + 1;
            let end = start + name.len();
            if bytes.len() >= end && bytes[start..end].eq_ignore_ascii_case(name.as_bytes()) {
                if let Some(gt) = bytes[end..].iter().position(|&b| b == b'>') {
                    return Some((lt, end + gt + 1));
                }
                return None;
            }
        }
        i = lt + 1;
    }
    None
}

/// Parses the tag starting at `lt` (which must index a `<`).
fn parse_tag(input: &str, lt: usize) -> TagScan {
    let bytes = input.as_bytes();
    let mut i = lt + 1;

    // `<!DOCTYPE …>` and `<?php …?>` are dropped whole.
    if matches!(bytes.get(i), Some(b'!') | Some(b'?')) {
        return match bytes[i..].iter().position(|&b| b == b'>') {
            Some(p) => TagScan::Tag {
                tag: ParsedTag {
                    name: String::new(),
                    closing: false,
                    self_closing: true,
                    attrs: Vec::new(),
                },
                end: i + p + 1,
            },
            None => TagScan::Incomplete,
        };
    }

    let closing = bytes.get(i) == Some(&b'/');
    if closing {
        i += 1;
    }

    let name_start = i;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    if i == name_start {
        return TagScan::NotATag;
    }
    let name = input[name_start..i].to_ascii_lowercase();

    let mut attrs = Vec::new();
    let mut self_closing = false;

    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => return TagScan::Incomplete,
            Some(b'>') => {
                i += 1;
                break;
            }
            Some(b'/') => {
                self_closing = true;
                i += 1;
                continue;
            }
            Some(_) => {}
        }

        let attr_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        if i == attr_start {
            // A character we do not understand where a name should be; skip it
            // rather than looping forever on it.
            i += 1;
            continue;
        }
        let attr_name = input[attr_start..i].to_ascii_lowercase();

        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if bytes.get(i) == Some(&b'=') {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            match bytes.get(i) {
                None => return TagScan::Incomplete,
                Some(q @ (b'"' | b'\'')) => {
                    let quote = *q;
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        return TagScan::Incomplete;
                    }
                    let v = input[start..i].to_string();
                    i += 1;
                    Some(v)
                }
                Some(_) => {
                    let start = i;
                    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                        i += 1;
                    }
                    Some(input[start..i].to_string())
                }
            }
        } else {
            None
        };

        attrs.push((attr_name, value));
    }

    TagScan::Tag {
        tag: ParsedTag {
            name,
            closing,
            self_closing,
            attrs,
        },
        end: i,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(s: &str) -> String {
        let mut out = String::new();
        let mut san = Sanitizer::new();
        san.push(s, &mut out);
        san.finish(&mut out);
        out
    }

    #[test]
    fn keeps_allowed_tags_and_attrs() {
        assert_eq!(
            filter(r#"<img src="a.png" alt="A" width="10">"#),
            r#"<img src="a.png" alt="A" width="10" />"#
        );
        assert_eq!(filter("<mark>hi</mark>"), "<mark>hi</mark>");
        assert_eq!(filter("<details open>"), r#"<details open="">"#);
    }

    #[test]
    fn drops_unknown_tags_but_keeps_their_text() {
        assert_eq!(filter("<div class=x>text</div>"), "text");
        assert_eq!(filter("<iframe src=evil>"), "");
    }

    #[test]
    fn drops_event_handlers() {
        let out = filter(r#"<img src="x" onerror="alert(1)">"#);
        assert!(!out.contains("onerror"), "got {out}");
        assert_eq!(out, r#"<img src="x" />"#);
    }

    #[test]
    fn swallows_script_content() {
        assert_eq!(filter("<svg><script>alert(1)</script></svg>"), "");
        assert_eq!(filter("<style>body{x}</style>after"), "after");
    }

    #[test]
    fn drops_srcdoc_and_javascript_src() {
        assert_eq!(filter(r#"<iframe srcdoc="<script>x</script>">"#), "");
        assert_eq!(filter(r#"<img src="javascript:alert(1)">"#), "<img />");
    }

    #[test]
    fn a_bare_less_than_is_escaped() {
        assert_eq!(filter("a < b"), "a &lt; b");
    }

    #[test]
    fn a_tag_split_across_fragments_is_still_filtered() {
        let mut out = String::new();
        let mut san = Sanitizer::new();
        san.push("<img\n", &mut out);
        san.push("  onerror=alert(1) src=\"a.png\">\n", &mut out);
        san.finish(&mut out);
        assert_eq!(out, "<img src=\"a.png\" />\n");
    }

    #[test]
    fn comments_are_dropped_across_fragments() {
        let mut out = String::new();
        let mut san = Sanitizer::new();
        san.push("<!-- start\n", &mut out);
        san.push("still comment\n", &mut out);
        san.push("end --> after\n", &mut out);
        san.finish(&mut out);
        assert_eq!(out, " after\n");
    }

    #[test]
    fn url_scheme_checks() {
        assert!(is_safe_url("./img/a.png"));
        assert!(is_safe_url("https://example.com"));
        assert!(is_safe_url("#anchor"));
        assert!(is_safe_url("data:image/png;base64,AAAA"));
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("JaVaScRiPt:alert(1)"));
        assert!(!is_safe_url("java\nscript:alert(1)"));
        assert!(!is_safe_url("java&#115;cript:alert(1)"));
        assert!(!is_safe_url("data:text/html,<script>x</script>"));
        assert!(!is_safe_url("vbscript:msgbox(1)"));
    }

    #[test]
    fn unterminated_tag_becomes_text() {
        assert_eq!(filter("<img src=\"a"), "&lt;img src=\"a");
    }
}

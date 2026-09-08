//! `[[Note]]` resolution.
//!
//! The render core does not know what a vault is, and must not learn: it runs
//! headless for `MD_HTML=1`, where there may be no vault at all. Resolution is a
//! callback on [`RenderOpts`](super::RenderOpts), so the same function renders a
//! note inside a vault, a loose file on the desktop, and a fixture in a test.
//!
//! `None` from the callback is a **normal** answer. A wiki-link to a note you
//! have not written yet is how vaults are used; it renders as a styled
//! unresolved link, not as an error and not as plain text.

use super::sanitize::{escape_attr, is_safe_url};
use super::RenderOpts;

/// A wiki-link after resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    /// The target as written, minus any `|display` part. Kept on the element as
    /// `data-target` in both states so the UI can offer "create this note".
    pub target: String,
    /// The href, or `None` when the target does not exist.
    pub href: Option<String>,
}

impl WikiLink {
    pub fn resolved(&self) -> bool {
        self.href.is_some()
    }
}

/// Resolves `dest` through the callback in `opts`.
///
/// `pulldown-cmark` already strips the `|display` half of `[[target|display]]`
/// and hands us the target, so the only normalisation left is trimming.
pub fn resolve(dest: &str, opts: &RenderOpts<'_>) -> WikiLink {
    let target = dest.trim().to_string();
    let href = opts
        .wiki_resolver
        .and_then(|f| f(&target))
        // A resolver is host code, but it reads names out of a `.md` file. A
        // callback tricked into returning `javascript:` would put an XSS behind
        // the one link type that bypasses the raw-HTML sanitizer.
        .filter(|h| is_safe_url(h));

    WikiLink { target, href }
}

/// Writes the opening `<a>` for a wiki-link.
///
/// There is no matching `close_tag`: a wiki-link closes with a plain `</a>` like
/// every other link, and the writer's one link-closing path handles both kinds.
pub fn open_tag(link: &WikiLink, out: &mut String) {
    match &link.href {
        Some(href) => {
            out.push_str("<a class=\"wikilink\" href=\"");
            escape_attr(href, out);
            out.push_str("\" data-target=\"");
        }
        // No `href` at all, deliberately: an unresolved link must not be
        // clickable-to-nowhere, and the absence of the attribute is what the
        // stylesheet and the "create note" affordance both key off.
        None => out.push_str("<a class=\"wikilink unresolved\" data-target=\""),
    }
    escape_attr(&link.target, out);
    out.push_str("\">");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_keeps_the_target_and_drops_the_href() {
        let link = resolve("Missing Note", &RenderOpts::default());
        assert!(!link.resolved());
        let mut out = String::new();
        open_tag(&link, &mut out);
        assert_eq!(
            out,
            r#"<a class="wikilink unresolved" data-target="Missing Note">"#
        );
    }

    #[test]
    fn resolved_gets_an_href() {
        let f = |t: &str| Some(format!("marklet://vault/{t}.md"));
        let opts = RenderOpts {
            wiki_resolver: Some(&f),
            ..RenderOpts::default()
        };
        let link = resolve("Note", &opts);
        let mut out = String::new();
        open_tag(&link, &mut out);
        assert_eq!(
            out,
            r#"<a class="wikilink" href="marklet://vault/Note.md" data-target="Note">"#
        );
    }

    #[test]
    fn a_resolver_returning_a_script_url_is_treated_as_unresolved() {
        let f = |_: &str| Some("javascript:alert(1)".to_string());
        let opts = RenderOpts {
            wiki_resolver: Some(&f),
            ..RenderOpts::default()
        };
        let link = resolve("Note", &opts);
        assert!(!link.resolved());
    }

    #[test]
    fn quotes_in_a_target_cannot_break_out_of_the_attribute() {
        let link = resolve(r#"a" onmouseover="x"#, &RenderOpts::default());
        let mut out = String::new();
        open_tag(&link, &mut out);
        assert!(!out.contains("onmouseover=\""), "got {out}");
        assert!(out.contains("&quot;"), "got {out}");
    }
}

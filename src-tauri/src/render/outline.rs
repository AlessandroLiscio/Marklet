//! Heading slugs and the outline.
//!
//! The slug is GitHub-compatible because people paste GitHub anchors into
//! Marklet and expect them to land. More importantly, the `id` emitted on the
//! heading and the `slug` recorded in the outline come from **one** function
//! call per heading, so they cannot drift: a mismatch between them shows up as
//! an outline entry that jumps nowhere, which is the kind of bug that survives
//! for months because nothing fails.

use std::collections::HashMap;

use super::Heading;

/// Assigns unique, GitHub-compatible ids to headings in document order.
///
/// Duplicates get `-1`, `-2`, … appended, matching GitHub. Order matters, so
/// one `Slugger` lives for one render and is never reused.
#[derive(Debug, Default)]
pub struct Slugger {
    seen: HashMap<String, usize>,
}

impl Slugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the unique slug for `text`, remembering it for later collisions.
    pub fn slug(&mut self, text: &str) -> String {
        self.unique(slugify(text))
    }

    /// De-duplicates an already-formed id.
    ///
    /// Used for an explicit `{#custom-id}`, which is taken verbatim but still
    /// has to take part in de-duplication — two identical explicit ids in one
    /// document would otherwise produce two elements with the same `id`, and
    /// the second anchor would be unreachable.
    pub fn unique(&mut self, base: String) -> String {
        match self.seen.get_mut(&base) {
            Some(n) => {
                *n += 1;
                format!("{base}-{n}")
            }
            None => {
                self.seen.insert(base.clone(), 0);
                base
            }
        }
    }
}

/// The GitHub slug rules, without the dedupe pass.
///
/// Lowercase; drop everything that is not a letter, a digit, a space, a hyphen
/// or an underscore; spaces to hyphens. Unicode letters and digits are kept —
/// `## Übersicht` has to anchor, and stripping non-ASCII would leave an empty
/// slug for whole languages.
pub fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

/// One node of the outline as a tree, derived from the flat heading list.
///
/// `RenderedDoc::outline` stays flat because the flat form is what the line-map
/// binary search wants and what serialises smallest. The sidebar needs nesting,
/// so it is built here rather than in TypeScript, where the level-skipping
/// rules would have to be re-implemented.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutlineNode {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub line: usize,
    pub children: Vec<OutlineNode>,
}

/// Nests a flat heading list.
///
/// A skipped level (`#` straight to `###`) nests rather than being promoted:
/// the author's structure is reported as written, because "fixing" it would
/// make the outline disagree with the document.
pub fn nest(headings: &[Heading]) -> Vec<OutlineNode> {
    let mut roots: Vec<OutlineNode> = Vec::new();
    // Path of indices from the root down to the last node inserted.
    let mut path: Vec<usize> = Vec::new();

    for h in headings {
        let node = OutlineNode {
            level: h.level,
            text: h.text.clone(),
            slug: h.slug.clone(),
            line: h.line,
            children: Vec::new(),
        };

        while let Some(&_last) = path.last() {
            if level_at(&roots, &path) >= h.level {
                path.pop();
            } else {
                break;
            }
        }

        if path.is_empty() {
            roots.push(node);
            path.push(roots.len() - 1);
        } else {
            let parent = node_at_mut(&mut roots, &path);
            parent.children.push(node);
            let idx = parent.children.len() - 1;
            path.push(idx);
        }
    }

    roots
}

fn level_at(roots: &[OutlineNode], path: &[usize]) -> u8 {
    let mut cur = &roots[path[0]];
    for &i in &path[1..] {
        cur = &cur.children[i];
    }
    cur.level
}

fn node_at_mut<'a>(roots: &'a mut [OutlineNode], path: &[usize]) -> &'a mut OutlineNode {
    let mut cur = &mut roots[path[0]];
    for &i in &path[1..] {
        cur = &mut cur.children[i];
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_slug_rules() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("data-l is a contract"), "data-l-is-a-contract");
        assert_eq!(slugify("C++ / Rust?"), "c--rust");
        assert_eq!(slugify("Übersicht"), "übersicht");
        assert_eq!(slugify("  spaced  out  "), "--spaced--out--");
    }

    #[test]
    fn duplicates_get_numeric_suffixes() {
        let mut s = Slugger::new();
        assert_eq!(s.slug("Notes"), "notes");
        assert_eq!(s.slug("Notes"), "notes-1");
        assert_eq!(s.slug("Notes"), "notes-2");
        assert_eq!(s.slug("Other"), "other");
    }

    fn h(level: u8, text: &str) -> Heading {
        Heading {
            level,
            text: text.into(),
            slug: slugify(text),
            line: 1,
        }
    }

    #[test]
    fn nesting_follows_levels() {
        let flat = vec![h(1, "A"), h(2, "B"), h(3, "C"), h(2, "D"), h(1, "E")];
        let tree = nest(&flat);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert_eq!(tree[1].children.len(), 0);
    }

    #[test]
    fn a_skipped_level_still_nests() {
        let tree = nest(&[h(1, "A"), h(3, "C")]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn a_document_starting_deep_has_multiple_roots() {
        let tree = nest(&[h(3, "A"), h(3, "B")]);
        assert_eq!(tree.len(), 2);
    }
}

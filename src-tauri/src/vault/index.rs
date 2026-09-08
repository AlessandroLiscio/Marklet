//! Titles, aliases, wiki-links and **backlinks**.
//!
//! There is no search index here, on purpose — see [`super::search`]. What this
//! module keeps is the small graph that cannot be recomputed per keystroke: the
//! name of every note and who links to it.
//!
//! **The link edges are a by-product of parsing, not a second pass.**
//! [`render::render`](crate::render::render) already reports every wiki-link in
//! `RenderedDoc.links`, with its source line. Handing it a resolver that returns
//! the target unchanged makes `Link.href` *be* the target, so one render per
//! note yields the title, the frontmatter and the whole outgoing edge list
//! together. A second parse to find `[[…]]` with a regex would be both slower
//! and a different answer inside code fences.
//!
//! **The cache is a convenience, never a source of truth.** It is one JSON file
//! keyed by the vault root, invalidated per file by `(mtime, size)`. A missing,
//! truncated, or stale cache costs a rebuild and nothing else; there is no
//! repair path because there is nothing to repair.
//!
//! ## What a note costs in memory
//!
//! Per note: the relative path, the title, any aliases, and one `(target, line)`
//! per outgoing wiki-link — plus one `u32` in each map it appears in. Nothing
//! keeps file *content*. [`Index::memory_bytes`] measures it, and a test asserts
//! a 5 000-note vault stays far under the 30 MB the vault is allowed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::protocol::AssetRoot;
use crate::render::{self, RenderOpts};

use super::scan::{self, Entry};

/// Bumped whenever [`NoteMeta`] changes shape. An older cache is discarded
/// rather than migrated: it is derived data, and a migration path for derived
/// data is code that can only ever be wrong.
const CACHE_VERSION: u32 = 1;

/// Everything the vault remembers about one note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMeta {
    /// Vault-relative, forward slashes. The note's identity everywhere.
    pub path: String,
    /// Frontmatter `title`, else the first H1, else the file stem.
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Outgoing wiki-link targets as written, with the line they appear on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<WikiRef>,
    pub mtime_ms: u64,
    pub size: u64,
}

/// One `[[target]]` occurrence inside a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiRef {
    pub target: String,
    pub line: usize,
}

/// A note that links *to* the one being displayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backlink {
    pub path: String,
    pub title: String,
    pub line: usize,
    /// The target as the referrer wrote it — `[[Note|display]]` and `[[note]]`
    /// both land here, and showing the user which spelling was used is the
    /// difference between "why does this link here" and an answer.
    pub target: String,
}

/// Progress while building. Streamed so a large vault shows a count moving.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct IndexProgress {
    pub done: usize,
    pub total: usize,
    /// Notes taken from the cache rather than re-read.
    pub cached: usize,
}

/// The name graph of a vault.
#[derive(Debug, Default)]
pub struct Index {
    root: PathBuf,
    notes: Vec<NoteMeta>,
    /// Normalised full relative path without extension → note. Unique.
    by_path: HashMap<String, u32>,
    /// Normalised file stem → notes. Not unique: two folders may hold `index.md`.
    by_stem: HashMap<String, Vec<u32>>,
    /// Normalised alias → note. First writer wins, deterministically, because
    /// the walk is sorted.
    by_alias: HashMap<String, u32>,
    /// Note → the notes that link to it.
    backlinks: HashMap<u32, Vec<Ref>>,
    /// Normalised target that resolves to nothing → who wanted it. This is what
    /// makes "3 notes link to a note you have not written" answerable, and an
    /// unresolved link is a **normal** state in a vault, not an error.
    unresolved: HashMap<String, Vec<Ref>>,
    /// Notes re-read from disk during the last build, for the receipt.
    pub parsed: usize,
    /// Notes taken from the cache during the last build.
    pub cached: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ref {
    from: u32,
    line: usize,
    /// Index into the referrer's `links`, so the target as written is one hop
    /// away without storing it twice.
    link: u32,
}

impl Index {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn notes(&self) -> &[NoteMeta] {
        &self.notes
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Builds the index for `root`, reusing `cache_dir` when a usable cache is
    /// there and writing one back when anything changed.
    ///
    /// `on_progress` returning `false` cancels; a cancelled build returns what
    /// it has, which is a usable partial index rather than an error — the
    /// sidebar can resolve links for the notes it did see.
    pub fn build(
        root: &Path,
        cache_dir: Option<&Path>,
        on_progress: &mut dyn FnMut(IndexProgress) -> bool,
    ) -> Self {
        let mut entries: Vec<Entry> = Vec::new();
        scan::walk(root, scan::BATCH, &mut |batch| {
            entries.extend(batch.iter().filter(|e| !e.dir).cloned());
            true
        });

        let mut cached: HashMap<String, NoteMeta> = cache_dir
            .and_then(|dir| Cache::load(dir, root))
            .map(|c| c.into_map())
            .unwrap_or_default();

        let total = entries.len();
        let mut notes = Vec::with_capacity(total);
        let mut reused = 0usize;
        let mut parsed = 0usize;
        let mut dirty = false;

        for (done, entry) in entries.into_iter().enumerate() {
            // The cache is invalidated per file, by mtime *and* size. Size
            // alone misses an edit that keeps the length; mtime alone misses a
            // filesystem with one-second resolution rewriting within the same
            // second. Together they are wrong only for a deliberate forgery.
            match cached.remove(&entry.path) {
                Some(meta) if meta.mtime_ms == entry.mtime_ms && meta.size == entry.size => {
                    reused += 1;
                    notes.push(meta);
                }
                _ => {
                    dirty = true;
                    parsed += 1;
                    notes.push(read_note(root, &entry));
                }
            }

            // Every 64 notes, not every note: on a warm cache the whole build
            // is faster than the progress events describing it would be.
            if done % 64 == 0
                && !on_progress(IndexProgress {
                    done,
                    total,
                    cached: reused,
                })
            {
                break;
            }
        }

        // Anything still in `cached` was deleted from the vault since the cache
        // was written, which is itself a reason to rewrite it.
        dirty |= !cached.is_empty();

        let mut index = Self::from_notes(root.to_path_buf(), notes);
        index.parsed = parsed;
        index.cached = reused;

        if dirty {
            if let Some(dir) = cache_dir {
                Cache::save(dir, root, &index.notes);
            }
        }

        index
    }

    /// Builds from notes already in hand. The maps are the only real work here,
    /// and they are rebuilt wholesale rather than patched: for 5 000 notes it
    /// is a few milliseconds, and a patched map that drifts from the note list
    /// produces backlinks that point at nothing.
    pub fn from_notes(root: PathBuf, notes: Vec<NoteMeta>) -> Self {
        let mut by_path = HashMap::with_capacity(notes.len());
        let mut by_stem: HashMap<String, Vec<u32>> = HashMap::with_capacity(notes.len());
        let mut by_alias = HashMap::new();

        for (i, note) in notes.iter().enumerate() {
            let id = i as u32;
            by_path.insert(normalize(strip_extension(&note.path)), id);
            by_stem
                .entry(normalize(stem_of(&note.path)))
                .or_default()
                .push(id);
            for alias in &note.aliases {
                by_alias.entry(normalize(alias)).or_insert(id);
            }
        }

        let mut index = Self {
            root,
            notes,
            by_path,
            by_stem,
            by_alias,
            backlinks: HashMap::new(),
            unresolved: HashMap::new(),
            parsed: 0,
            cached: 0,
        };
        index.link_edges();
        index
    }

    fn link_edges(&mut self) {
        let mut backlinks: HashMap<u32, Vec<Ref>> = HashMap::new();
        let mut unresolved: HashMap<String, Vec<Ref>> = HashMap::new();

        for (i, note) in self.notes.iter().enumerate() {
            let from = i as u32;
            for (j, link) in note.links.iter().enumerate() {
                let edge = Ref {
                    from,
                    line: link.line,
                    link: j as u32,
                };
                match self.lookup(&link.target) {
                    // A note linking to itself is an edge, not a backlink: it
                    // would show up in its own panel and tell the reader
                    // nothing.
                    Some(to) if to != from => backlinks.entry(to).or_default().push(edge),
                    Some(_) => {}
                    None => unresolved
                        .entry(normalize(&trim_target(&link.target)))
                        .or_default()
                        .push(edge),
                }
            }
        }

        self.backlinks = backlinks;
        self.unresolved = unresolved;
    }

    /// Resolves a `[[target]]` to a note.
    ///
    /// Full relative path first, then file stem, then alias — most specific
    /// wins, so `[[projects/index]]` and `[[index]]` can mean different things
    /// in a vault that has several.
    pub fn resolve(&self, target: &str) -> Option<&NoteMeta> {
        let id = self.lookup(target)?;
        self.notes.get(id as usize)
    }

    /// The note id a target names, or `None`.
    ///
    /// When several notes share a stem — every vault with more than one
    /// `index.md` — the shallowest wins, ties broken by path. Obsidian
    /// disambiguates by prompting; a viewer cannot, so the rule has to be
    /// stated and stable rather than "whichever the walk saw first".
    fn lookup(&self, target: &str) -> Option<u32> {
        let key = normalize(&trim_target(target));
        if key.is_empty() {
            return None;
        }
        if let Some(&id) = self.by_path.get(&key) {
            return Some(id);
        }
        if let Some(ids) = self.by_stem.get(&key) {
            return ids.iter().copied().min_by_key(|id| {
                let path = self.notes.get(*id as usize).map(|n| n.path.as_str());
                (
                    path.map_or(usize::MAX, |p| p.matches('/').count()),
                    path.unwrap_or(""),
                )
            });
        }
        self.by_alias.get(&key).copied()
    }

    /// The `href` for a resolved wiki-link, as the render core wants it.
    ///
    /// A `marklet://` URL rather than a bare path because it is the one scheme
    /// the webview can actually fetch, and because `data-target` — which the
    /// UI routes through `open_document` — is emitted alongside it either way.
    /// Percent-encoded so `protocol.rs`'s decoder round-trips it exactly.
    pub fn resolve_href(&self, target: &str) -> Option<String> {
        let note = self.resolve(target)?;
        Some(format!(
            "{}{}",
            AssetRoot::url_prefix(),
            encode_path(&note.path)
        ))
    }

    /// Every note that links to `path`, sorted by path then line so the panel
    /// does not reshuffle between rebuilds.
    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink> {
        let Some(&id) = self.by_path.get(&normalize(strip_extension(path))) else {
            return Vec::new();
        };
        let Some(refs) = self.backlinks.get(&id) else {
            return Vec::new();
        };

        let mut out: Vec<Backlink> = refs.iter().filter_map(|r| self.backlink(r)).collect();
        out.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        out
    }

    /// Every note that links to a name nothing answers to.
    ///
    /// The vault's to-do list, effectively — and the reason the unresolved map
    /// is kept rather than discarded.
    pub fn unresolved_for(&self, target: &str) -> Vec<Backlink> {
        let Some(refs) = self.unresolved.get(&normalize(&trim_target(target))) else {
            return Vec::new();
        };
        let mut out: Vec<Backlink> = refs.iter().filter_map(|r| self.backlink(r)).collect();
        out.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        out
    }

    /// Distinct unresolved targets, most-wanted first.
    pub fn missing_notes(&self) -> Vec<(String, usize)> {
        let mut out: Vec<(String, usize)> = self
            .unresolved
            .values()
            .filter_map(|v| {
                // The map's key is a lookup key, not a name. Show the spelling
                // somebody actually typed.
                let first = v.first()?;
                let note = self.notes.get(first.from as usize)?;
                let link = note.links.get(first.link as usize)?;
                Some((trim_target(&link.target), v.len()))
            })
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    fn backlink(&self, r: &Ref) -> Option<Backlink> {
        let note = self.notes.get(r.from as usize)?;
        let link = note.links.get(r.link as usize)?;
        Some(Backlink {
            path: note.path.clone(),
            title: note.title.clone(),
            line: r.line,
            target: link.target.clone(),
        })
    }

    /// Replaces one note in place — what the file watcher calls on a save.
    ///
    /// The edges are rebuilt afterwards because a single note's change can
    /// resolve or orphan links in *other* notes: writing `Roadmap.md` turns
    /// every `[[Roadmap]]` in the vault from unresolved into resolved.
    pub fn update(&mut self, rel: &str, meta: Option<NoteMeta>) {
        let mut notes = std::mem::take(&mut self.notes);
        match notes.iter().position(|n| n.path == rel) {
            Some(i) => match meta {
                Some(m) => notes[i] = m,
                None => {
                    notes.remove(i);
                }
            },
            None => {
                if let Some(m) = meta {
                    notes.push(m);
                    notes.sort_by(|a, b| a.path.cmp(&b.path));
                }
            }
        }
        let root = std::mem::take(&mut self.root);
        *self = Self::from_notes(root, notes);
    }

    /// Heap bytes this index holds, near enough to be worth quoting.
    ///
    /// Counts the string contents and the vector/map capacities that dominate;
    /// it does not count the allocator's own bookkeeping. The number exists so
    /// "the RSS delta for opening a vault stays under 30 MB" is a measurement
    /// rather than a hope.
    pub fn memory_bytes(&self) -> usize {
        let notes: usize = self
            .notes
            .iter()
            .map(|n| {
                std::mem::size_of::<NoteMeta>()
                    + n.path.capacity()
                    + n.title.capacity()
                    + n.aliases.iter().map(|a| a.capacity() + 24).sum::<usize>()
                    + n.links
                        .iter()
                        .map(|l| l.target.capacity() + std::mem::size_of::<WikiRef>())
                        .sum::<usize>()
            })
            .sum();

        // 40 bytes per entry is this allocator's bucket overhead for a
        // `HashMap<String, _>`, near enough for a budget check.
        let maps = self
            .by_path
            .keys()
            .map(|k| k.capacity() + 40)
            .sum::<usize>()
            + self
                .by_stem
                .iter()
                .map(|(k, v)| k.capacity() + 40 + v.capacity() * 4)
                .sum::<usize>()
            + self
                .by_alias
                .keys()
                .map(|k| k.capacity() + 40)
                .sum::<usize>();

        let edges = self
            .backlinks
            .values()
            .map(|v| v.capacity() * std::mem::size_of::<Ref>() + 40)
            .sum::<usize>()
            + self
                .unresolved
                .iter()
                .map(|(k, v)| k.capacity() + 40 + v.capacity() * std::mem::size_of::<Ref>())
                .sum::<usize>();

        notes + maps + edges
    }
}

/// Reads and parses one note into its metadata.
///
/// The resolver handed to the render core returns the target unchanged, which
/// makes `Link.href` carry the target verbatim. That is the whole trick: the
/// edges come out of the parse that was happening anyway, in document order,
/// with their line numbers already computed.
pub fn read_note(root: &Path, entry: &Entry) -> NoteMeta {
    let path = root.join(&entry.path);
    let bytes = std::fs::read(&path).unwrap_or_default();

    let identity = |t: &str| Some(t.to_string());
    let doc = render::render(
        &bytes,
        RenderOpts {
            wiki_resolver: Some(&identity),
            ..RenderOpts::default()
        },
    );

    let frontmatter = doc.frontmatter.as_ref();
    let title = frontmatter
        .and_then(|f| f.get("title"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|t| !t.is_empty())
        .or_else(|| {
            doc.outline
                .iter()
                .find(|h| h.level == 1)
                .map(|h| h.text.clone())
        })
        .unwrap_or_else(|| stem_of(&entry.path).to_string());

    let mut aliases = Vec::new();
    for key in ["aliases", "alias"] {
        match frontmatter.and_then(|f| f.get(key)) {
            Some(serde_json::Value::Array(items)) => aliases.extend(
                items
                    .iter()
                    .filter_map(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            ),
            Some(serde_json::Value::String(s)) if !s.is_empty() => aliases.push(s.clone()),
            _ => {}
        }
    }

    let links = doc
        .links
        .iter()
        // `resolved` is false only when the target was not a safe URL — a note
        // called `javascript:x` is not a note. Dropping it here keeps a target
        // that could never be opened out of the graph.
        .filter(|l| l.wiki && l.resolved && !l.href.is_empty())
        .map(|l| WikiRef {
            target: l.href.clone(),
            line: l.line,
        })
        .collect();

    NoteMeta {
        path: entry.path.clone(),
        title,
        aliases,
        links,
        mtime_ms: entry.mtime_ms,
        size: entry.size,
    }
}

/// Reads one note by its vault-relative path, for the watcher's update path.
pub fn read_note_at(root: &Path, rel: &str) -> Option<NoteMeta> {
    let path = scan::resolve_in(root, rel).ok()?;
    let (size, mtime_ms) = scan::stat(&path)?;
    Some(read_note(
        root,
        &Entry {
            path: rel.to_string(),
            name: stem_of(rel).to_string(),
            dir: false,
            depth: 0,
            size,
            mtime_ms,
        },
    ))
}

/// Strips the parts of a wiki target that address *inside* a note.
///
/// `[[Note#Heading]]` and `[[Note^block-id]]` both point at `Note`; the anchor
/// is the UI's problem, not the graph's. A trailing `.md` is stripped too,
/// because people paste filenames.
fn trim_target(target: &str) -> String {
    let t = target.trim();
    let t = t.split(['#', '^']).next().unwrap_or(t).trim();
    let t = t.trim_end_matches('/');
    strip_extension(t).to_string()
}

fn strip_extension(path: &str) -> &str {
    for ext in scan::NOTE_EXTENSIONS {
        if path.len() > ext.len() + 1 {
            let (head, tail) = path.split_at(path.len() - ext.len() - 1);
            if tail.starts_with('.') && tail[1..].eq_ignore_ascii_case(ext) {
                return head;
            }
        }
    }
    path
}

fn stem_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    strip_extension(name)
}

/// The comparison key for a name.
///
/// Lowercased with `to_lowercase` rather than `to_ascii_lowercase`, because a
/// vault written in any language still expects `[[Über]]` to find `Über.md`.
/// Backslashes fold to forward slashes so a target pasted from Explorer
/// resolves.
fn normalize(s: &str) -> String {
    s.trim().replace('\\', "/").to_lowercase()
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encodes a vault-relative path for the `marklet://` scheme.
///
/// The exact inverse of `webpath::percent_decode`, which `protocol.rs` applies
/// before it touches the disk. Anything outside the unreserved set is escaped —
/// a `#` in a filename would otherwise truncate the URL at the fragment, and a
/// `%` would be decoded twice.
fn encode_path(rel: &str) -> String {
    let mut out = String::with_capacity(rel.len() + 8);
    for b in rel.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

/// The on-disk shape of the cache.
#[derive(Serialize, Deserialize)]
struct Cache {
    version: u32,
    root: String,
    written_ms: u64,
    notes: Vec<NoteMeta>,
}

impl Cache {
    fn into_map(self) -> HashMap<String, NoteMeta> {
        self.notes
            .into_iter()
            .map(|n| (n.path.clone(), n))
            .collect()
    }

    /// One file per vault, named by a hash of the canonical root.
    ///
    /// A hash rather than a sanitised path: paths contain separators, colons,
    /// and characters no filesystem agrees about, and the cache directory must
    /// not become a place where a crafted vault name writes outside it.
    fn path(dir: &Path, root: &Path) -> PathBuf {
        dir.join(format!("vault-index-{:016x}.json", fnv1a(root)))
    }

    fn load(dir: &Path, root: &Path) -> Option<Self> {
        let bytes = std::fs::read(Self::path(dir, root)).ok()?;
        // A truncated or hand-edited cache is discarded silently. It is derived
        // data; the only cost of ignoring it is one rebuild.
        let cache: Self = serde_json::from_slice(&bytes).ok()?;
        (cache.version == CACHE_VERSION && cache.root == root.to_string_lossy()).then_some(cache)
    }

    fn save(dir: &Path, root: &Path, notes: &[NoteMeta]) {
        let cache = Self {
            version: CACHE_VERSION,
            root: root.to_string_lossy().into_owned(),
            written_ms: scan::now_ms(),
            notes: notes.to_vec(),
        };
        let Ok(bytes) = serde_json::to_vec(&cache) else {
            return;
        };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        // Write-then-rename, so a crash mid-write leaves the previous cache
        // rather than a half-written one that parses.
        let final_path = Self::path(dir, root);
        let tmp = final_path.with_extension("json.tmp");
        if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, &final_path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn fnv1a(path: &Path) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::testing::Sandbox;

    fn build(s: &Sandbox) -> Index {
        Index::build(s.root(), None, &mut |_| true)
    }

    #[test]
    fn a_wikilink_resolves_to_a_real_note() {
        let s = Sandbox::new("index-resolve");
        s.note("Note.md", "# Note\n");
        s.note("Other.md", "See [[Note]].\n");

        let index = build(&s);
        assert_eq!(
            index.resolve("Note").map(|n| n.path.as_str()),
            Some("Note.md")
        );
        assert_eq!(
            index.resolve("note").map(|n| n.path.as_str()),
            Some("Note.md")
        );
        assert!(index
            .resolve_href("Note")
            .is_some_and(|h| h.ends_with("Note.md")));
    }

    #[test]
    fn an_unresolved_link_is_a_normal_state_not_an_error() {
        let s = Sandbox::new("index-unresolved");
        s.note("a.md", "[[Nowhere]]\n");

        let index = build(&s);
        assert!(index.resolve("Nowhere").is_none());
        assert!(index.resolve_href("Nowhere").is_none());
        assert_eq!(index.unresolved_for("Nowhere").len(), 1);
        assert_eq!(index.missing_notes(), [("Nowhere".to_string(), 1)]);
    }

    #[test]
    fn the_backlinks_panel_lists_every_referrer() {
        let s = Sandbox::new("index-backlinks");
        s.note("Note.md", "# Note\n\n[[Note]] links to itself.\n");
        s.note("a.md", "# A\n\n[[Note]]\n");
        s.note("b.md", "# B\n\nline one\n\n[[note]] and [[Note#Heading]]\n");
        s.note("c.md", "# C\n\nno links\n");

        let index = build(&s);
        let back = index.backlinks_for("Note.md");
        assert_eq!(
            back.iter().map(|b| b.path.as_str()).collect::<Vec<_>>(),
            ["a.md", "b.md", "b.md"],
            "self-links are excluded, every other referrer is listed"
        );
        assert_eq!(back[0].line, 3);
        assert_eq!(back[0].title, "A");
        assert_eq!(back[2].target, "Note#Heading");
    }

    #[test]
    fn a_piped_wikilink_indexes_the_target_not_the_display_text() {
        let s = Sandbox::new("index-piped");
        s.note("Target.md", "# Target\n");
        s.note("a.md", "[[Target|something else entirely]]\n");

        let index = build(&s);
        assert_eq!(index.backlinks_for("Target.md").len(), 1);
    }

    #[test]
    fn a_link_inside_a_code_fence_is_not_an_edge() {
        // The reason edges come from the parser rather than from a regex.
        let s = Sandbox::new("index-fence");
        s.note("Target.md", "# Target\n");
        s.note("a.md", "```\n[[Target]]\n```\n");

        let index = build(&s);
        assert!(index.backlinks_for("Target.md").is_empty());
    }

    #[test]
    fn titles_prefer_frontmatter_then_h1_then_the_filename() {
        let s = Sandbox::new("index-titles");
        s.note("fm.md", "---\ntitle: From frontmatter\n---\n\n# From H1\n");
        s.note("h1.md", "# From H1\n");
        s.note("bare.md", "just text\n");

        let index = build(&s);
        let title = |p: &str| {
            index
                .notes()
                .iter()
                .find(|n| n.path == p)
                .map(|n| n.title.clone())
                .unwrap_or_default()
        };
        assert_eq!(title("fm.md"), "From frontmatter");
        assert_eq!(title("h1.md"), "From H1");
        assert_eq!(title("bare.md"), "bare");
    }

    #[test]
    fn aliases_resolve() {
        let s = Sandbox::new("index-aliases");
        s.note(
            "Real.md",
            "---\naliases: [Nickname, Second Name]\n---\n\n# Real\n",
        );
        s.note("a.md", "[[Nickname]] and [[second name]]\n");

        let index = build(&s);
        assert_eq!(
            index.resolve("Nickname").map(|n| n.path.as_str()),
            Some("Real.md")
        );
        assert_eq!(index.backlinks_for("Real.md").len(), 2);
    }

    #[test]
    fn a_full_path_target_beats_a_stem_of_the_same_name() {
        let s = Sandbox::new("index-specificity");
        s.note("index.md", "# Root index\n");
        s.note("projects/index.md", "# Project index\n");

        let index = build(&s);
        assert_eq!(
            index.resolve("projects/index").map(|n| n.path.as_str()),
            Some("projects/index.md")
        );
    }

    #[test]
    fn the_cache_is_reused_when_nothing_changed_and_dropped_when_it_did() {
        let s = Sandbox::new("index-cache");
        let cache = s.cache_dir();
        s.note("a.md", "# A\n\n[[b]]\n");
        s.note("b.md", "# B\n");

        let first = Index::build(s.root(), Some(&cache), &mut |_| true);
        assert_eq!((first.parsed, first.cached), (2, 0));

        let second = Index::build(s.root(), Some(&cache), &mut |_| true);
        assert_eq!((second.parsed, second.cached), (0, 2), "warm cache");
        assert_eq!(second.backlinks_for("b.md").len(), 1);

        // A rewrite with a different length invalidates that one file only.
        s.note("b.md", "# B rewritten, and longer than before\n");
        let third = Index::build(s.root(), Some(&cache), &mut |_| true);
        assert_eq!((third.parsed, third.cached), (1, 1));
        assert_eq!(
            third.resolve("b").map(|n| n.title.as_str()),
            Some("B rewritten, and longer than before")
        );
    }

    #[test]
    fn a_corrupt_cache_costs_a_rebuild_and_nothing_else() {
        let s = Sandbox::new("index-cache-corrupt");
        let cache = s.cache_dir();
        s.note("a.md", "# A\n");
        Index::build(s.root(), Some(&cache), &mut |_| true);

        std::fs::write(Cache::path(&cache, s.root()), b"{ not json").unwrap();
        let rebuilt = Index::build(s.root(), Some(&cache), &mut |_| true);
        assert_eq!((rebuilt.parsed, rebuilt.cached), (1, 0));
    }

    #[test]
    fn a_deleted_note_leaves_the_index_on_the_next_build() {
        let s = Sandbox::new("index-delete");
        let cache = s.cache_dir();
        s.note("a.md", "# A\n");
        s.note("b.md", "# B\n");
        Index::build(s.root(), Some(&cache), &mut |_| true);

        std::fs::remove_file(s.root().join("b.md")).unwrap();
        let after = Index::build(s.root(), Some(&cache), &mut |_| true);
        assert_eq!(after.len(), 1);
        assert!(after.resolve("b").is_none());
    }

    #[test]
    fn writing_a_missing_note_resolves_every_link_that_wanted_it() {
        let s = Sandbox::new("index-update");
        s.note("a.md", "[[Roadmap]]\n");
        s.note("b.md", "[[Roadmap]]\n");

        let mut index = build(&s);
        assert_eq!(index.unresolved_for("Roadmap").len(), 2);

        let path = s.note("Roadmap.md", "# Roadmap\n");
        let (size, mtime_ms) = scan::stat(&path).unwrap();
        index.update(
            "Roadmap.md",
            Some(NoteMeta {
                path: "Roadmap.md".into(),
                title: "Roadmap".into(),
                aliases: Vec::new(),
                links: Vec::new(),
                mtime_ms,
                size,
            }),
        );

        assert_eq!(index.backlinks_for("Roadmap.md").len(), 2);
        assert!(index.unresolved_for("Roadmap").is_empty());
    }

    #[test]
    fn a_percent_encoded_href_round_trips_through_the_protocol_decoder() {
        let s = Sandbox::new("index-encoding");
        // A space and a literal percent: both would be a different filename by
        // the time `protocol.rs` decoded them, if either escaped unencoded.
        // (`#` cannot be tested from a wiki target — `[[a#b]]` addresses a
        // heading inside `a`, so the target never contains one.)
        s.note("my notes/100% done.md", "# Done\n");

        let index = build(&s);
        let href = index.resolve_href("100% done").expect("resolved");
        let path = href
            .strip_prefix(AssetRoot::url_prefix())
            .expect("prefixed with the scheme");
        assert!(!path.contains(' '), "got {path}");
        assert_eq!(
            crate::webpath::percent_decode(path),
            "my notes/100% done.md"
        );
    }

    #[test]
    fn an_unreadable_note_does_not_panic() {
        // A vault contains whatever a user put in it, including a file that
        // vanishes between the walk and the read.
        let s = Sandbox::new("index-vanished");
        let meta = read_note(
            s.root(),
            &Entry {
                path: "gone.md".into(),
                name: "gone".into(),
                dir: false,
                depth: 0,
                size: 0,
                mtime_ms: 0,
            },
        );
        assert_eq!(meta.title, "gone");
        assert!(meta.links.is_empty());
    }

    #[test]
    fn trimming_a_target_drops_anchors_and_extensions() {
        assert_eq!(trim_target("Note#Heading"), "Note");
        assert_eq!(trim_target("Note^block-id"), "Note");
        assert_eq!(trim_target(" Note.md "), "Note");
        assert_eq!(trim_target("folder/Note.MARKDOWN"), "folder/Note");
    }
}

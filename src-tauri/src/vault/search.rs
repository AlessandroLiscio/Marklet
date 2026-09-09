//! Vault search: `walkdir` + `aho-corasick`, streaming, **no index**.
//!
//! An index would mean staleness, a build step, a cache that can corrupt, and
//! disk to keep in sync — three failure modes bought to solve a problem that
//! does not exist at this scale. Literal multi-term AND over 5 000 notes is a
//! sequential read of a few megabytes the page cache already holds, and
//! `aho-corasick` finds every term in one pass over each file. `tantivy` costs
//! 4-6 MB to answer the same question and is denied by name in `deny.toml`.
//!
//! **Results stream.** [`search`] calls back per hit rather than returning a
//! `Vec`, so the first result reaches the sidebar before the last file is read.
//! That is not a micro-optimisation: it is the difference between a search box
//! that feels instant and one that feels like it hung.
//!
//! ## Case sensitivity
//!
//! Literal search is **ASCII** case-insensitive: `aho-corasick` folds `a-z`
//! against `A-Z` inside the automaton at no cost. Folding non-ASCII would mean
//! lowercasing every file into a fresh allocation whose byte offsets no longer
//! line up with the original — a real cost, and a class of off-by-one bug, for
//! a case people rarely search across. Full Unicode casefolding is what the
//! `regex-search` feature gives you.
//!
//! ## Excerpts
//!
//! A hit carries its line split into [`Span`]s rather than byte offsets into a
//! string. JavaScript strings are UTF-16, so a byte offset computed in Rust is
//! wrong in the webview the moment a note contains an emoji — and the UI would
//! have to slice and escape the result itself. Pre-split spans make the
//! renderer a loop over `<mark>` or not, with no arithmetic and no `innerHTML`.

use std::path::Path;
use std::time::Instant;

use aho_corasick::{AhoCorasick, MatchKind};

use super::scan;

/// Hits returned when the caller does not say. Enough to fill a sidebar twice
/// over; a search that matches 3 000 notes is a query to refine, not a list to
/// scroll.
pub const DEFAULT_LIMIT: usize = 200;

/// Lines shown per file. One is usually the whole answer; two gives the reader
/// enough to tell two similar notes apart without turning one file into a page.
pub const DEFAULT_LINES_PER_FILE: usize = 2;

/// Terms beyond this are ignored. The `seen` bitmask is a `u64`, and a
/// 65-term query is not a query.
pub const MAX_TERMS: usize = 64;

/// Bytes of a line kept around a match before it is elided.
const EXCERPT_BUDGET: usize = 240;

/// How much of a file is sniffed for NUL before deciding it is binary.
const BINARY_SNIFF: usize = 8192;

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct Query {
    /// Whitespace-separated terms; `"quoted phrases"` stay whole.
    pub text: String,
    /// Full regex instead of literal terms. Requires the `regex-search`
    /// feature — the FULL edition only. Ignored in lite, which reports it.
    pub regex: bool,
    /// 0 means [`DEFAULT_LIMIT`].
    pub limit: usize,
    /// 0 means [`DEFAULT_LINES_PER_FILE`].
    pub lines_per_file: usize,
}

/// A run of excerpt text, matched or not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Span {
    pub text: String,
    /// True for the run a term matched. The UI wraps these in `<mark>`.
    pub hit: bool,
}

/// One result line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Hit {
    /// Vault-relative, forward slashes — the same identity the tree uses.
    pub path: String,
    /// 1-based, so it can be handed to `data-l` and to an editor unchanged.
    pub line: usize,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct SearchStats {
    pub files_scanned: usize,
    pub files_matched: usize,
    pub hits: usize,
    pub elapsed_ms: u64,
    /// Milliseconds until the **first** hit reached the callback. The number
    /// the user actually feels.
    pub first_hit_ms: u64,
    pub cancelled: bool,
    /// The limit was reached; there are more matches than were reported.
    pub truncated: bool,
    /// A regex query arrived in a build without `regex-search`. The search ran
    /// as a literal one; the UI says so rather than silently answering a
    /// different question.
    pub regex_unavailable: bool,
}

/// Splits a query into terms, keeping `"quoted phrases"` whole.
///
/// Duplicate terms are dropped case-insensitively: `foo Foo` is one term, and
/// leaving both in would make the AND trivially satisfiable by a single match.
pub fn terms_of(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;

    let push = |current: &mut String, terms: &mut Vec<String>| {
        let term = std::mem::take(current);
        if !term.is_empty() && !terms.iter().any(|t| t.eq_ignore_ascii_case(&term)) {
            terms.push(term);
        }
    };

    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                if !quoted {
                    push(&mut current, &mut terms);
                }
            }
            c if c.is_whitespace() && !quoted => push(&mut current, &mut terms),
            c => current.push(c),
        }
    }
    push(&mut current, &mut terms);

    terms.truncate(MAX_TERMS);
    terms
}

/// Searches `root`, streaming each hit to `on_hit`.
///
/// `on_hit` returning `false` cancels — which is how the next keystroke stops
/// the previous query instead of racing it to the sidebar.
///
/// AND across terms, evaluated per **file**: a note matches when every term
/// appears somewhere in it, not necessarily on one line. That is what people
/// mean by "notes about rust and tokio", and it is why the file is scanned
/// fully before anything is emitted for it.
pub fn search(root: &Path, query: &Query, on_hit: &mut dyn FnMut(&Hit) -> bool) -> SearchStats {
    let started = Instant::now();
    let mut stats = SearchStats::default();

    #[cfg(feature = "regex-search")]
    if query.regex {
        return regex_search(root, query, on_hit, started);
    }
    #[cfg(not(feature = "regex-search"))]
    if query.regex {
        // Lite has no regex engine. Running the pattern as a literal is the
        // honest fallback — it may even match — but the caller is told, so the
        // UI can say "regex needs the full edition" instead of showing an
        // empty result and letting the user conclude the vault is empty.
        stats.regex_unavailable = true;
    }

    let terms = terms_of(&query.text);
    if terms.is_empty() {
        stats.elapsed_ms = started.elapsed().as_millis() as u64;
        return stats;
    }

    let Ok(ac) = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::Standard)
        .build(&terms)
    else {
        stats.elapsed_ms = started.elapsed().as_millis() as u64;
        return stats;
    };

    let limit = if query.limit == 0 {
        DEFAULT_LIMIT
    } else {
        query.limit
    };
    let lines_per_file = if query.lines_per_file == 0 {
        DEFAULT_LINES_PER_FILE
    } else {
        query.lines_per_file
    };
    let all_terms = mask(terms.len());

    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);

    scan::walk(root, scan::BATCH, &mut |batch| {
        for entry in batch.iter().filter(|e| !e.dir) {
            if scan::read_into(&root.join(&entry.path), &mut buf).is_err() {
                // A note that vanished between the walk and the read, or one
                // the user cannot read. Ordinary; skip it.
                continue;
            }
            stats.files_scanned += 1;

            if is_binary(&buf) {
                continue;
            }

            let Some(lines) = matching_lines(&ac, &buf, all_terms, lines_per_file) else {
                continue;
            };
            stats.files_matched += 1;

            for (offset, matches) in lines {
                let hit = build_hit(&entry.path, &buf, offset, &matches);
                stats.hits += 1;
                if stats.first_hit_ms == 0 {
                    stats.first_hit_ms = started.elapsed().as_millis() as u64;
                }
                if !on_hit(&hit) {
                    stats.cancelled = true;
                    return false;
                }
                if stats.hits >= limit {
                    stats.truncated = true;
                    return false;
                }
            }
        }
        true
    });

    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    stats
}

/// A bitmask with the low `n` bits set. `n` is capped at [`MAX_TERMS`].
fn mask(n: usize) -> u64 {
    if n >= 64 {
        u64::MAX
    } else {
        (1u64 << n) - 1
    }
}

/// NUL in the first few kilobytes. Text files do not contain one; a PNG someone
/// renamed to `.md` does, and searching it would emit a line of mojibake.
fn is_binary(buf: &[u8]) -> bool {
    let head = &buf[..buf.len().min(BINARY_SNIFF)];
    memchr::memchr(0, head).is_some()
}

/// One match: where it starts and how long it is.
type Match = (usize, usize);

/// Scans a file once and returns the lines worth showing, or `None` when the
/// file does not satisfy the AND.
///
/// The single pass is the whole design. Matches are bucketed by their line's
/// start offset as they are found, so the file is never re-scanned to locate
/// them and the buckets are already in document order.
fn matching_lines(
    ac: &AhoCorasick,
    buf: &[u8],
    all_terms: u64,
    lines_per_file: usize,
) -> Option<Vec<(usize, Vec<Match>)>> {
    let mut seen: u64 = 0;
    // `(line_start, matches, terms_on_this_line)`, in document order.
    let mut lines: Vec<(usize, Vec<Match>, u64)> = Vec::new();

    for m in ac.find_iter(buf) {
        let id = m.pattern().as_usize();
        if id < 64 {
            seen |= 1 << id;
        }

        let start = line_start(buf, m.start());
        let bit = if id < 64 { 1u64 << id } else { 0 };
        match lines.last_mut() {
            Some((s, hits, terms)) if *s == start => {
                hits.push((m.start(), m.len()));
                *terms |= bit;
            }
            _ => lines.push((start, vec![(m.start(), m.len())], bit)),
        }
    }

    if seen & all_terms != all_terms {
        return None;
    }

    // Best lines first — the ones carrying the most distinct terms are the ones
    // that answer the query — then back into document order for display, so the
    // reader sees the file's own sequence rather than a ranking.
    lines.sort_by(|a, b| b.2.count_ones().cmp(&a.2.count_ones()).then(a.0.cmp(&b.0)));
    lines.truncate(lines_per_file);
    lines.sort_by_key(|l| l.0);

    Some(lines.into_iter().map(|(s, hits, _)| (s, hits)).collect())
}

/// The offset of the first byte of the line containing `offset`.
fn line_start(buf: &[u8], offset: usize) -> usize {
    memchr::memrchr(b'\n', &buf[..offset]).map_or(0, |i| i + 1)
}

/// Turns a matched line into a displayable hit.
fn build_hit(path: &str, buf: &[u8], line_start: usize, matches: &[Match]) -> Hit {
    let mut end = memchr::memchr(b'\n', &buf[line_start..]).map_or(buf.len(), |i| line_start + i);
    // A CRLF file would otherwise put a carriage return in the excerpt, which
    // renders as nothing and copies as a surprise.
    if end > line_start && buf.get(end - 1) == Some(&b'\r') {
        end -= 1;
    }

    let line = 1 + memchr::memchr_iter(b'\n', &buf[..line_start]).count();
    let (from, to) = window(buf, line_start, end, matches);

    let mut spans = Vec::new();
    let mut cursor = from;

    if from > line_start {
        spans.push(Span {
            text: "…".into(),
            hit: false,
        });
    }

    for &(m_start, m_len) in matches {
        let m_end = m_start + m_len;
        if m_start < cursor || m_end > to {
            continue;
        }
        if m_start > cursor {
            spans.push(text_span(&buf[cursor..m_start], false));
        }
        spans.push(text_span(&buf[m_start..m_end], true));
        cursor = m_end;
    }

    if cursor < to {
        spans.push(text_span(&buf[cursor..to], false));
    }
    if to < end {
        spans.push(Span {
            text: "…".into(),
            hit: false,
        });
    }

    Hit {
        path: path.to_string(),
        line,
        spans,
    }
}

/// The slice of a line to show: [`EXCERPT_BUDGET`] bytes centred on the first
/// match, snapped outward to character boundaries.
///
/// A minified 2 MB JSON file pasted into a note is one line; without a window
/// its whole length would cross the IPC boundary for one match.
fn window(buf: &[u8], start: usize, end: usize, matches: &[Match]) -> (usize, usize) {
    if end - start <= EXCERPT_BUDGET {
        return (start, end);
    }
    let first = matches.first().map_or(start, |m| m.0);
    let half = EXCERPT_BUDGET / 2;
    let mut from = first.saturating_sub(half).max(start);
    let mut to = (from + EXCERPT_BUDGET).min(end);
    from = floor_boundary(buf, from, start);
    to = ceil_boundary(buf, to, end);
    (from, to)
}

/// Walks back to a UTF-8 character boundary. Slicing mid-character would make
/// the lossy decode below emit a replacement character for text that is fine.
fn floor_boundary(buf: &[u8], mut i: usize, floor: usize) -> usize {
    while i > floor && buf.get(i).is_some_and(|b| b & 0xC0 == 0x80) {
        i -= 1;
    }
    i
}

fn ceil_boundary(buf: &[u8], mut i: usize, ceil: usize) -> usize {
    while i < ceil && buf.get(i).is_some_and(|b| b & 0xC0 == 0x80) {
        i += 1;
    }
    i
}

/// Lossy on purpose: a vault contains whatever a user put in it, including a
/// Windows-1252 note. A replacement character in one excerpt is better than a
/// result that silently disappears from a search.
fn text_span(bytes: &[u8], hit: bool) -> Span {
    Span {
        text: String::from_utf8_lossy(bytes).into_owned(),
        hit,
    }
}

/// Full regex search — FULL edition only.
///
/// The ripgrep stack costs +1.2-1.8 MB, which is 45% of lite's entire installer
/// ceiling for a feature literal multi-term AND already covers for most people.
/// `grep-searcher` rather than a bare `regex`: it handles binary detection,
/// line termination and multi-line patterns, which is most of what makes a
/// search over a folder of unknown files not crash.
#[cfg(feature = "regex-search")]
fn regex_search(
    root: &Path,
    query: &Query,
    on_hit: &mut dyn FnMut(&Hit) -> bool,
    started: Instant,
) -> SearchStats {
    use grep_regex::RegexMatcherBuilder;
    use grep_searcher::sinks::UTF8;
    use grep_searcher::{BinaryDetection, SearcherBuilder};

    let mut stats = SearchStats::default();

    let Ok(matcher) = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .line_terminator(Some(b'\n'))
        .build(&query.text)
    else {
        // An invalid pattern is a normal state while typing one, not an error
        // worth a dialog. Zero results, and the UI shows the box as invalid.
        stats.elapsed_ms = started.elapsed().as_millis() as u64;
        return stats;
    };

    let limit = if query.limit == 0 {
        DEFAULT_LIMIT
    } else {
        query.limit
    };
    let lines_per_file = if query.lines_per_file == 0 {
        DEFAULT_LINES_PER_FILE
    } else {
        query.lines_per_file
    };

    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(0))
        .line_number(true)
        .build();

    scan::walk(root, scan::BATCH, &mut |batch| {
        for entry in batch.iter().filter(|e| !e.dir) {
            stats.files_scanned += 1;
            let mut in_file = 0usize;
            let mut matched = false;
            let mut stop = false;

            let sink = UTF8(|line_number, text| {
                matched = true;
                let hit = regex_hit(&entry.path, line_number as usize, text);
                stats.hits += 1;
                if stats.first_hit_ms == 0 {
                    stats.first_hit_ms = started.elapsed().as_millis() as u64;
                }
                if !on_hit(&hit) {
                    stats.cancelled = true;
                    stop = true;
                    return Ok(false);
                }
                if stats.hits >= limit {
                    stats.truncated = true;
                    stop = true;
                    return Ok(false);
                }
                in_file += 1;
                Ok(in_file < lines_per_file)
            });

            let _ = searcher.search_path(&matcher, root.join(&entry.path), sink);
            if matched {
                stats.files_matched += 1;
            }
            if stop {
                return false;
            }
        }
        true
    });

    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    stats
}

/// A regex hit has no per-match offsets from the sink, so the excerpt is the
/// line, trimmed to the same budget, with no highlight run.
#[cfg(feature = "regex-search")]
fn regex_hit(path: &str, line: usize, text: &str) -> Hit {
    let trimmed = text.trim_end_matches(['\n', '\r']);
    let mut spans = Vec::with_capacity(2);
    if trimmed.len() <= EXCERPT_BUDGET {
        spans.push(Span {
            text: trimmed.to_string(),
            hit: false,
        });
    } else {
        let mut to = EXCERPT_BUDGET;
        while to > 0 && !trimmed.is_char_boundary(to) {
            to -= 1;
        }
        spans.push(Span {
            text: trimmed[..to].to_string(),
            hit: false,
        });
        spans.push(Span {
            text: "…".into(),
            hit: false,
        });
    }
    Hit {
        path: path.to_string(),
        line,
        spans,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::testing::Sandbox;

    fn run(s: &Sandbox, text: &str) -> (Vec<Hit>, SearchStats) {
        let mut hits = Vec::new();
        let stats = search(
            s.root(),
            &Query {
                text: text.into(),
                ..Query::default()
            },
            &mut |h| {
                hits.push(h.clone());
                true
            },
        );
        (hits, stats)
    }

    fn plain(hit: &Hit) -> String {
        hit.spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn terms_split_on_whitespace_and_keep_quoted_phrases_whole() {
        assert_eq!(terms_of("rust tokio"), ["rust", "tokio"]);
        assert_eq!(
            terms_of("  \"async runtime\"  rust "),
            ["async runtime", "rust"]
        );
        assert_eq!(terms_of("foo FOO"), ["foo"], "duplicates fold");
        assert!(terms_of("   ").is_empty());
    }

    #[test]
    fn and_across_terms_is_evaluated_per_file_not_per_line() {
        let s = Sandbox::new("search-and");
        s.note("both.md", "mentions rust here\n\nand tokio there\n");
        s.note("one.md", "only rust\n");

        let (hits, stats) = run(&s, "rust tokio");
        assert_eq!(stats.files_matched, 1);
        assert!(hits.iter().all(|h| h.path == "both.md"));
    }

    #[test]
    fn matching_is_ascii_case_insensitive() {
        let s = Sandbox::new("search-case");
        s.note("a.md", "The Rust Programming Language\n");
        let (hits, _) = run(&s, "rust");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].spans.iter().any(|sp| sp.hit && sp.text == "Rust"));
    }

    #[test]
    fn a_hit_carries_the_line_split_into_spans_ready_to_mark() {
        let s = Sandbox::new("search-spans");
        s.note("a.md", "# Title\n\nthe quick brown fox\n");

        let (hits, _) = run(&s, "quick");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 3, "1-based, so it can be handed to data-l");
        assert_eq!(plain(&hits[0]), "the quick brown fox");
        assert_eq!(
            hits[0].spans,
            [
                Span {
                    text: "the ".into(),
                    hit: false
                },
                Span {
                    text: "quick".into(),
                    hit: true
                },
                Span {
                    text: " brown fox".into(),
                    hit: false
                },
            ]
        );
    }

    #[test]
    fn several_matches_on_one_line_each_get_their_own_span() {
        let s = Sandbox::new("search-multi");
        s.note("a.md", "rust and rust and rust\n");
        let (hits, _) = run(&s, "rust");
        assert_eq!(hits[0].spans.iter().filter(|s| s.hit).count(), 3);
    }

    #[test]
    fn results_stream_rather_than_being_collected() {
        let s = Sandbox::new("search-stream");
        for i in 0..40 {
            s.note(&format!("n{i:02}.md"), "needle\n");
        }

        let mut seen = 0;
        let stats = search(
            s.root(),
            &Query {
                text: "needle".into(),
                ..Query::default()
            },
            &mut |_| {
                seen += 1;
                // Cancelling after three proves the callback runs *during* the
                // walk: a collected implementation would have read all forty.
                seen < 3
            },
        );
        assert_eq!(seen, 3);
        assert!(stats.cancelled);
        assert!(stats.files_scanned < 40, "scanned {}", stats.files_scanned);
    }

    #[test]
    fn the_limit_truncates_rather_than_running_to_the_end() {
        let s = Sandbox::new("search-limit");
        for i in 0..20 {
            s.note(&format!("n{i:02}.md"), "needle\n");
        }
        let mut hits = 0;
        let stats = search(
            s.root(),
            &Query {
                text: "needle".into(),
                limit: 5,
                ..Query::default()
            },
            &mut |_| {
                hits += 1;
                true
            },
        );
        assert_eq!(hits, 5);
        assert!(stats.truncated);
    }

    #[test]
    fn lines_per_file_caps_one_file_from_filling_the_panel() {
        let s = Sandbox::new("search-lines");
        s.note("a.md", &"needle\n".repeat(50));
        let (hits, _) = run(&s, "needle");
        assert_eq!(hits.len(), DEFAULT_LINES_PER_FILE);
    }

    #[test]
    fn the_line_carrying_the_most_terms_is_preferred() {
        let s = Sandbox::new("search-rank");
        s.note(
            "a.md",
            "alpha alone\nbeta alone\nalpha and beta together\nalpha again\n",
        );
        let mut hits = Vec::new();
        search(
            s.root(),
            &Query {
                text: "alpha beta".into(),
                lines_per_file: 1,
                ..Query::default()
            },
            &mut |h| {
                hits.push(h.clone());
                true
            },
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 3);
    }

    #[test]
    fn a_very_long_line_is_elided_around_the_match() {
        let s = Sandbox::new("search-long");
        let mut line = "x".repeat(4000);
        line.push_str("needle");
        line.push_str(&"y".repeat(4000));
        s.note("a.md", &format!("{line}\n"));

        let (hits, _) = run(&s, "needle");
        let text = plain(&hits[0]);
        assert!(
            text.len() < EXCERPT_BUDGET + 32,
            "excerpt was {} bytes",
            text.len()
        );
        assert!(text.contains("needle"));
        assert!(text.starts_with('…') && text.ends_with('…'));
    }

    #[test]
    fn a_binary_file_named_md_is_skipped_rather_than_rendered_as_mojibake() {
        let s = Sandbox::new("search-binary");
        s.bytes(
            "fake.md",
            &[
                0x89, b'P', b'N', b'G', 0x00, b'n', b'e', b'e', b'd', b'l', b'e',
            ],
        );
        s.note("real.md", "needle\n");

        let (hits, _) = run(&s, "needle");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "real.md");
    }

    #[test]
    fn a_non_utf8_note_still_matches_and_does_not_panic() {
        let s = Sandbox::new("search-cp1252");
        // "café" in Windows-1252, which is not valid UTF-8.
        s.bytes("a.md", b"the caf\xe9 needle here\n");
        let (hits, _) = run(&s, "needle");
        assert_eq!(hits.len(), 1);
        assert!(plain(&hits[0]).contains("needle"));
    }

    #[test]
    fn a_multibyte_line_is_never_sliced_mid_character() {
        let s = Sandbox::new("search-utf8");
        let padding = "é".repeat(400);
        s.note("a.md", &format!("{padding} needle {padding}\n"));
        let (hits, _) = run(&s, "needle");
        // The lossy decode would have produced U+FFFD had the window landed
        // inside a two-byte sequence.
        assert!(!plain(&hits[0]).contains('\u{FFFD}'));
    }

    #[test]
    fn an_empty_query_matches_nothing_rather_than_everything() {
        let s = Sandbox::new("search-empty");
        s.note("a.md", "content\n");
        let (hits, stats) = run(&s, "   ");
        assert!(hits.is_empty());
        assert_eq!(stats.files_scanned, 0);
    }

    #[test]
    fn crlf_lines_do_not_leak_a_carriage_return_into_the_excerpt() {
        let s = Sandbox::new("search-crlf");
        s.bytes("a.md", b"first\r\nneedle here\r\nlast\r\n");
        let (hits, _) = run(&s, "needle");
        assert_eq!(plain(&hits[0]), "needle here");
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    #[cfg(not(feature = "regex-search"))]
    fn lite_reports_that_regex_is_unavailable_instead_of_answering_a_different_question() {
        let s = Sandbox::new("search-noregex");
        s.note("a.md", "needle\n");
        let stats = search(
            s.root(),
            &Query {
                text: "n.edle".into(),
                regex: true,
                ..Query::default()
            },
            &mut |_| true,
        );
        assert!(stats.regex_unavailable);
    }

    #[test]
    #[cfg(feature = "regex-search")]
    fn full_answers_a_real_regex() {
        let s = Sandbox::new("search-regex");
        s.note("a.md", "version 1.2.3 here\n");
        s.note("b.md", "no numbers\n");

        let mut hits = Vec::new();
        let stats = search(
            s.root(),
            &Query {
                text: r"\d+\.\d+\.\d+".into(),
                regex: true,
                ..Query::default()
            },
            &mut |h| {
                hits.push(h.clone());
                true
            },
        );
        assert!(!stats.regex_unavailable);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "a.md");
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    #[cfg(feature = "regex-search")]
    fn an_invalid_pattern_is_zero_results_rather_than_an_error() {
        let s = Sandbox::new("search-badregex");
        s.note("a.md", "content\n");
        let mut hits = 0;
        search(
            s.root(),
            &Query {
                text: "([unclosed".into(),
                regex: true,
                ..Query::default()
            },
            &mut |_| {
                hits += 1;
                true
            },
        );
        assert_eq!(hits, 0);
    }
}

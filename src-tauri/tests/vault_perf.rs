//! The vault's performance contract, measured against a generated 5 000-note
//! vault.
//!
//! These are the numbers the design was chosen for. `walkdir` + `aho-corasick`
//! with **no index** is only the right answer if a literal multi-term AND over
//! a real vault is genuinely fast, so the claim is asserted rather than
//! asserted-in-prose:
//!
//! | | target |
//! |---|---|
//! | first tree batch (a painted tree) | < 400 ms |
//! | first search result | < 150 ms |
//! | search completion | < 600 ms |
//! | index heap for 5 000 notes | < 30 MB |
//!
//! **The targets are release-build numbers.** A debug build runs
//! `pulldown-cmark` and `aho-corasick` unoptimised and is several times slower;
//! the assertions are therefore compiled only into the release test, and the
//! debug run prints its numbers without judging them. Measure with:
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml --release --test vault_perf -- --nocapture
//! ```
//!
//! The vault is generated into the temp directory and **kept** between runs,
//! keyed by note count. Regenerating 5 000 files per run would measure the
//! filesystem's write path rather than the vault's read path, and the targets
//! assume a warm cache — which is the state a user's own vault is in.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use marklet::vault::{index::Index, scan, search, Query};

const NOTES: usize = 5_000;
const FOLDERS: usize = 100;

#[test]
fn a_five_thousand_note_vault_scans_and_searches_within_budget() {
    let root = generated_vault();
    let release = !cfg!(debug_assertions);
    let rss_before = rss_bytes();

    // ---- scan: time to the FIRST batch, which is the first painted tree ----
    let started = Instant::now();
    let mut first_batch_ms = u128::MAX;
    let mut entries = 0usize;
    let stats = scan::walk(&root, scan::BATCH, &mut |batch| {
        if first_batch_ms == u128::MAX {
            first_batch_ms = started.elapsed().as_millis();
        }
        entries += batch.len();
        true
    });
    assert_eq!(stats.notes, NOTES);
    assert_eq!(entries, NOTES + FOLDERS);

    // ---- index: cold, then warm from the cache ----
    let cache = root.parent().expect("vault has a parent").join("cache");
    let _ = std::fs::remove_dir_all(&cache);

    let t = Instant::now();
    let index = Index::build(&root, Some(&cache), &mut |_| true);
    let index_cold_ms = t.elapsed().as_millis();
    assert_eq!(index.len(), NOTES);
    assert_eq!(index.parsed, NOTES);

    let t = Instant::now();
    let warm = Index::build(&root, Some(&cache), &mut |_| true);
    let index_warm_ms = t.elapsed().as_millis();
    assert_eq!(warm.cached, NOTES, "every note came from the cache");

    let memory = index.memory_bytes();

    // ---- wiki-links and backlinks over the real graph ----
    let target = index.resolve("note-0007").expect("[[note-0007]] resolves");
    assert_eq!(target.path, "folder-07/note-0007.md");
    let back = index.backlinks_for("folder-07/note-0007.md");
    // Each note links to exactly two others by a bijective stride over the
    // note numbers, so `note-0007` has exactly two referrers and both are
    // known — "every referrer" is a count, not a hand-wave.
    assert_eq!(
        back.iter().map(|b| b.path.as_str()).collect::<Vec<_>>(),
        ["folder-72/note-3572.md", "folder-92/note-2692.md"],
        "every referrer is listed, sorted by path"
    );
    assert!(back.iter().all(|b| b.target == "note-0007"));
    assert!(index.resolve("note-that-was-never-written").is_none());
    // Unresolved links are a normal state and are counted, not dropped: every
    // note points at one note that was never written.
    assert_eq!(index.missing_notes().len(), NOTES);

    // ---- search: two literal terms, AND ----
    let t = Instant::now();
    let mut hits = 0usize;
    let found = search::search(
        &root,
        &Query {
            text: "haystack quicksilver".into(),
            limit: 1_000,
            ..Query::default()
        },
        &mut |_| {
            hits += 1;
            true
        },
    );
    let search_ms = t.elapsed().as_millis();

    assert_eq!(found.files_scanned, NOTES);
    assert!(hits > 0, "the planted term was found");
    assert_eq!(
        found.files_matched,
        NOTES / 50,
        "one note in fifty carries both terms"
    );

    let profile = if release { "release" } else { "debug" };
    let scan_ms = stats.elapsed_ms;
    let first_hit_ms = found.first_hit_ms;
    let heap_kb = memory / 1024;
    let per_note = memory / NOTES;
    println!();
    println!("  vault perf ({profile} build, {NOTES} notes in {FOLDERS} folders)");
    println!("    first tree batch    {first_batch_ms:>6} ms   (target < 400)");
    println!("    full scan           {scan_ms:>6} ms");
    println!("    index build, cold   {index_cold_ms:>6} ms");
    println!("    index build, warm   {index_warm_ms:>6} ms");
    println!("    first search hit    {first_hit_ms:>6} ms   (target < 150)");
    println!("    search complete     {search_ms:>6} ms   (target < 600)");
    println!("    index heap          {heap_kb:>6} KB   ({per_note} B/note, budget 30 MB)");
    if let (Some(before), Some(after)) = (rss_before, rss_bytes()) {
        // The number the 30 MB budget is actually about: what opening a vault
        // costs the process, index and read buffers and allocator slack
        // included. `memory_bytes` accounts for the part that stays; this is
        // the part the operating system sees.
        let delta = after.saturating_sub(before);
        println!(
            "    process RSS delta   {:>6} KB   (scan + index, budget 30 MB)",
            delta / 1024
        );
        assert!(
            delta < 30 * 1024 * 1024,
            "opening a {NOTES}-note vault cost {delta} bytes of RSS, budget 30 MB"
        );
    }
    println!();

    // 30 MB is the budget for the whole vault feature; the index is the part
    // that stays resident, so it is checked in every build, debug included.
    assert!(
        memory < 30 * 1024 * 1024,
        "index heap {memory} bytes exceeds the 30 MB vault budget"
    );

    if release {
        assert!(
            first_batch_ms < 400,
            "first tree batch took {first_batch_ms} ms, target < 400"
        );
        assert!(
            found.first_hit_ms < 150,
            "first search hit took {} ms, target < 150",
            found.first_hit_ms
        );
        assert!(search_ms < 600, "search took {search_ms} ms, target < 600");
    }
}

/// Resident set size, from `/proc/self/statm`.
///
/// Linux only, and `None` everywhere else — the CI matrix runs Windows too, and
/// a test that fails there because it cannot read `/proc` would be measuring
/// the wrong thing. The budget is checked where it can be measured; the
/// assertion is simply skipped elsewhere rather than faked.
fn rss_bytes() -> Option<u64> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

/// Builds — or reuses — a vault of [`NOTES`] notes across [`FOLDERS`] folders.
///
/// Every note links to two others by stem, so the backlink graph is real rather
/// than empty, and one note in fifty carries both search terms so the AND has a
/// known answer to be checked against.
///
/// **Generated at most once per process.** Every test in this file shares one
/// fixture and `cargo test` runs them on separate threads, so without this
/// barrier each of them checked the marker, found nothing, and raced to
/// `remove_dir_all` the vault and rewrite all 5 000 notes underneath the
/// others. That is not merely a slow fixture, it is a *wrong* one: a
/// regeneration landing between the cold index build and the warm one gives
/// every note a new mtime, the cache correctly refuses keys that no longer
/// describe the file on disk, and `warm.cached == NOTES` fails for a vault that
/// genuinely did change. The same race also failed the two `expect`s below
/// outright, when one thread deleted the tree another was writing into.
///
/// It never reproduced on a developer's machine because the marker from the
/// previous run short-circuits every thread before it can generate anything;
/// only a fresh runner has all of them arrive cold at once.
fn generated_vault() -> PathBuf {
    static VAULT: OnceLock<PathBuf> = OnceLock::new();
    VAULT.get_or_init(generate_vault).clone()
}

/// Counts the generations that actually wrote files, for the regression test.
/// The marker path writes nothing, so a reused vault leaves this at 0.
static GENERATIONS: AtomicUsize = AtomicUsize::new(0);

fn generate_vault() -> PathBuf {
    let base = std::env::temp_dir().join(format!("marklet-vault-perf-{NOTES}"));
    let root = base.join("vault");
    let marker = base.join(".generated");

    if marker.exists() && std::fs::read_to_string(&marker).ok().as_deref() == Some(VERSION) {
        return root.canonicalize().expect("canonical vault root");
    }

    GENERATIONS.fetch_add(1, Ordering::Relaxed);
    let _ = std::fs::remove_dir_all(&base);
    for f in 0..FOLDERS {
        std::fs::create_dir_all(root.join(format!("folder-{f:02}"))).expect("folder");
    }

    for i in 0..NOTES {
        let folder = i % FOLDERS;
        let body = note_body(i);
        std::fs::write(
            root.join(format!("folder-{folder:02}/note-{i:04}.md")),
            body,
        )
        .expect("note");
    }

    std::fs::write(&marker, VERSION).expect("marker");
    root.canonicalize().expect("canonical vault root")
}

/// Bumped when [`note_body`] changes, so a stale vault from an older run is
/// regenerated instead of quietly measuring different content.
const VERSION: &str = "1";

fn note_body(i: usize) -> String {
    let a = (i * 7 + 3) % NOTES;
    let b = (i * 13 + 11) % NOTES;
    let planted = if i % 50 == 0 {
        "This paragraph has a haystack and a quicksilver in it.\n\n"
    } else if i % 7 == 0 {
        "This paragraph has a haystack but not the other one.\n\n"
    } else {
        ""
    };

    format!(
        "---\ntitle: Note {i}\ntags: [generated, perf]\n---\n\n\
         # Note {i}\n\n\
         Prose that exists so the parser has something ordinary to do, of about \
         the length a real note's opening paragraph has, give or take a line.\n\n\
         {planted}\
         ## Links\n\n\
         See [[note-{a:04}]] and [[note-{b:04}]] for context, plus \
         [[missing-note-{i}]] which was never written.\n\n\
         ## Detail\n\n\
         - a list item\n- another one\n- a third\n\n\
         | column | value |\n|---|---|\n| one | {i} |\n\n\
         ```rust\nfn main() {{ println!(\"{i}\"); }}\n```\n\n\
         Closing prose, again about the length of a real one.\n"
    )
}

/// A search whose mode is **not known at compile time**.
///
/// This exists for the size measurement as much as for the behaviour. Every
/// other call in this file builds its `Query` from a literal, so fat LTO can
/// prove `query.regex` is false, delete the `regex-search` branch and strip the
/// whole ripgrep stack with it — which made the first attempt at measuring what
/// the feature costs report 896 bytes. Reading the flag from the environment
/// makes the branch genuinely reachable, so the two builds differ by what the
/// feature actually adds.
///
/// Set `MARKLET_REGEX=1` to take the regex path; in a lite build it reports
/// `regex_unavailable` and searches literally, which is the documented
/// behaviour and is asserted here.
#[test]
fn a_runtime_chosen_search_mode_keeps_both_paths_linked() {
    let root = generated_vault();
    let regex = std::env::var("MARKLET_REGEX").is_ok();

    let mut hits = 0usize;
    let stats = search::search(
        &root,
        &Query {
            text: if regex {
                r"quick\w+".into()
            } else {
                "quicksilver".into()
            },
            regex,
            limit: 10,
            lines_per_file: 1,
        },
        &mut |_| {
            hits += 1;
            true
        },
    );

    let unavailable = regex && cfg!(not(feature = "regex-search"));
    assert_eq!(
        stats.regex_unavailable, unavailable,
        "lite says so rather than silently answering a different question"
    );
    // A lite build handed `quick\w+` searches for those nine characters
    // literally and finds nothing — which is exactly why it also sets
    // `regex_unavailable`, so the UI can say "regex needs the full edition"
    // instead of letting the user conclude the vault is empty.
    assert_eq!(hits > 0, !unavailable, "{hits} hits, regex={regex}");
}

/// The fixture is built once, however many tests ask for it at the same moment.
///
/// This is the regression test for a green local suite and a red CI one. The
/// tests in this file share a generated vault; each of them used to check the
/// marker and then regenerate, so on a fresh runner several threads rewrote the
/// same 5 000 files at once. Whichever test was between its cold and warm index
/// build when a neighbour rewrote the notes saw every mtime move, and
/// `warm.cached == NOTES` reported a fraction — 1 086 of 5 000 on the run that
/// found this.
///
/// It hammers `generated_vault` from more threads than the harness itself uses.
/// Reverting the barrier fails it 4 times out of 4 on an empty temp directory
/// and passes it every time on a populated one — which is the honest shape of
/// this test and the whole reason the bug lived: a CI runner is always the
/// first case and a developer's machine, after one run, is always the second.
/// It fires where the defect is, not where it is convenient to observe.
#[test]
fn the_generated_vault_is_built_at_most_once_per_process() {
    let roots: Vec<PathBuf> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..8).map(|_| scope.spawn(generated_vault)).collect();
        threads
            .into_iter()
            .map(|t| {
                t.join()
                    .expect("no thread was left generating a half-deleted vault")
            })
            .collect()
    });

    assert!(
        roots.windows(2).all(|w| w[0] == w[1]),
        "every caller got the same vault: {roots:?}"
    );
    let generations = GENERATIONS.load(Ordering::Relaxed);
    assert!(
        generations <= 1,
        "the vault was written {generations} times; concurrent callers must wait, not regenerate"
    );
    // Whatever the count, the vault the callers were handed is whole — a
    // barrier that let a caller through mid-generation would pass the count
    // and still hand out a tree that is still being written.
    assert_eq!(
        std::fs::read_dir(roots[0].join("folder-00"))
            .expect("the first folder exists")
            .count(),
        NOTES / FOLDERS
    );
}

/// The vault must never read outside its root, whatever a note asks for.
#[test]
fn a_generated_vault_refuses_to_read_outside_itself() {
    let root = generated_vault();
    assert!(scan::resolve_in(&root, "folder-00/note-0000.md").is_ok());
    for escape in [
        "../../etc/passwd",
        "../.generated",
        "folder-00/../../.generated",
    ] {
        assert!(
            scan::resolve_in(&root, escape).is_err(),
            "{escape} must be refused"
        );
    }
    assert!(Path::new(&root).is_absolute());
}

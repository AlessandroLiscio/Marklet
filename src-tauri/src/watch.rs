//! Live reload: watch one file, debounce the burst a save produces, call back
//! once.
//!
//! Editors do not save with a single write. vim writes a swap file then
//! renames over the target; VS Code and most GUI editors write-then-rename
//! too, and the OS may report a chmod alongside either. That is three-plus
//! raw filesystem events for what the user experiences as "I saved" — firing
//! `file-changed` once per raw event would flash the document repeatedly and,
//! worse, could reopen mid-rename and read a half-written file. Coalescing
//! the whole burst into one callback is this module's entire job.
//!
//! **Not a `#[tauri::command]`.** `src/ipc.rs` stays the only module that
//! declares one — see `.claude/skills/tauri-ipc/SKILL.md`. This module
//! exposes a plain `pub fn watch`, taking a plain path and a plain callback,
//! so it is headless and testable exactly like `render::` and `protocol::`.
//! The caller (main thread, wired at wave close) supplies a callback that
//! emits the `file-changed` event to the webview.

use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use notify::{Error as NotifyError, Event, RecommendedWatcher, RecursiveMode, Watcher as _};

/// How long to wait after the last raw event before firing the callback.
/// Long enough to coalesce a save's write+rename+chmod burst (those land
/// well under 50 ms apart on both ext4/WSL2 inotify and NTFS's
/// `ReadDirectoryChangesW`), short enough that a reload still feels
/// immediate to the person who just hit save.
const DEBOUNCE: Duration = Duration::from_millis(120);

/// A live watch. Dropping it stops the OS-level watch and joins the debounce
/// thread, so a document closed mid-watch does not leak a background thread.
pub struct Watch {
    // `Option` so `Drop` can take and drop it explicitly, on purpose, before
    // joining the thread below — see that impl for why the order matters.
    watcher: Option<RecommendedWatcher>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        // Dropping the watcher first stops the OS-level watch AND drops the
        // `EventHandler` closure it owns, which is what drops the debounce
        // channel's sender. That is what makes the receiver on the debounce
        // thread return `Err` and the thread's loop exit, so the join below
        // returns instead of blocking forever.
        //
        // Letting the compiler drop fields in declaration order instead would
        // run this function's *body* — the join — before either field is
        // auto-dropped, which is backwards: the join would wait on a channel
        // that is still open. Dropping `watcher` by hand first is what fixes
        // the ordering.
        self.watcher.take();
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Watches a single file and calls `on_change` at most once per debounce
/// window, no matter how many raw OS events one save produces.
///
/// `path` must be a file, not a directory — `notify` watches either, but a
/// live-reload target is always one open document. `on_change` runs on the
/// debounce thread, not the notify callback thread; it must be cheap or hand
/// off to somewhere that is (an `emit` to the webview qualifies).
pub fn watch(path: &Path, on_change: impl Fn() + Send + 'static) -> Result<Watch, NotifyError> {
    let (tx, rx) = mpsc::channel::<()>();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, NotifyError>| {
        if res.is_ok() {
            // A full channel here would mean thousands of unprocessed raw
            // events for one watched file, which is not a real scenario —
            // and an unbounded channel keeps the sender's `send` from ever
            // failing for a reason unrelated to "the receiver is gone".
            let _ = tx.send(());
        }
    })?;
    watcher.watch(path, RecursiveMode::NonRecursive)?;

    let thread = thread::spawn(move || coalesce(&rx, DEBOUNCE, on_change));

    Ok(Watch {
        watcher: Some(watcher),
        thread: Some(thread),
    })
}

/// Turns a burst of `()` signals on `rx` into calls to `on_change`, at most
/// one per `window` of silence.
///
/// Split out from [`watch`] so the coalescing logic is testable on its own,
/// fast and deterministically — driven by a plain channel instead of real
/// filesystem events, which would make the test slow and flaky across
/// platforms for no benefit: the part worth proving here is the timing logic,
/// not that `notify` delivers events (it does; that is `notify`'s own test
/// suite's job).
///
/// Blocks the calling thread until `rx` disconnects, so callers run it on its
/// own thread.
fn coalesce(rx: &mpsc::Receiver<()>, window: Duration, mut on_change: impl FnMut()) {
    // Block for the first signal of a new burst.
    while rx.recv().is_ok() {
        // Keep draining and pushing the deadline out on every further signal
        // that arrives inside the window — this is what turns N raw events
        // into exactly one callback instead of firing per event.
        let mut deadline = Instant::now() + window;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match rx.recv_timeout(deadline - now) {
                Ok(()) => deadline = Instant::now() + window,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        on_change();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A short window so the test suite stays fast; the timing logic being
    /// proven does not care what the constant's actual value is.
    const TEST_WINDOW: Duration = Duration::from_millis(30);

    #[test]
    fn coalesces_a_burst_into_exactly_one_callback() {
        let (tx, rx) = mpsc::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&count);
        let handle = thread::spawn(move || {
            coalesce(&rx, TEST_WINDOW, move || {
                counted.fetch_add(1, Ordering::SeqCst);
            });
        });

        // Simulate a save's write + rename + chmod: three raw signals a few
        // ms apart, well inside the debounce window.
        for _ in 0..3 {
            tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(5));
        }
        // Let the window elapse with no further signal so the callback fires.
        thread::sleep(TEST_WINDOW * 3);
        assert_eq!(count.load(Ordering::SeqCst), 1, "one burst, one callback");

        drop(tx);
        handle.join().unwrap();
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "no extra callback on shutdown"
        );
    }

    #[test]
    fn two_bursts_separated_by_the_window_are_two_callbacks() {
        let (tx, rx) = mpsc::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&count);
        let handle = thread::spawn(move || {
            coalesce(&rx, TEST_WINDOW, move || {
                counted.fetch_add(1, Ordering::SeqCst);
            });
        });

        tx.send(()).unwrap();
        thread::sleep(TEST_WINDOW * 3); // let the first burst's callback fire
        tx.send(()).unwrap();
        thread::sleep(TEST_WINDOW * 3); // let the second burst's callback fire

        assert_eq!(count.load(Ordering::SeqCst), 2);

        drop(tx);
        handle.join().unwrap();
    }

    #[test]
    fn an_idle_receiver_never_calls_back() {
        let (tx, rx) = mpsc::channel();
        let count = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&count);
        let handle = thread::spawn(move || {
            coalesce(&rx, TEST_WINDOW, move || {
                counted.fetch_add(1, Ordering::SeqCst);
            });
        });

        drop(tx); // disconnect with no signal ever sent
        handle.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    /// End-to-end through the real `notify` backend: proves `watch` wires a
    /// real file to `coalesce` correctly, on top of `coalesce`'s own focused
    /// timing tests above.
    #[test]
    fn watch_reports_a_real_file_write() {
        let dir = std::env::temp_dir().join(format!("marklet-watch-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("watched.md");
        fs::write(&path, "before").unwrap();

        let count = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&count);
        let _watch = watch(&path, move || {
            counted.fetch_add(1, Ordering::SeqCst);
        })
        .expect("watch a real file");

        // A save on top of an existing file: write + rename over it, the same
        // burst shape a text editor produces.
        let tmp = dir.join("watched.md.tmp");
        fs::write(&tmp, "after").unwrap();
        fs::rename(&tmp, &path).unwrap();

        // Generous poll: real filesystem events, unlike the synthetic tests
        // above, are not on a clock this test controls.
        let deadline = Instant::now() + Duration::from_secs(5);
        while count.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "exactly one callback for one save"
        );
    }
}

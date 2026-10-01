//! Marklet — a reading-first Markdown viewer.
//!
//! The crate is split so that the parts which must run without a window stay
//! free of `tauri`:
//!
//! - [`render`] turns bytes into HTML plus the offset map. Headless; used by
//!   `MD_HTML=1` as well as by the app.
//! - [`export`] turns a rendered document into a standalone file. Headless.
//! - [`cli`] parses argv/env into a [`cli::Mode`] and runs every headless path
//!   to completion, before Tauri is ever built.
//! - [`webpath`] is the shared URL-to-file plumbing both of those and the
//!   protocol need.
//! - [`vault`] scans, indexes and searches a folder of notes, and resolves
//!   `[[wiki-links]]` for the render core through a callback.
//! - [`store`] persists settings and reading positions; [`watch`] reports that
//!   a file changed on disk.
//! - [`platform`] is OS integration — file association and the Explorer verbs.
//!
//! [`ipc`] is the **only** module declaring `#[tauri::command]`, and
//! [`protocol`] the only way a file reaches the webview. Every module above
//! exports plain functions; `ipc` wraps them. That is what keeps the whole
//! privileged surface readable in one sitting instead of scattered across six
//! files — see `.claude/skills/tauri-ipc/SKILL.md`.

pub mod cli;
pub mod editor;
pub mod export;
pub mod ipc;
pub mod platform;
pub mod protocol;
pub mod render;
pub mod store;
pub mod vault;
pub mod watch;
pub mod webpath;

use std::time::Instant;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::protocol::AssetRoot;
use crate::vault::VaultState;

/// Boots the windowed application.
///
/// **The cold-start budget lives on this function.** Two things protect it, and
/// both are easy to undo without noticing:
///
/// 1. **The document is parsed to HTML before the window exists** and injected
///    as `window.__MARKLET_BOOT__` through an initialization script. The first
///    paint therefore already has content: no IPC round trip, no spinner, no
///    second layout. Moving this to a command after the window opens costs a
///    full round trip on the one path where it is most visible.
/// 2. **The window is created hidden and shown from the first frame** that has
///    that HTML in the DOM. This removes the white flash which makes an app
///    *feel* slow even when its numbers are fine.
///
/// The window is built here rather than declared in `tauri.conf.json` precisely
/// because the initialization script has to be attached *before* creation, and
/// a config-declared window is already open by the time `setup` runs.
pub fn run(job: cli::WindowJob, started: Instant) {
    let root = AssetRoot::new();

    let vault = VaultState::new();

    // A directory argument opens a vault rather than failing as "not a
    // document". This is not a convenience: the `Directory\shell\marklet_vault`
    // verb that phase P6 registers passes exactly this, so without it the
    // "Open folder as Vault" context-menu entry would launch the app and
    // immediately show an error.
    let launch_is_vault = job.file.as_deref().is_some_and(|p| p.is_dir());
    let vault_path = if launch_is_vault {
        job.file
            .as_deref()
            .and_then(|path| vault.open(&root, path).ok().map(|info| info.root))
    } else {
        None
    };

    // Read and render before anything touches a webview. On a warm cache this
    // is single-digit milliseconds for an ordinary document, and it is the
    // whole reason the first paint has content.
    let boot = if launch_is_vault {
        Ok(None)
    } else {
        job.file
            .as_deref()
            .map(|path| ipc::open_path(&root, None, path))
            .transpose()
    };

    let (mut boot_script, title) = match &boot {
        Ok(Some(doc)) => (boot_script(doc), doc.title.clone()),
        Ok(None) => (String::new(), "Marklet".to_string()),
        Err(err) => (boot_error_script(err), "Marklet".to_string()),
    };

    if let Some(path) = &vault_path {
        boot_script.push_str(&vault_boot_script(path));
    }

    // `--settings` is a panel in this window, not a second one. A separate
    // settings window is another WebView2 instance, roughly 40 MB RSS, for a
    // surface with nine controls on it. The flag therefore reaches the frontend
    // the same way the document does — before the window exists — so the panel
    // is open on the first frame rather than opening visibly a moment later.
    if job.settings {
        boot_script.push_str("window.__MARKLET_SETTINGS__ = true;");
    }

    let protocol_root = root.clone();
    let watch_target = if launch_is_vault {
        None
    } else {
        job.file.clone()
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            // A second WebView2 instance costs roughly 40 MB RSS. Forward the
            // path to the window that already exists instead of spawning one.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                if let Some((event, path)) = forwarded_open(&cwd, &argv) {
                    let _ = window.emit(event, path);
                }
            }
        }))
        .manage(root)
        .manage(vault)
        // `MD_EDITOR`, parsed once by `cli::parse`. Held here so `F4` can ask
        // for "my editor, at this line" without the webview ever naming a
        // program — see `editor.rs`'s module doc for why that matters.
        .manage(ipc::EditorSetting(job.editor.clone()))
        // The clock `paint-ms` is measured against. Started in `main.rs`, as
        // the first statement of the program.
        .manage(ipc::StartedAt(
            started,
            std::sync::atomic::AtomicBool::new(false),
        ))
        // The watcher handle has to outlive `setup`, or `notify` stops watching
        // the moment the function returns and live reload silently never fires.
        .manage(ipc::PickedSave::default())
        .manage(std::sync::Mutex::new(Option::<watch::Watch>::None))
        .register_uri_scheme_protocol("marklet", move |_ctx, request| {
            protocol::handle(&protocol_root, &request)
        })
        .invoke_handler(tauri::generate_handler![
            ipc::open_document,
            ipc::open_note,
            ipc::read_settings,
            ipc::write_settings,
            ipc::reading_position,
            ipc::record_reading_position,
            ipc::open_vault,
            ipc::close_vault,
            ipc::scan_vault,
            ipc::index_vault,
            ipc::search_vault,
            ipc::resolve_wikilink,
            ipc::backlinks_for,
            ipc::splice_range,
            ipc::read_source,
            ipc::save_pasted_image,
            ipc::reveal_in_editor,
            ipc::export_pdf,
            ipc::export_html,
            ipc::open_external,
            ipc::report_ready,
            ipc::list_dir,
            ipc::pick_file,
            ipc::pick_folder,
            ipc::pick_save,
        ])
        .setup(move |app| {
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(title)
                .inner_size(1000.0, 760.0)
                .min_inner_size(420.0, 320.0)
                // Shown by the frontend on the first frame that has content.
                .visible(false)
                .initialization_script(&boot_script)
                .build()?;

            // Live reload. Debounced in `watch.rs` because saving from an editor
            // produces a burst — write, rename, chmod — and re-rendering three
            // times for one save is visible.
            if let Some(path) = watch_target {
                let notify_window = window.clone();
                match watch::watch(&path, move || {
                    let _ = notify_window.emit("file-changed", ());
                }) {
                    Ok(handle) => {
                        if let Some(slot) =
                            app.try_state::<std::sync::Mutex<Option<watch::Watch>>>()
                        {
                            if let Ok(mut guard) = slot.lock() {
                                *guard = Some(handle);
                            }
                        }
                    }
                    // A file that cannot be watched still opens and still reads.
                    // Losing live reload is worth saying out loud and worth
                    // nothing more than that.
                    Err(e) => eprintln!("live reload unavailable: {e}"),
                }
            }

            // stderr, not stdout: stdout belongs to `MD_HTML=1`, and a
            // diagnostic that corrupts a pipe is worse than no diagnostic.
            // `release.yml`'s cold-start gate greps this exact string.
            // The window exists. This is NOT when the user can read anything
            // — the bundle has not been fetched and Svelte has not mounted.
            // `paint-ms`, reported by the frontend through `report_ready`, is
            // that number, and it is the one the cold-start gate uses.
            eprintln!("boot-ms={}", started.elapsed().as_millis());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Marklet");
}

/// Builds the `window.__MARKLET_BOOT__` assignment.
///
/// Serialised with `serde_json` rather than string-formatted: the payload
/// contains arbitrary document text, and hand-quoting it would be an injection
/// bug in the one place that handles untrusted input before the sanitizer's
/// output reaches the DOM.
fn boot_script(doc: &ipc::OpenedDocument) -> String {
    match serde_json::to_string(doc) {
        Ok(json) => format!("window.__MARKLET_BOOT__ = {};", escape_js_literal(&json)),
        Err(_) => String::new(),
    }
}

/// Tells the frontend a vault is already open, so the sidebar can populate
/// without a round trip asking which one.
fn vault_boot_script(path: &str) -> String {
    match serde_json::to_string(path) {
        Ok(json) => format!("window.__MARKLET_VAULT__ = {};", escape_js_literal(&json)),
        Err(_) => String::new(),
    }
}

fn boot_error_script(err: &ipc::IpcError) -> String {
    match serde_json::to_string(err) {
        Ok(json) => format!(
            "window.__MARKLET_BOOT_ERROR__ = {};",
            escape_js_literal(&json)
        ),
        Err(_) => String::new(),
    }
}

/// What a second launch asked for, as the event the running window should get.
///
/// **A folder is not a file.** Every forwarded path used to be emitted as
/// `open-file`, and the frontend opened each one as a document — so with
/// Marklet already running, Explorer's *Open folder as Vault* tried to read a
/// directory as markdown and failed, while the same verb with Marklet closed
/// opened the vault. Deciding here, rather than in the webview, is not a
/// preference: the webview has no filesystem permission and cannot ask whether
/// a path is a directory.
///
/// **A relative path is relative to the second launch.** `marklet .` typed in
/// a terminal means *that* terminal's directory, which the plugin hands over
/// as `cwd`; resolving it against the first instance's directory instead opened
/// whatever folder Marklet had happened to start in. `Path::join` leaves an
/// absolute path untouched, which is what Explorer always sends.
fn forwarded_open(cwd: &str, argv: &[String]) -> Option<(&'static str, String)> {
    let arg = argv.iter().skip(1).find(|a| !a.starts_with('-'))?;
    let path = std::path::Path::new(cwd).join(arg);
    let event = if path.is_dir() {
        "open-folder"
    } else {
        "open-file"
    };
    Some((event, path.display().to_string()))
}

/// U+2028 and U+2029 are legal inside a JSON string and were, historically,
/// line terminators inside a JavaScript one. Escaping them costs nothing and
/// removes a class of "works everywhere except one webview" bug.
fn escape_js_literal(json: &str) -> String {
    json.replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        std::iter::once("marklet")
            .chain(args.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn a_forwarded_folder_opens_as_a_folder() {
        // The reported case: Open folder as Vault while Marklet is running.
        let dir = std::env::temp_dir();
        let (event, path) = forwarded_open("/", &argv(&[dir.to_str().unwrap()])).unwrap();
        assert_eq!(event, "open-folder");
        assert_eq!(path, dir.display().to_string());
    }

    #[test]
    fn a_forwarded_file_still_opens_as_a_file() {
        let file = std::env::temp_dir().join("marklet-forwarded-open-test.md");
        std::fs::write(&file, "# x").unwrap();
        let (event, _) = forwarded_open("/", &argv(&[file.to_str().unwrap()])).unwrap();
        std::fs::remove_file(&file).ok();
        assert_eq!(event, "open-file");
    }

    #[test]
    fn a_relative_path_resolves_against_the_second_launch() {
        // `marklet .` means the terminal it was typed in, not wherever the
        // first instance happened to start.
        let here = std::env::temp_dir();
        let (event, path) = forwarded_open(here.to_str().unwrap(), &argv(&["."])).unwrap();
        assert_eq!(event, "open-folder");
        assert_eq!(path, here.join(".").display().to_string());
    }

    #[test]
    fn flags_are_not_paths() {
        assert!(forwarded_open("/", &argv(&["--settings"])).is_none());
        assert!(forwarded_open("/", &argv(&[])).is_none());
    }

    #[test]
    fn line_separators_are_escaped_for_javascript() {
        let json = "\"a\u{2028}b\u{2029}c\"";
        let escaped = escape_js_literal(json);
        assert!(!escaped.contains('\u{2028}'));
        assert!(!escaped.contains('\u{2029}'));
        assert!(escaped.contains("\\u2028"));
        assert!(escaped.contains("\\u2029"));
    }

    #[test]
    fn ordinary_json_passes_through_unchanged() {
        let json = r#"{"html":"<p>hi</p>"}"#;
        assert_eq!(escape_js_literal(json), json);
    }
}

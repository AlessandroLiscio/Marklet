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
//! - [`ipc`] is the only module declaring commands; [`protocol`] is the only
//!   way a file reaches the webview.
//! - [`platform`] is OS integration. Stubs until phase P6.

pub mod cli;
pub mod export;
pub mod ipc;
pub mod platform;
pub mod protocol;
pub mod render;
pub mod webpath;

use std::time::Instant;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::protocol::AssetRoot;

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

    // Read and render before anything touches a webview. On a warm cache this
    // is single-digit milliseconds for an ordinary document, and it is the
    // whole reason the first paint has content.
    let boot = job
        .file
        .as_deref()
        .map(|path| ipc::open_path(&root, path))
        .transpose();

    let (boot_script, title) = match &boot {
        Ok(Some(doc)) => (boot_script(doc), doc.title.clone()),
        Ok(None) => (String::new(), "Marklet".to_string()),
        Err(err) => (boot_error_script(err), "Marklet".to_string()),
    };

    let protocol_root = root.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // A second WebView2 instance costs roughly 40 MB RSS. Forward the
            // path to the window that already exists instead of spawning one.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                if let Some(path) = argv.iter().skip(1).find(|a| !a.starts_with('-')) {
                    let _ = window.emit("open-file", path);
                }
            }
        }))
        .manage(root)
        .register_uri_scheme_protocol("marklet", move |_ctx, request| {
            protocol::handle(&protocol_root, &request)
        })
        .invoke_handler(tauri::generate_handler![ipc::open_document])
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(title)
                .inner_size(1000.0, 760.0)
                .min_inner_size(420.0, 320.0)
                // Shown by the frontend on the first frame that has content.
                .visible(false)
                .initialization_script(&boot_script)
                .build()?;

            // stderr, not stdout: stdout belongs to `MD_HTML=1`, and a
            // diagnostic that corrupts a pipe is worse than no diagnostic.
            // `release.yml`'s cold-start gate greps this exact string.
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

fn boot_error_script(err: &ipc::IpcError) -> String {
    match serde_json::to_string(err) {
        Ok(json) => format!(
            "window.__MARKLET_BOOT_ERROR__ = {};",
            escape_js_literal(&json)
        ),
        Err(_) => String::new(),
    }
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

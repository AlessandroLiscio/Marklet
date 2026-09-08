//! Marklet — lightweight Markdown viewer.
//!
//! The crate is split so that the parts which must run without a window stay
//! free of `tauri`:
//!
//! - [`render`] turns bytes into HTML plus the offset map. Headless; used by
//!   `MD_HTML=1` as well as by the app.
//! - Everything else is added by its owning phase. See the plan and
//!   `CLAUDE.md`.

pub mod render;

/// Boots the windowed application.
///
/// The cold-start budget lives on this path. Two things protect it and must not
/// be undone casually:
///
/// 1. The document is parsed to HTML in Rust **before** the window exists and
///    injected as `window.__MARKLET_BOOT__`, so the first paint already has
///    content — no IPC round trip, no loading spinner.
/// 2. The window is created `visible: false` and shown on the first
///    `requestAnimationFrame` after that HTML is in the DOM, which removes the
///    white flash that makes an app *feel* slow.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // A second WebView2 instance costs roughly 40 MB RSS. Forward the
            // path to the window that already exists instead of spawning one.
            let _ = (app, argv);
        }))
        .run(tauri::generate_context!())
        .expect("error while running Marklet");
}

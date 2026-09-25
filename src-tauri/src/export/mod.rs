//! Export formats.
//!
//! `html` is kept free of `tauri` like [`crate::render`]: its `export_file`
//! is reached from `MD_HTML=1`, which has to run and exit before
//! `tauri::Builder::build()`. `pdf` has no such constraint — it is reached
//! only from a live, already-open webview (a Tauri command, not a CLI path),
//! so it may reference platform webview types; see its own module doc.

pub mod html;
pub mod pdf;

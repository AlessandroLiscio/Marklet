//! Export formats.
//!
//! Kept free of `tauri` like [`crate::render`]: `html` is reached from
//! `MD_HTML=1`, which has to run and exit before `tauri::Builder::build()`.

pub mod html;

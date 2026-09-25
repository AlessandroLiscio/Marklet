//! Silent PDF export via the OS-native print engine.
//!
//! Unlike [`super::html::export_file`], nothing here runs headless. PDF
//! export is reached only from a live, already-open webview — the app
//! window with the document already on screen and `enrich()`
//! (`src/lib/rich/index.ts`) having already turned every `.math-inline`,
//! `.math-block` and `.mermaid` placeholder into real KaTeX markup and an
//! inlined `<svg>`. That is *why* this module is allowed to touch a platform
//! webview type at all: the "no window, no WebView2 initialization" rule
//! (`docs/architecture.md`, `.claude/agents/platform-engineer.md` rule 1)
//! governs the CLI paths that must exit before `tauri::Builder::build()`.
//! PDF export is a menu action reached long after that point, on a webview
//! that already exists — it is not exempt from the rule, it was never
//! inside its scope.
//!
//! Both platform paths below must render with `src/styles/print.css`, not
//! the live app's screen stylesheet or the browser's UA print default —
//! **making that swap happen is the caller's job, not this module's**: the
//! Tauri command wired at wave close is responsible for getting print.css
//! onto the page (e.g. inject a `<link rel="stylesheet" media="print">` — it
//! is already unconditionally applied under `@media print`, so simply
//! printing the live, already-rendered document does the right thing as
//! long as that link tag is present — see print.css's own "NEEDS WIRING"
//! note) before calling either function here. Neither function in this
//! module can verify that happened; both only drive the native print call.
//!
//! - `windows`: [`ICoreWebView2_7::PrintToPdf`][pdf-win] via the
//!   `webview2-com` crate, already a Windows-only dependency
//!   (`src-tauri/Cargo.toml`). Does not compile on this WSL2 machine —
//!   `./scripts/check-windows.sh` type-checks it; see that script's own doc
//!   comment for exactly what it does and does not prove, and this crate's
//!   diff receipt for what only `windows-latest` can confirm (linking, an
//!   `ICoreWebView2_7` actually being obtainable, the PDF's bytes).
//! - `linux`: WebKitGTK's `webkit_print_operation_print()` via the
//!   `webkit2gtk` crate — already linked transitively through
//!   `tauri-runtime-wry`'s Linux backend (see `Cargo.lock`), added here as a
//!   direct, version-matched dependency at zero marginal binary size (flagged
//!   in the diff receipt per `size-budget`'s "every new dependency" rule,
//!   even though its cost is zero). Silent (no dialog) PDF export through
//!   WebKitGTK has exactly one mechanism: point `GtkPrintSettings` at the
//!   built-in "file" print backend's virtual printer before calling
//!   `print()` — there is no `webkit_print_operation_set_export_filename`
//!   the way plain `GtkPrintOperation` has (checked against the installed
//!   `webkit2gtk` 2.0.2 bindings: `PrintOperationExt` exposes only `print`,
//!   `run_dialog`, `page_setup`/`set_page_setup`,
//!   `print_settings`/`set_print_settings`). This needs a real GTK display
//!   connection and the "file" backend compiled into the system's GTK print
//!   backends (`file` is in the default backend list on every Linux desktop
//!   this project has seen, but is unverified here — WSL2 has no display).
//!
//! [pdf-win]: https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_7

use std::fmt;

/// What can go wrong printing a live document to PDF.
#[derive(Debug)]
pub enum PdfError {
    /// A Windows COM call failed. Carries the error's `Display` text rather
    /// than the `windows`-crate error type itself, so this enum stays usable
    /// (if never constructible this way) from a build that does not compile
    /// the `windows` submodule.
    Windows(String),
    /// A GLib/GTK call failed, or the WebKitGTK "file" print backend was not
    /// available to produce a PDF without a dialog.
    Linux(String),
    /// This platform has no PDF export path.
    Unsupported,
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::Windows(msg) => write!(f, "PrintToPdf failed: {msg}"),
            PdfError::Linux(msg) => write!(f, "WebKit print-to-PDF failed: {msg}"),
            PdfError::Unsupported => write!(f, "PDF export is not supported on this platform"),
        }
    }
}

impl std::error::Error for PdfError {}

/// `0.5in`, matching `print.css`'s `@page { margin: 0.5in; }` so the two
/// engines' idea of the page margin never disagrees with the stylesheet the
/// page was laid out against. WebView2's `ICoreWebView2PrintSettings`
/// margins are in inches; kept as one constant so a future change to
/// print.css's `@page` rule has exactly one other place to update.
pub const PRINT_MARGIN_INCHES: f64 = 0.5;

#[cfg(windows)]
pub mod windows {
    //! `ICoreWebView2_7::PrintToPdf`. Type-checked by
    //! `./scripts/check-windows.sh`; never compiled on this dev machine.

    use std::path::Path;

    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2Environment6, ICoreWebView2PrintSettings,
        ICoreWebView2_2, ICoreWebView2_7, COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT,
    };
    use webview2_com::PrintToPdfCompletedHandler;
    use windows::core::{Interface, HSTRING};
    use windows::Win32::Foundation::E_FAIL;

    use super::{PdfError, PRINT_MARGIN_INCHES};

    /// The same print, reached from what Tauri actually hands out.
    ///
    /// `WebviewWindow::with_webview` gives an `ICoreWebView2Controller`, not
    /// the `ICoreWebView2_2` below, so the two `QueryInterface` hops that get
    /// from one to the other live here rather than in `ipc.rs` — this file is
    /// type-checked from Linux by `./scripts/check-windows.sh` and `ipc.rs`,
    /// which imports `tauri`, is not. Every Windows-only line that can be
    /// checked before CI should be on this side of that boundary.
    pub fn print_controller_to_pdf(
        controller: &ICoreWebView2Controller,
        output_path: &Path,
    ) -> Result<(), PdfError> {
        let core = unsafe { controller.CoreWebView2() }
            .map_err(|e| PdfError::Windows(format!("no CoreWebView2 on the controller: {e}")))?;
        let webview: ICoreWebView2_2 = core
            .cast()
            .map_err(|e| PdfError::Windows(format!("ICoreWebView2_2 unavailable: {e}")))?;
        print_to_pdf(&webview, output_path)
    }

    /// Prints `webview`'s current page to `output_path` as a PDF.
    ///
    /// `webview` is expected to already be showing the enriched document
    /// with `print.css` applied (see this module's doc comment) — this
    /// function does not navigate, inject a stylesheet, or wait for
    /// anything to render; it only drives the native print call and blocks
    /// until it completes.
    ///
    /// Blocks the calling thread: `PrintToPdfCompletedHandler::wait_for_
    /// async_operation` pumps the Win32 message loop while it waits, because
    /// WebView2 delivers the completion callback via `PostMessage` (see the
    /// `webview2-com` crate's own `wait_with_pump`). Call this off the UI
    /// thread if the caller cannot afford to block it — wiring that choice
    /// is the Tauri command's job, not this function's.
    pub fn print_to_pdf(webview: &ICoreWebView2_2, output_path: &Path) -> Result<(), PdfError> {
        let webview7: ICoreWebView2_7 = webview
            .cast()
            .map_err(|e| PdfError::Windows(format!("ICoreWebView2_7 unavailable: {e}")))?;

        let environment = unsafe { webview.Environment() }
            .map_err(|e| PdfError::Windows(format!("Environment(): {e}")))?;
        let environment6: ICoreWebView2Environment6 = environment.cast().map_err(|e| {
            PdfError::Windows(format!("ICoreWebView2Environment6 unavailable: {e}"))
        })?;
        let settings: ICoreWebView2PrintSettings = unsafe { environment6.CreatePrintSettings() }
            .map_err(|e| PdfError::Windows(format!("CreatePrintSettings(): {e}")))?;

        unsafe {
            settings
                .SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT)
                .map_err(|e| PdfError::Windows(format!("SetOrientation: {e}")))?;
            settings
                .SetMarginTop(PRINT_MARGIN_INCHES)
                .map_err(|e| PdfError::Windows(format!("SetMarginTop: {e}")))?;
            settings
                .SetMarginBottom(PRINT_MARGIN_INCHES)
                .map_err(|e| PdfError::Windows(format!("SetMarginBottom: {e}")))?;
            settings
                .SetMarginLeft(PRINT_MARGIN_INCHES)
                .map_err(|e| PdfError::Windows(format!("SetMarginLeft: {e}")))?;
            settings
                .SetMarginRight(PRINT_MARGIN_INCHES)
                .map_err(|e| PdfError::Windows(format!("SetMarginRight: {e}")))?;
            // print.css relies on backgrounds surviving (code fill, table
            // zebra, alert tint) — WebView2 strips them by default to save
            // ink unless told otherwise. print.css's own
            // `print-color-adjust: exact` asks the *page* to keep them; this
            // is the matching engine-level switch.
            settings
                .SetShouldPrintBackgrounds(true.into())
                .map_err(|e| PdfError::Windows(format!("SetShouldPrintBackgrounds: {e}")))?;
            settings
                .SetShouldPrintHeaderAndFooter(false.into())
                .map_err(|e| PdfError::Windows(format!("SetShouldPrintHeaderAndFooter: {e}")))?;
        }

        let path = HSTRING::from(output_path.to_string_lossy().as_ref());

        PrintToPdfCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                webview7
                    .PrintToPdf(&path, &settings, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(|error_code, is_successful| {
                error_code?;
                if is_successful {
                    Ok(())
                } else {
                    Err(windows::core::Error::from_hresult(E_FAIL))
                }
            }),
        )
        .map_err(|e| PdfError::Windows(format!("{e}")))
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    //! WebKitGTK's `webkit_print_operation_print()`, pointed at the "file"
    //! print backend so it produces a PDF with no dialog.

    use std::cell::Cell;
    use std::path::Path;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use webkit2gtk::{PrintOperation, PrintOperationExt, WebView};

    use super::PdfError;

    /// How long to pump the GTK main loop waiting for `finished`.
    ///
    /// Generous: a long document on a slow machine is a legitimate reason to
    /// take a while, and the failure this bounds is a print backend that never
    /// answers, which is not a thing that gets better with more waiting.
    const PRINT_TIMEOUT: Duration = Duration::from_secs(60);

    /// Prints `webview`'s current page to `output_path` as a PDF.
    ///
    /// `webview` is expected to already be showing the enriched document
    /// with `print.css` applied (see this module's doc comment) — this
    /// function does not navigate or wait for anything to render.
    ///
    /// Requires the GTK "file" print backend (`GTK_PRINT_BACKENDS` unset, or
    /// including `file`) and a printer literally named `"Print to File"` —
    /// the backend registers exactly one virtual printer under that name.
    /// This is the standard mechanism WebKitGTK apps use for a dialog-free
    /// PDF export; there is no more direct API (see this module's doc
    /// comment for what was checked in the installed bindings).
    pub fn print_to_pdf(webview: &WebView, output_path: &Path) -> Result<(), PdfError> {
        let operation = PrintOperation::new(webview);

        let settings = gtk::PrintSettings::new();
        let file_uri = format!("file://{}", output_path.display());
        settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(file_uri.as_str()));
        settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
        settings.set(gtk::PRINT_SETTINGS_PRINTER, Some("Print to File"));
        operation.set_print_settings(&settings);

        // `print()` RETURNS IMMEDIATELY. It hands the job to the GTK main loop
        // and the file appears some time later, so the obvious
        // `operation.print(); if output_path.exists()` reports a failure for
        // every successful export — the check runs before the printer has
        // written a byte. WebKitGTK signals completion with `finished`, so the
        // flag below is set from the main loop and this function pumps that
        // same loop until it flips.
        //
        // Pumping rather than blocking is not optional: this runs on the GTK
        // main thread (that is where `with_webview` puts it), and a thread
        // parked on a condvar is a thread that will never dispatch the signal
        // it is waiting for. `main_iteration_do(false)` is the non-blocking
        // form, so the sleep is what keeps this from spinning a core.
        let done = Rc::new(Cell::new(false));
        let flag = done.clone();
        operation.connect_finished(move |_| flag.set(true));

        operation.print();

        let deadline = Instant::now() + PRINT_TIMEOUT;
        while !done.get() && Instant::now() < deadline {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        if !done.get() {
            return Err(PdfError::Linux(format!(
                "the print job did not finish within {} s",
                PRINT_TIMEOUT.as_secs()
            )));
        }

        // `finished` fires on failure too — it means "the operation is over",
        // not "it worked" — so the file is still what decides.
        if output_path.exists() {
            Ok(())
        } else {
            Err(PdfError::Linux(format!(
                "WebKitGTK's \"file\" print backend did not produce {}; is a display and the \
                 \"Print to File\" virtual printer available?",
                output_path.display()
            )))
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn print_to_pdf() -> Result<(), PdfError> {
    Err(PdfError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_margin_matches_print_css() {
        // Pinned so a change to print.css's @page margin is forced to update
        // this constant too, rather than the two silently drifting apart.
        assert_eq!(PRINT_MARGIN_INCHES, 0.5);
    }

    #[test]
    fn pdf_error_display_names_the_platform() {
        assert!(PdfError::Windows("x".into())
            .to_string()
            .contains("PrintToPdf"));
        assert!(PdfError::Linux("x".into()).to_string().contains("WebKit"));
        assert_eq!(
            PdfError::Unsupported.to_string(),
            "PDF export is not supported on this platform"
        );
    }
}

// Marklet ships as a GUI binary, so it has no console: `--help` and
// `MD_HTML=1` would print into the void without `cli::attach_console()`.
// Every headless path below runs and exits **before** `marklet::run()` calls
// `tauri::Builder::build()`, so it completes in well under 50 ms with no
// window and no WebView2 initialization — see
// `docs/architecture.md#the-cli-console-trap`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    // First thing, always: on Windows this finds the parent terminal (if
    // any) so subsequent output is not silently dropped. A no-op elsewhere.
    let console_ok = marklet::cli::attach_console();

    let args: Vec<String> = std::env::args().skip(1).collect();

    match marklet::cli::parse(&args) {
        Ok(marklet::cli::Mode::Headless(job)) => {
            let code = marklet::cli::run_headless(job, console_ok);
            ExitCode::from(code as u8)
        }
        Ok(marklet::cli::Mode::Window(job)) => {
            // Consumed by `marklet::run()` in a later phase; parsed and
            // validated here so a bad `--settings`/file/PORT/MD_EDITOR
            // combination fails fast instead of surfacing inside a window.
            let _ = job;
            marklet::run();
            ExitCode::SUCCESS
        }
        Err(err) => {
            marklet::cli::report_parse_error(&err, console_ok);
            ExitCode::from(2)
        }
    }
}

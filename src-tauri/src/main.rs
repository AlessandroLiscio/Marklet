// Marklet ships as a GUI binary, so it has no console: `--help` and `MD_HTML=1`
// would print into the void. Phase P1c adds the `AttachConsole` shim and the
// early-exit dispatch that makes the CLI paths complete in under 50 ms with no
// window and no WebView2 initialization.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    marklet::run()
}

//! OS integration entry points. `cli.rs` calls these three functions
//! directly for `--install` / `--uninstall` / `--unbind`; this module is
//! just the per-platform dispatcher.
//!
//! - Windows: `windows.rs` is the single source of truth for every HKCU
//!   registry key Marklet writes. See the exact list in
//!   `.claude/skills/win-integration/SKILL.md`. `installer/hooks.nsh` shells
//!   out to `marklet.exe --install/--uninstall --silent` rather than
//!   duplicating that list — two copies of a registry key list drift, and
//!   the symptom is orphaned keys nobody notices for months.
//! - Linux: `linux.rs` writes a `.desktop` entry plus a small shared-mime-info
//!   package (for the `.mdown`/`.mkd` globs that `text/markdown` doesn't
//!   already carry) via `xdg-mime`, and best-effort refreshes
//!   `update-desktop-database`. Best-effort because Linux is the dev target
//!   here (see root `CLAUDE.md`), not every dev box has `desktop-file-utils`
//!   installed, and a missing refresh tool must not fail the CLI.
//! - Everything else: a documented no-op, so the contract stays stable on
//!   hosts this project doesn't ship to.

#[cfg(not(any(windows, target_os = "linux")))]
use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{install, unbind, uninstall};

#[cfg(target_os = "linux")]
pub use linux::{install, unbind, uninstall};

/// Registers the file association / desktop integration. No-op outside
/// Windows and Linux.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn install(silent: bool) -> io::Result<()> {
    log(silent, "--install has no effect on this platform");
    Ok(())
}

/// `unbind` plus context-menu verbs. No-op outside Windows and Linux.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn uninstall(silent: bool) -> io::Result<()> {
    log(silent, "--uninstall has no effect on this platform");
    Ok(())
}

/// Removes everything `install` wrote. No-op outside Windows and Linux.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn unbind(silent: bool) -> io::Result<()> {
    log(silent, "--unbind has no effect on this platform");
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn log(silent: bool, message: &str) {
    if !silent {
        eprintln!("marklet: {message}");
    }
}

#[cfg(all(test, not(any(windows, target_os = "linux"))))]
mod tests {
    use super::*;

    #[test]
    fn fallback_stubs_currently_succeed() {
        assert!(install(true).is_ok());
        assert!(uninstall(true).is_ok());
        assert!(unbind(true).is_ok());
    }
}

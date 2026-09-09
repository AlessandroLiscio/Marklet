//! Linux desktop integration: a `.desktop` entry plus a small
//! shared-mime-info package, both installed per-user under
//! `$XDG_DATA_HOME` (`~/.local/share` if unset) — never a system-wide path,
//! matching the Windows side's "HKCU only, no elevation" rule.
//!
//! Two pieces, both idempotent:
//!
//! - `applications/marklet.desktop` — advertises `Exec=` and
//!   `MimeType=text/markdown;`. Once `update-desktop-database` (best-effort;
//!   see below) refreshes the cache, Marklet appears in every file manager's
//!   "Open With" list for that MIME type. This alone is enough for `.md`
//!   and `.markdown`, which most distros already map to `text/markdown` via
//!   `shared-mime-info`.
//! - `mime/packages/marklet-markdown.xml` — a small MIME package adding the
//!   `*.mdown` / `*.mkd` globs to `text/markdown`, since `shared-mime-info`
//!   does not ship those by default and a file manager can't offer "Open
//!   With Marklet" for an extension it doesn't recognize as markdown at
//!   all. Installed and removed via the real `xdg-mime install` /
//!   `xdg-mime uninstall` (mirrors the two commands the win-integration
//!   skill names), which also re-run `update-mime-database` internally.
//!
//! Like the Windows side, this never sets a default handler — no
//! `xdg-mime default` call. Being listed under "Open With" is ours to do;
//! becoming the default is the user's call.
//!
//! `update-desktop-database` is best-effort: it ships in `desktop-file-utils`,
//! which is not installed on every dev box (this WSL2 machine included —
//! only `shared-mime-info`, which provides `xdg-mime` and
//! `update-mime-database`, is present). A missing refresh tool must not fail
//! `--install`/`--unbind`; the `.desktop` file and MIME package are still
//! written/removed correctly, just not immediately visible until the next
//! cache rebuild.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const DESKTOP_FILE_NAME: &str = "marklet.desktop";
const MIME_PACKAGE_NAME: &str = "marklet-markdown.xml";
const MIME_TYPE: &str = "text/markdown";

/// Writes the `.desktop` entry and the MIME package. Idempotent.
pub fn install(silent: bool) -> io::Result<()> {
    install_to(&data_home(), silent)
}

/// Identical to [`unbind`] — Linux has no separate "context-menu verb"
/// concept the way `SystemFileAssociations` does on Windows, so there is
/// nothing `--uninstall` adds on top.
pub fn uninstall(silent: bool) -> io::Result<()> {
    unbind(silent)
}

/// Removes everything `install` wrote.
pub fn unbind(silent: bool) -> io::Result<()> {
    unbind_from(&data_home(), silent)
}

// ---------------------------------------------------------------------
// Base-dir-parameterized implementation — the seam tests use to run
// against a temp XDG base instead of the developer's real
// `~/.local/share`.
// ---------------------------------------------------------------------

fn install_to(base: &Path, silent: bool) -> io::Result<()> {
    let exe = current_exe_string()?;

    let applications = base.join("applications");
    fs::create_dir_all(&applications)?;
    fs::write(applications.join(DESKTOP_FILE_NAME), desktop_entry(&exe))?;
    run_best_effort(
        silent,
        base,
        "update-desktop-database",
        &[applications.to_string_lossy().as_ref()],
    );

    let scratch = mime_package_scratch_path();
    fs::write(&scratch, mime_package_xml())?;
    run_best_effort(
        silent,
        base,
        "xdg-mime",
        &[
            "install",
            "--mode",
            "user",
            "--novendor",
            scratch.to_string_lossy().as_ref(),
        ],
    );
    let _ = fs::remove_file(&scratch);

    log(
        silent,
        "installed the .desktop entry and markdown MIME globs (XDG, user-mode)",
    );
    Ok(())
}

fn unbind_from(base: &Path, silent: bool) -> io::Result<()> {
    // `xdg-mime uninstall` identifies the installed copy by the basename of
    // its argument (see /usr/bin/xdg-mime), so a freshly-written scratch
    // file with the same name works as well as the original would — the
    // content just needs to exist and parse, it does not need to be the
    // literal bytes `install` used.
    let scratch = mime_package_scratch_path();
    fs::write(&scratch, mime_package_xml())?;
    run_best_effort(
        silent,
        base,
        "xdg-mime",
        &[
            "uninstall",
            "--mode",
            "user",
            scratch.to_string_lossy().as_ref(),
        ],
    );
    let _ = fs::remove_file(&scratch);

    let applications = base.join("applications");
    let _ = fs::remove_file(applications.join(DESKTOP_FILE_NAME));
    run_best_effort(
        silent,
        base,
        "update-desktop-database",
        &[applications.to_string_lossy().as_ref()],
    );

    log(
        silent,
        "removed the .desktop entry and markdown MIME globs (XDG, user-mode)",
    );
    Ok(())
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn data_home() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share")
}

fn current_exe_string() -> io::Result<String> {
    Ok(std::env::current_exe()?.to_string_lossy().into_owned())
}

fn mime_package_scratch_path() -> PathBuf {
    std::env::temp_dir().join(MIME_PACKAGE_NAME)
}

fn desktop_entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Marklet\n\
         Comment=Lightweight Markdown viewer\n\
         Exec=\"{exe}\" %f\n\
         Terminal=false\n\
         MimeType={MIME_TYPE};\n\
         Categories=Utility;\n"
    )
}

fn mime_package_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n\
         \x20 <mime-type type=\"{MIME_TYPE}\">\n\
         \x20   <comment>Markdown document</comment>\n\
         \x20   <glob pattern=\"*.md\"/>\n\
         \x20   <glob pattern=\"*.markdown\"/>\n\
         \x20   <glob pattern=\"*.mdown\"/>\n\
         \x20   <glob pattern=\"*.mkd\"/>\n\
         \x20 </mime-type>\n\
         </mime-info>\n"
    )
}

/// Runs `program` with `XDG_DATA_HOME` pinned to `xdg_data_home`, so it
/// never touches paths outside the base this call was asked to operate on
/// (the real home dir in production, a temp dir in tests). Logs and
/// continues on any failure — a missing binary or a nonzero exit must not
/// fail `--install`/`--unbind`, since both refreshes are best-effort caches.
fn run_best_effort(silent: bool, xdg_data_home: &Path, program: &str, args: &[&str]) {
    let result = Command::new(program)
        .args(args)
        .env("XDG_DATA_HOME", xdg_data_home)
        .output();
    match result {
        Ok(output) if !output.status.success() => {
            log(
                silent,
                &format!(
                    "{program} exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            );
        }
        Ok(_) => {}
        Err(e) => {
            log(
                silent,
                &format!("{program} not available ({e}) — skipping, best effort only"),
            );
        }
    }
}

fn log(silent: bool, message: &str) {
    if !silent {
        eprintln!("marklet: {message}");
    }
}

// ---------------------------------------------------------------------
// Tests — run for real on this machine (Linux/WSL2), against an isolated
// temp XDG base so they never touch the developer's real
// `~/.local/share`.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// One round trip, not two assertions: install into a throwaway XDG
    /// base, prove the `.desktop` file and the MIME package landed, unbind,
    /// prove both are gone. Exercises the real `xdg-mime` binary — the
    /// isolated `XDG_DATA_HOME` means it can never touch the real user MIME
    /// database, so this is safe to run on a developer machine.
    #[test]
    fn install_and_unbind_round_trip() {
        let base = std::env::temp_dir().join(format!(
            "marklet-test-xdg-{}-{}",
            std::process::id(),
            "install-unbind"
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create temp XDG base");

        install_to(&base, true).expect("install_to");
        assert!(
            base.join("applications").join(DESKTOP_FILE_NAME).exists(),
            ".desktop file missing after install"
        );
        assert!(
            base.join("mime")
                .join("packages")
                .join(MIME_PACKAGE_NAME)
                .exists(),
            "MIME package missing after install"
        );

        unbind_from(&base, true).expect("unbind_from");
        assert!(
            !base.join("applications").join(DESKTOP_FILE_NAME).exists(),
            ".desktop file survived unbind"
        );
        assert!(
            !base
                .join("mime")
                .join("packages")
                .join(MIME_PACKAGE_NAME)
                .exists(),
            "MIME package survived unbind"
        );

        let _ = fs::remove_dir_all(&base);
    }
}

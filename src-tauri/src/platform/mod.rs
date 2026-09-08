//! OS integration — Windows file association, context menu, and (later)
//! whatever Linux needs.
//!
//! **Stubs only.** Phase P6 implements the real HKCU registry work described
//! in `.claude/skills/win-integration/SKILL.md`: the exact key list, the
//! `--install`/`--unbind` round trip, `SHChangeNotify` after every change.
//! Do not write registry code here yet, and do not add `windows.rs` /
//! `linux.rs` before that phase — the key list belongs in exactly one file
//! once it exists, per the skill's rule 6, and an early split with nothing
//! real in it just invites the two copies it warns about.
//!
//! Every function here currently just logs what it would do and returns
//! `Ok(())`, so `cli.rs` and the NSIS hooks (`--install --silent` /
//! `--uninstall --silent`) already have a stable contract to build against.

use std::io;

/// Would write the `Marklet.Document` ProgID and the `OpenWithProgids` keys
/// for `.md`/`.markdown`/`.mdown`/`.mkd`, all under HKCU, then call
/// `SHChangeNotify(SHCNE_ASSOCCHANGED, ...)`. See the key list in
/// `.claude/skills/win-integration/SKILL.md`.
pub fn install(silent: bool) -> io::Result<()> {
    log(
        silent,
        "--install is not implemented yet (phase P6) — would write the HKCU \
         file-association keys and notify Explorer",
    );
    Ok(())
}

/// Would do everything `unbind` does, plus remove the
/// `SystemFileAssociations\\.md\\shell\\marklet` context-menu verb.
pub fn uninstall(silent: bool) -> io::Result<()> {
    log(
        silent,
        "--uninstall is not implemented yet (phase P6) — would remove the \
         context-menu verb and every key --install wrote",
    );
    Ok(())
}

/// Would remove every key `install` wrote, and every parent key left empty —
/// `reg query HKCU\\Software\\Classes /f marklet /s` must return nothing
/// afterwards, per the skill's round-trip contract.
pub fn unbind(silent: bool) -> io::Result<()> {
    log(
        silent,
        "--unbind is not implemented yet (phase P6) — would remove the HKCU \
         file-association keys and notify Explorer",
    );
    Ok(())
}

fn log(silent: bool, message: &str) {
    if !silent {
        eprintln!("marklet: {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_currently_succeed() {
        assert!(install(true).is_ok());
        assert!(uninstall(true).is_ok());
        assert!(unbind(true).is_ok());
    }
}

//! Windows file association and Explorer integration.
//!
//! **This file is the single source of truth for every registry key Marklet
//! writes.** Nothing else — not `installer/hooks.nsh`, not any other
//! module — may hardcode a key path. `.claude/skills/win-integration/SKILL.md`
//! documents the same list; keep them in sync if either changes.
//!
//! Everything lives under `HKCU\Software\Classes`. No key is ever written
//! under `HKEY_LOCAL_MACHINE` — that would need elevation, and would strand
//! machine-wide state a per-user uninstall cannot clean up.
//!
//! ```text
//! HKCU\Software\Classes\Marklet.Document
//! HKCU\Software\Classes\Marklet.Document\DefaultIcon               = "<exe>",0
//! HKCU\Software\Classes\Marklet.Document\shell\open\command        = "<exe>" "%1"
//!
//! HKCU\Software\Classes\.md\OpenWithProgids\Marklet.Document       (empty REG_SZ)
//! HKCU\Software\Classes\.markdown\OpenWithProgids\Marklet.Document
//! HKCU\Software\Classes\.mdown\OpenWithProgids\Marklet.Document
//! HKCU\Software\Classes\.mkd\OpenWithProgids\Marklet.Document
//!
//! HKCU\Software\Classes\SystemFileAssociations\.md\shell\marklet           = "Open with Marklet"
//! HKCU\Software\Classes\SystemFileAssociations\.md\shell\marklet\command   = "<exe>" "%1"
//!
//! HKCU\Software\Classes\Directory\shell\marklet_vault                     = "Open folder as Vault"
//! HKCU\Software\Classes\Directory\shell\marklet_vault\command             = "<exe>" "%1"
//! HKCU\Software\Classes\Directory\Background\shell\marklet_vault          = "Open folder as Vault"
//! HKCU\Software\Classes\Directory\Background\shell\marklet_vault\command  = "<exe>" "%V"
//! ```
//!
//! The `(Default)` values on the two verb keys and the two `\command` keys
//! under `SystemFileAssociations`/`Directory` are not spelled out with `=`
//! in the skill's ASCII list, but they are required for the verb to actually
//! do anything (an empty `\command` value invokes nothing) and for the
//! context-menu entry to show a real label instead of the literal key name
//! ("marklet" / "marklet_vault"). They live at the same key paths the skill
//! lists, so they are not a new key — just the value that key needs to work.
//!
//! `OpenWithProgids` rather than overwriting `.md`'s default handler: it
//! adds Marklet to the "Open with" list without silently stealing an
//! association the user already chose. Making Marklet the default is the
//! user's decision, taken in Windows Settings — never a side effect of
//! installing.
//!
//! The legacy `SystemFileAssociations\.md\shell\marklet` verb appears under
//! Explorer's "Show more options" on Windows 11, not the top-level menu — the
//! top-level menu needs a packaged `IExplorerCommand` COM extension with
//! MSIX identity, deferred to v2 and documented as a known limitation in
//! `README.md`.
//!
//! ## `--install` / `--unbind` / `--uninstall`
//!
//! `--install` writes every key above and is idempotent — every operation
//! here is "create or open" / "set", never "create only".
//!
//! `--unbind` removes every key `install` wrote, including the two
//! context-menu verb trees, and every parent left empty by that removal
//! (`SystemFileAssociations\.md\shell` itself, `.md\OpenWithProgids` itself,
//! …) — `README.md` documents `--unbind` as already "leaving no residual
//! registry keys", and the round-trip CI test (`--install` then `--unbind`
//! alone, nothing else) checks exactly that: `reg query
//! HKCU\Software\Classes /f marklet /s` must return nothing afterwards.
//! Since `--unbind` already removes the context-menu verbs, `--uninstall`
//! has nothing left to add and just calls it — see its doc comment.
//!
//! `SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None)` runs after
//! every install and every unbind. Without it Explorer keeps showing the old
//! icon/menu until it restarts.

use std::io;

use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows_registry::{Key, CURRENT_USER};

/// The ProgID Marklet registers for every extension below.
const PROG_ID: &str = "Marklet.Document";

/// Extensions added to the "Open with" list via `OpenWithProgids`.
const EXTENSIONS: &[&str] = &[".md", ".markdown", ".mdown", ".mkd"];

/// Legacy per-extension context-menu verb (`SystemFileAssociations\.md\shell\<verb>`).
const ASSOC_VERB: &str = "marklet";
const ASSOC_VERB_LABEL: &str = "Open with Marklet";

/// "Open folder as vault" verb, registered under both `Directory\shell` and
/// `Directory\Background\shell`. `%1` is the selected folder; `%V` is the
/// folder being viewed, which `Directory\Background\shell` verbs must use
/// since there is no selected item there.
const VAULT_VERB: &str = "marklet_vault";
const VAULT_VERB_LABEL: &str = "Open folder as Vault";
const VAULT_BASES: &[(&str, &str)] = &[
    ("Directory\\shell", "%1"),
    ("Directory\\Background\\shell", "%V"),
];

const CLASSES_ROOT: &str = "Software\\Classes";

/// Writes every key in the module doc's list. Idempotent.
pub fn install(silent: bool) -> io::Result<()> {
    let exe = current_exe_string()?;
    let classes = open_classes()?;

    let progid = classes.create(PROG_ID).map_err(reg_err)?;
    progid
        .create("DefaultIcon")
        .map_err(reg_err)?
        .set_string("", format!("\"{exe}\",0"))
        .map_err(reg_err)?;
    progid
        .create("shell\\open\\command")
        .map_err(reg_err)?
        .set_string("", open_command(&exe))
        .map_err(reg_err)?;

    for ext in EXTENSIONS {
        classes
            .create(format!("{ext}\\OpenWithProgids"))
            .map_err(reg_err)?
            .set_string(PROG_ID, "")
            .map_err(reg_err)?;
    }

    let verb = classes
        .create(format!("SystemFileAssociations\\.md\\shell\\{ASSOC_VERB}"))
        .map_err(reg_err)?;
    verb.set_string("", ASSOC_VERB_LABEL).map_err(reg_err)?;
    verb.create("command")
        .map_err(reg_err)?
        .set_string("", open_command(&exe))
        .map_err(reg_err)?;

    for (base, arg) in VAULT_BASES {
        let verb = classes
            .create(format!("{base}\\{VAULT_VERB}"))
            .map_err(reg_err)?;
        verb.set_string("", VAULT_VERB_LABEL).map_err(reg_err)?;
        verb.create("command")
            .map_err(reg_err)?
            .set_string("", format!("\"{exe}\" \"{arg}\""))
            .map_err(reg_err)?;
    }

    notify_shell();
    log(
        silent,
        "registered the .md/.markdown/.mdown/.mkd file association and context-menu verbs (HKCU)",
    );
    Ok(())
}

/// Identical to [`unbind`]. `unbind` already removes the legacy
/// `SystemFileAssociations` verb and the `Directory` vault verbs — "every
/// key `install` wrote" — so there is nothing left for `--uninstall` to
/// additionally remove; the round-trip CI test proves `--unbind` alone
/// already reaches zero residual keys. Kept as a distinct entry point
/// (rather than a `cli.rs`-level alias) so the NSIS pre-uninstall hook and
/// the CLI surface can diverge later without a breaking rename.
pub fn uninstall(silent: bool) -> io::Result<()> {
    unbind(silent)
}

/// Removes every key `install` wrote, and every parent key left empty by
/// that removal. `reg query HKCU\Software\Classes /f marklet /s` must
/// return nothing afterwards.
pub fn unbind(silent: bool) -> io::Result<()> {
    let classes = open_classes()?;

    remove_tree_if_exists(&classes, PROG_ID)?;

    for ext in EXTENSIONS {
        remove_value_and_clean(&classes, &format!("{ext}\\OpenWithProgids"), PROG_ID)?;
    }

    remove_key_and_empty_ancestors(
        &classes,
        &format!("SystemFileAssociations\\.md\\shell\\{ASSOC_VERB}"),
    )?;

    for (base, _) in VAULT_BASES {
        remove_key_and_empty_ancestors(&classes, &format!("{base}\\{VAULT_VERB}"))?;
    }

    notify_shell();
    log(
        silent,
        "removed the .md file association and context-menu verbs (HKCU) — zero residual keys",
    );
    Ok(())
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn open_classes() -> io::Result<Key> {
    CURRENT_USER.create(CLASSES_ROOT).map_err(reg_err)
}

fn current_exe_string() -> io::Result<String> {
    Ok(std::env::current_exe()?.to_string_lossy().into_owned())
}

fn open_command(exe: &str) -> String {
    format!("\"{exe}\" \"%1\"")
}

/// Turns any registry failure into an `io::Error`.
///
/// Generic over `Display` rather than naming `windows_registry::Error`, which
/// is not nameable: the crate glob-re-exports it from `windows_result` and does
/// not re-export the path, so `windows_registry::Error` is private and only
/// fails to compile on Windows — where nobody developing on Linux would see it.
fn reg_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

fn notify_shell() {
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}

fn log(silent: bool, message: &str) {
    if !silent {
        eprintln!("marklet: {message}");
    }
}

/// `true` if `key` has zero subkeys and zero values (including no `(Default)`
/// value set).
fn is_empty(key: &Key) -> io::Result<bool> {
    let no_subkeys = key.keys().map_err(reg_err)?.next().is_none();
    let no_values = key.values().map_err(reg_err)?.next().is_none();
    Ok(no_subkeys && no_values)
}

/// Deletes `path` (relative to `parent`) if it exists. A no-op — not an
/// error — if it does not, so unbind stays idempotent even if called twice
/// or without a prior install.
fn remove_tree_if_exists(parent: &Key, path: &str) -> io::Result<()> {
    if parent.open(path).is_err() {
        return Ok(());
    }
    parent.remove_tree(path).map_err(reg_err)
}

/// Deletes `full_path` (a key relative to `classes`, i.e. rooted at
/// `HKCU\Software\Classes`), then walks upward deleting each ancestor that
/// is left with zero subkeys and zero values. Stops the moment an ancestor
/// is non-empty (e.g. another app also uses `Directory\shell`, or `.md`
/// carries an OS-set `Content Type` value), or when it reaches `classes`
/// itself — `Software\Classes` is never a key Marklet wrote and is never a
/// removal candidate.
fn remove_key_and_empty_ancestors(classes: &Key, full_path: &str) -> io::Result<()> {
    remove_tree_if_exists(classes, full_path)?;

    let mut path = full_path;
    while let Some((parent, _)) = path.rsplit_once('\\') {
        let parent_key = match classes.open(parent) {
            Ok(k) => k,
            Err(_) => return Ok(()),
        };
        let empty = is_empty(&parent_key)?;
        drop(parent_key);
        if !empty {
            break;
        }
        remove_tree_if_exists(classes, parent)?;
        path = parent;
    }
    Ok(())
}

/// Removes the `value_name` value from `key_path` (relative to `classes`) if
/// present, then — if that leaves `key_path` itself empty — removes it and
/// walks the same empty-ancestor cleanup as [`remove_key_and_empty_ancestors`].
/// Used for `.md\OpenWithProgids`, where `Marklet.Document` is a *value*
/// under the key, not a subkey.
/// Removes one value from a shared key, then the key itself only if nothing
/// else remains in it.
///
/// Two details here were bugs, and both were invisible for the same reason.
///
/// `Key::open` opens **read-only** — `remove_value` on that handle fails with
/// access denied, and the original `let _ = key.remove_value(..)` threw the
/// error away, so `unbind` reported success while removing nothing. The value
/// survived every uninstall. `options().read().write().open()` asks for the
/// access this function actually needs; plain `create()` would be wrong, since
/// it resurrects a key we may be about to delete.
///
/// The error is no longer discarded. `remove_value` fails both when the value
/// is absent and when it cannot be removed, and those are opposite outcomes —
/// so absence is established first, by reading, and only a genuine removal
/// failure propagates.
fn remove_value_and_clean(classes: &Key, key_path: &str, value_name: &str) -> io::Result<()> {
    let key = match classes.options().read().write().open(key_path) {
        Ok(k) => k,
        // The key does not exist, so neither does our value in it.
        Err(_) => return Ok(()),
    };

    if key.get_string(value_name).is_ok() {
        key.remove_value(value_name).map_err(reg_err)?;
    }

    let empty = is_empty(&key)?;
    drop(key);
    if empty {
        remove_key_and_empty_ancestors(classes, key_path)
    } else {
        // Someone else is still registered for this extension. Removing the
        // key here would un-register every other application that opens .md.
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Tests — windows-latest only. There is no registry under WSL2; a mock
// would only test the mock. See `.claude/skills/win-integration/SKILL.md`.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// One round trip, not two assertions: install, prove the keys exist,
    /// unbind, prove every one of them — and their once-empty parents — is
    /// gone. Mirrors the PowerShell step in `.github/workflows/code-test.yml`,
    /// but exercises the Rust logic directly instead of the packaged binary.
    #[test]
    fn install_and_unbind_round_trip() {
        install(true).expect("install");

        let classes = open_classes().expect("open Software\\Classes");
        assert!(
            classes.open(PROG_ID).is_ok(),
            "ProgID key missing after install"
        );
        let open_with = classes
            .open(".md\\OpenWithProgids")
            .expect("OpenWithProgids key missing after install");
        assert!(
            open_with
                .values()
                .expect("enumerate OpenWithProgids values")
                .any(|(name, _)| name == PROG_ID),
            "Marklet.Document not listed in .md\\OpenWithProgids"
        );
        assert!(classes
            .open(format!(
                "SystemFileAssociations\\.md\\shell\\{ASSOC_VERB}\\command"
            ))
            .is_ok());
        for (base, _) in VAULT_BASES {
            assert!(classes
                .open(format!("{base}\\{VAULT_VERB}\\command"))
                .is_ok());
        }

        unbind(true).expect("unbind");

        assert!(classes.open(PROG_ID).is_err(), "ProgID key survived unbind");

        // `OpenWithProgids` is a SHARED key: every application that can open a
        // `.md` lists its own ProgID there. What unbind must remove is our
        // value; what it must NOT remove is the key itself when someone else is
        // still listed in it. Deleting a shared key would silently un-register
        // every other editor on the machine — the same class of harm the
        // `OpenWithProgids`-rather-than-default-handler decision exists to
        // avoid, arriving through cleanup instead of through install.
        //
        // The first version of this test asserted the key was gone. It passed
        // nowhere and would have been "fixed" by making unbind destructive.
        for ext in EXTENSIONS {
            match classes.open(format!("{ext}\\OpenWithProgids")) {
                // Key gone: correct, and only possible when we were the only
                // application listed.
                Err(_) => {}
                // Key survives: correct only if OUR value is gone from it.
                Ok(key) => assert!(
                    key.get_string(PROG_ID).is_err(),
                    "{ext}\\OpenWithProgids still lists {PROG_ID} after unbind"
                ),
            }
        }
        assert!(
            classes
                .open(format!("SystemFileAssociations\\.md\\shell\\{ASSOC_VERB}"))
                .is_err(),
            "legacy context-menu verb survived unbind"
        );
        assert!(
            classes.open("SystemFileAssociations\\.md\\shell").is_err(),
            "SystemFileAssociations\\.md\\shell left stranded and empty"
        );
        for (base, _) in VAULT_BASES {
            assert!(
                classes.open(format!("{base}\\{VAULT_VERB}")).is_err(),
                "vault verb under {base} survived unbind"
            );
        }
    }
}

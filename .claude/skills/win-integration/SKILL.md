---
name: win-integration
description: The exact HKCU registry keys Marklet writes for .md file association and the Explorer context menu, how --install and --unbind reverse each other, the NSIS hook contract, and the console-attach requirement. Read before touching src-tauri/src/platform/**, installer/hooks.nsh, or the bundle block of tauri.conf.json.
---

# Windows integration

Two goals, equally important: double-clicking a `.md` file opens Marklet, and uninstalling
Marklet leaves **zero** trace in the registry.

## Everything is HKCU

No key is ever written under `HKEY_LOCAL_MACHINE`. That means no UAC elevation, per-user
installation, and a clean uninstall that cannot strand machine-wide state. The NSIS installer
runs in `currentUser` mode for the same reason.

## The key list — one source of truth

`src-tauri/src/platform/windows.rs` owns the list. Nothing else may hardcode a key path,
including the NSIS script.

```
HKCU\Software\Classes\Marklet.Document
HKCU\Software\Classes\Marklet.Document\DefaultIcon               = "<exe>",0
HKCU\Software\Classes\Marklet.Document\shell\open\command        = "<exe>" "%1"

HKCU\Software\Classes\.md\OpenWithProgids\Marklet.Document       (empty REG_SZ)
HKCU\Software\Classes\.markdown\OpenWithProgids\Marklet.Document
HKCU\Software\Classes\.mdown\OpenWithProgids\Marklet.Document
HKCU\Software\Classes\.mkd\OpenWithProgids\Marklet.Document

HKCU\Software\Classes\SystemFileAssociations\.md\shell\marklet
HKCU\Software\Classes\SystemFileAssociations\.md\shell\marklet\command

HKCU\Software\Classes\Directory\shell\marklet_vault
HKCU\Software\Classes\Directory\shell\marklet_vault\command
HKCU\Software\Classes\Directory\Background\shell\marklet_vault
HKCU\Software\Classes\Directory\Background\shell\marklet_vault\command
```

`OpenWithProgids` rather than overwriting `.md`'s default handler: it adds Marklet to the
"Open with" list without silently stealing an association the user chose. Making Marklet the
default is the user's decision, taken in the Windows settings dialog or in our own settings
panel — never a side effect of installing.

### Why `bundle.fileAssociations` is absent from `tauri.conf.json`

**Do not add it back.** It looks like the obvious place to declare `.md`, and it quietly does
the opposite of what this file says.

Whenever that block is present, Tauri's NSIS bundler inserts its own `APP_ASSOCIATE` macro
into the generated installer, *in addition to* `installerHooks` rather than instead of it —
`installer.nsi` calls it at line 666, our `NSIS_HOOK_POSTINSTALL` runs at line 733. The macro
body is unambiguous (`FileAssociation.nsh:73`):

```nsis
WriteRegStr SHELL_CONTEXT "Software\Classes\.${EXT}" "" "${FILECLASS}"
```

An empty value name is the key's **default** value, so that line sets the extension's default
handler outright. Every Windows install would have made Marklet the default `.md` opener
without asking — the exact behaviour the paragraph above forbids, arriving through a config
field rather than through any code in this repository.

`hooks.nsh` shelling out to `marklet.exe --install --silent` is therefore the *only* Windows
association mechanism, which is what that file already claimed to be. Verified against
`tauri-apps/tauri@dev`, not inferred.

After every write and every removal:

```rust
SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
```

Without it Explorer keeps showing the old icon and the old menu until it restarts, and the
change looks broken.

## `--install` and `--unbind` are exact inverses

Both are implemented in `windows.rs` over the same key list.

- `--install` writes every key. Idempotent: running it twice changes nothing.
- `--unbind` removes every key **and every parent key that is now empty**. Leaving an empty
  `SystemFileAssociations\.md\shell` behind is a failure, not a cosmetic detail.
- `--uninstall` is `--unbind` plus removing the Explorer context-menu verbs.

There is a PowerShell test in CI:

```powershell
marklet.exe --install
# assert HKCU:\Software\Classes\.md\OpenWithProgids\Marklet.Document exists
marklet.exe --unbind
reg query HKCU\Software\Classes /f marklet /s   # must return nothing
```

## The NSIS hook calls the binary

`src-tauri/installer/hooks.nsh` must **not** duplicate the key list. It shells out:

```nsis
!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\marklet.exe" --install --silent'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\marklet.exe" --uninstall --silent'
!macroend
```

Two copies of a registry key list drift. One always gets updated and the other does not, and
the symptom is orphaned keys that nobody notices for months.

## Context menu: legacy verb, and why

Windows 11's top-level context menu requires a packaged `IExplorerCommand` COM extension with
MSIX identity — a disproportionate amount of work and a signing requirement. v1 ships the
legacy `SystemFileAssociations` verb, which appears under **"Show more options"**.

This is stated plainly in the README. Do not present it as a bug; it is a documented v1
limitation with a v2 issue attached.

## Single instance

`tauri-plugin-single-instance` is not a nicety. A second WebView2 instance costs roughly
40 MB RSS, and double-clicking three files in Explorer would otherwise mean three of them.
The second launch forwards its path to the running window.

## The console that does not exist

The binary is built with `#![windows_subsystem = "windows"]`, so it has no console and
`--help` prints into the void.

Before any CLI-mode output:

```rust
AttachConsole(ATTACH_PARENT_PROCESS)  // reopen stdout/stderr handles on success
```

If it fails — the process was launched from Explorer, not a terminal — fall back to a message
box for `--help` and `--version`.

Then exit **before** `tauri::Builder::build()`. `--install`, `--unbind`, `--help` and
`MD_HTML=1` must complete in under 50 ms with no window and no WebView2 initialization. That
is the entire reason a GUI binary has a CLI at all.

## Testing

Registry, file association and `PrintToPdf` are tested **only** on `windows-latest`, behind
`#[cfg(windows)]`. Do not attempt to test them from WSL2 — there is no registry there, and a
mock would only test the mock.

# What Marklet writes to your Windows registry

Everything below is under `HKEY_CURRENT_USER`. Nothing is written to `HKEY_LOCAL_MACHINE`,
nothing asks for administrator rights, and nothing is written by the installer that the
application cannot remove by itself.

## The keys

`marklet --install` — and the NSIS installer, which calls the same code rather than
duplicating the list — writes exactly these, all under `HKCU\Software\Classes`:

| Key | What it is |
|---|---|
| `Marklet.Document` | The ProgID. Carries `DefaultIcon` and `shell\open\command`. |
| `.md\OpenWithProgids`, and the same for `.markdown`, `.mdown`, `.mkd` | One **value** named `Marklet.Document` in each. This is what puts Marklet in the "Open with" list. |
| `SystemFileAssociations\.md\shell\marklet` | The "Open with Marklet" context-menu verb, plus its `command` subkey. |
| `Directory\shell\marklet_vault` | "Open folder as Vault" on a selected folder. Passes `%1`. |
| `Directory\Background\shell\marklet_vault` | The same verb on the folder you are *inside*. Passes `%V`, because a background click has no selected item. |

`SHChangeNotify(SHCNE_ASSOCCHANGED, ...)` is called afterwards, so Explorer notices without
a sign-out.

## What it does not do

**It does not take the association.** Writing `OpenWithProgids` adds Marklet to the "Open
with" list; it does not set the default handler. Windows will not let an application set its
own default anyway — that is the user's choice in Settings, and deliberately so.

**It does not appear at the top of the Windows 11 context menu.** The verbs above are the
legacy kind, so they live under **"Show more options"**. The modern menu requires a packaged
`IExplorerCommand` COM extension with MSIX identity — a signing and packaging requirement,
not a size one. Deferred; see [`editions.md`](editions.md).

## Removing it

```
marklet --unbind
```

This removes every key in the table **and every parent key the removal leaves empty**, so
nothing is left behind. `--uninstall` is the same operation under a second name, kept so the
NSIS pre-uninstall hook and the CLI can diverge later without a rename.

One exception is deliberate and worth knowing about: `.md\OpenWithProgids` is a **shared**
key. Every application that can open a `.md` file registers a value there. `--unbind`
removes *its own value* and removes the key only if it is then empty. An earlier version of
the test for this asserted the key must disappear, which would have made an uninstall of
Marklet quietly remove every other editor from the "Open with" list.

The check CI runs after an install/unbind round trip on `windows-latest` is:

```
reg query HKCU\Software\Classes /f marklet /s
```

It must report no matches. If you want to confirm it on your own machine, run it before
installing, after installing, and after `--unbind`.

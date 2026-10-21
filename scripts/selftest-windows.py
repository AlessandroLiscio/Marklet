"""Drives the real Windows build of Marklet through WebView2's debug port.

Run by `selftest-windows.sh` with the Windows Python that has Playwright; not
meant to be called by hand. One scenario per call: `selftest-windows.py <scenario> <marklet.exe> [fixture]`.

Exit 0 and one `OK ...` line when the scenario holds, exit 1 and one `FAIL ...` line otherwise.
The native file dialogs (New file..., Open file...) cannot be driven this way and are not covered.
"""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

from playwright.sync_api import TimeoutError as PwTimeout
from playwright.sync_api import sync_playwright

PORT = 9333
WAIT_MS = 20_000


def launch(exe, arg=None):
    """Starts the app with a debug port and a throwaway WebView2 profile."""
    data = tempfile.mkdtemp(prefix="marklet-selftest-data-")
    env = dict(
        os.environ,
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={PORT}",
        WEBVIEW2_USER_DATA_FOLDER=data,
    )
    return subprocess.Popen([exe] + ([arg] if arg else []), env=env), data


def stop(proc, data):
    # Only the process this script started, never a Marklet the user has open.
    subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"], capture_output=True)
    time.sleep(1)
    shutil.rmtree(data, ignore_errors=True)


def attach(pw):
    deadline = time.time() + WAIT_MS / 1000
    while time.time() < deadline:
        try:
            browser = pw.chromium.connect_over_cdp(f"http://127.0.0.1:{PORT}")
            for ctx in browser.contexts:
                for page in ctx.pages:
                    if "tauri.localhost" in page.url:
                        return page
        except Exception:
            pass
        time.sleep(0.5)
    raise RuntimeError("the app never exposed a page on the debug port")


def start_screen(page, _):
    page.get_by_role("button", name="New file").wait_for(timeout=WAIT_MS)
    return "start screen offers New file"


def new_file_in_explorer(page, vault):
    """Right-click empty space, create a note, expect split view; then a taken name is refused."""
    scroller = page.locator(".scroller")
    scroller.wait_for(timeout=WAIT_MS)

    def ask(name):
        scroller.click(button="right", position={"x": 20, "y": 200})
        page.locator('.popup [role="menuitem"]').click()
        field = page.locator(".popup input")
        field.fill(name)
        field.press("Enter")

    ask("selftest")
    created = pathlib.Path(vault) / "selftest.md"
    # A selector, not wait_for_function: the app's CSP forbids evaluating strings.
    page.locator("body.marklet-mode-split").wait_for(state="attached", timeout=WAIT_MS)
    if not created.is_file() or created.stat().st_size != 0:
        raise RuntimeError("selftest.md was not created empty")

    # The toolbar is there in an editing mode, and a button edits the buffer.
    page.locator('[role="toolbar"] [data-kind="table"]').click()
    page.locator(".cm-content", has_text="| Column | Column | Column |").wait_for(timeout=WAIT_MS)

    ask("selftest")
    problem = page.locator(".popup .problem")
    problem.wait_for(timeout=WAIT_MS)
    if "already exists" not in problem.inner_text():
        raise RuntimeError(f"unexpected refusal: {problem.inner_text()!r}")
    return "explorer creates a note in split view, the toolbar edits it, and a taken name is refused"


def wsl_symlink(page, marker):
    """A file reached through a symlink with an absolute Linux target opens."""
    if not marker:
        raise RuntimeError("SELFTEST_MARKER is not set")
    page.locator("#doc").wait_for(timeout=WAIT_MS)
    page.locator("#doc", has_text=marker).wait_for(timeout=WAIT_MS)
    if page.locator(".notice.bad").count():
        raise RuntimeError(page.locator(".notice.bad").inner_text())
    return "WSL symlink opens"


def wait_for_real_symlink(path):
    """Windows must fail to read the link itself, or the scenario proves nothing.

    A symlink created seconds ago can be served from the 9P attribute cache as a
    plain file for a while, in which case the app opens it without any fix. The
    check passes only once Windows sees what a user's old symlink looks like.
    """
    deadline = time.time() + 120
    while time.time() < deadline:
        try:
            open(path, "rb").close()
        except OSError:
            return
        time.sleep(5)
    raise RuntimeError("the fixture symlink is still readable from Windows; it would not test the fix")


def vault_fixture():
    vault = tempfile.mkdtemp(prefix="marklet-selftest-vault-")
    (pathlib.Path(vault) / "existing.md").write_text("# existing\n", encoding="utf-8")
    return vault


# name -> (check, makes its own fixture folder to open)
SCENARIOS = {
    "start": (start_screen, False),
    "explorer-new-file": (new_file_in_explorer, True),
    "wsl-symlink": (wsl_symlink, False),
}


def main():
    name, exe = sys.argv[1], sys.argv[2]
    check, own_vault = SCENARIOS[name]
    # `wsl-symlink` is given the UNC path to open and, in the environment, a marker the file contains.
    arg = vault_fixture() if own_vault else (sys.argv[3] if len(sys.argv) > 3 else None)
    if name == "wsl-symlink":
        try:
            wait_for_real_symlink(arg)
        except RuntimeError as e:
            print(f"FAIL {name}: {e}")
            sys.exit(1)
    proc, data = launch(exe, arg)
    try:
        with sync_playwright() as pw:
            page = attach(pw)
            ctx = arg if own_vault else os.environ.get("SELFTEST_MARKER")
            print(f"OK   {name}: {check(page, ctx)}")
    except (PwTimeout, RuntimeError) as e:
        print(f"FAIL {name}: {str(e).splitlines()[0]}")
        sys.exit(1)
    finally:
        stop(proc, data)
        if own_vault:
            shutil.rmtree(arg, ignore_errors=True)


if __name__ == "__main__":
    main()

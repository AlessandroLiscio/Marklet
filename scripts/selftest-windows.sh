#!/usr/bin/env bash
# Runs the real Windows build of Marklet from WSL and drives it through WebView2's
# debug port with Playwright, so a change can be checked before anyone is asked to
# install it. Quiet by contract: one line per stage, PASS or FAIL, logs on disk.
#
#   scripts/selftest-windows.sh --installer <setup.exe>
#   scripts/selftest-windows.sh --run <github-actions-run-id>   # downloads marklet-setup
#
# The installer is unpacked with 7-Zip, never installed: the user's Marklet, its
# file associations and its settings are not touched. Refuses to run while a
# Marklet is open (the app is single-instance, so ours would hand over and exit).
# Native file dialogs cannot be driven this way and are not covered.
set -uo pipefail

NAME=selftest-windows
ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
BASE="$ROOT/tmp/reports/$NAME"
RUN_REL="tmp/reports/$NAME/run_$(date +%Y%m%d_%H%M%S)"
RUN="$ROOT/$RUN_REL"
VERBOSE=0; INSTALLER=""; RUN_ID=""
while (( $# )); do
  case "$1" in
    --installer) INSTALLER=${2:-}; shift 2 ;;
    --run) RUN_ID=${2:-}; shift 2 ;;
    --verbose) VERBOSE=1; shift ;;
    *) echo "usage: $0 (--installer <setup.exe> | --run <run-id>) [--verbose]"; exit 2 ;;
  esac
done
if [[ -z "$INSTALLER" && -z "$RUN_ID" ]]; then
  echo "usage: $0 (--installer <setup.exe> | --run <run-id>) [--verbose]"; exit 2
fi

mkdir -p "$BASE"
ls -1d "$BASE"/run_* 2>/dev/null | sort | head -n -9 | while IFS= read -r d; do rm -rf -- "$d"; done
mkdir -p "$RUN" && ln -sfn "$RUN" "$BASE/latest"

WINUSER=$(cmd.exe /c "echo %USERNAME%" 2>/dev/null | tr -d '\r')
WINTEMP="/mnt/c/Users/$WINUSER/AppData/Local/Temp"
APP_DIR="$WINTEMP/marklet-selftest"
VENV="$WINTEMP/marklet-selftest-venv"
APP_EXE=$(wslpath -w "$APP_DIR/marklet.exe")
PY=$(wslpath -w "$VENV/Scripts/python.exe")
SCRIPT=$(wslpath -w "$ROOT/scripts/selftest-windows.py")
SEVEN="/mnt/c/Program Files/7-Zip/7z.exe"
FIXTURE="$HOME/.cache/marklet-selftest"
FAILED=0; TOTAL=0

stage() {   # stage <name> <command...>
  local name=$1; shift; local log="$RUN/$name.log"; TOTAL=$((TOTAL+1))
  if (( VERBOSE )); then "$@" 2>&1 | tee "$log"; local rc=${PIPESTATUS[0]}
  else "$@" >"$log" 2>&1; local rc=$?; fi
  if (( rc == 0 )); then echo "OK   $name"
  else
    FAILED=$((FAILED+1)); echo "FAIL $name → $RUN_REL/$name.log"
    { grep -m5 -iE 'error|fail|exception|traceback' "$log" || tail -n5 "$log"; } | cut -c1-200 | sed 's/^/     /'
  fi
}

precheck() {
  [[ -x "$SEVEN" ]] || { echo "error: 7-Zip not found at $SEVEN"; return 1; }
  if tasklist.exe | grep -qi '^marklet.exe'; then
    echo "error: a Marklet is already running; close it first (single-instance app)"; return 1
  fi
}

fetch() {
  [[ -z "$RUN_ID" ]] && return 0
  local dl="$RUN/download"; mkdir -p "$dl"
  gh run download "$RUN_ID" -n marklet-setup -D "$dl" || return 1
  INSTALLER=$(ls "$dl"/nsis/*.exe | head -n1)
}

unpack() {
  [[ -f "$INSTALLER" ]] || { echo "error: installer not found: $INSTALLER"; return 1; }
  mkdir -p "$APP_DIR"
  (cd /mnt/c && "$SEVEN" x -y "$(wslpath -w "$INSTALLER")" -o"$(wslpath -w "$APP_DIR")" >/dev/null) || return 1
  [[ -f "$APP_DIR/marklet.exe" ]] || { echo "error: no marklet.exe in the installer"; return 1; }
}

venv() {
  if [[ ! -f "$VENV/Scripts/python.exe" ]]; then
    (cd /mnt/c && python.exe -m venv "$(wslpath -w "$VENV")") || return 1
  fi
  (cd /mnt/c && "$VENV/Scripts/python.exe" -c "import playwright" 2>/dev/null) \
    || (cd /mnt/c && "$VENV/Scripts/python.exe" -m pip install -q playwright) || return 1
}

scenario() {   # scenario <name> [arg]
  (cd /mnt/c && "$VENV/Scripts/python.exe" "$SCRIPT" "$1" "$APP_EXE" ${2:+"$2"}) \
    && return 0
  return 1
}

wsl_fixture() {
  # A symlink whose target is an absolute Linux path: the shape Windows cannot follow.
  # Created once and left alone: a fresh symlink is served as a plain file by the
  # 9P cache for a while, and recreating it every run would restart that clock.
  mkdir -p "$FIXTURE" && printf '# selftest\n\nsymlink-marker-7f3a\n' >"$FIXTURE/target.md" || return 1
  [[ "$(readlink "$FIXTURE/link.md" 2>/dev/null)" == "$FIXTURE/target.md" ]] \
    || ln -sfn "$FIXTURE/target.md" "$FIXTURE/link.md"
}

stage precheck precheck
stage fetch fetch
stage unpack unpack
stage venv venv
if (( FAILED )); then echo "FAIL $FAILED/$TOTAL stages · logs: $RUN_REL"; exit 1; fi

export WSLENV="SELFTEST_MARKER${WSLENV:+:$WSLENV}"; export SELFTEST_MARKER=symlink-marker-7f3a
stage wsl-fixture wsl_fixture
stage start scenario start
stage explorer-new-file scenario explorer-new-file
# Not `wslpath -w .../link.md`: it resolves the symlink and hands back the target, which tests nothing.
stage wsl-symlink scenario wsl-symlink "$(wslpath -w "$FIXTURE")\\link.md"

if (( FAILED )); then echo "FAIL $FAILED/$TOTAL stages · logs: $RUN_REL"; exit 1; fi
echo PASS

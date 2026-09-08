#!/usr/bin/env bash
# Dev loop for Marklet.
#
# The two WEBKIT_* variables are not optional under WSL2. Without them WebKitGTK
# negotiates a DMABUF renderer that WSLg cannot back, and Tauri paints an empty
# white window with no error on stdout. Keep them here rather than in a shell
# profile so a fresh clone works on the first try.
#
# Windows rendering is the source of truth. WebKitGTK is for iteration speed
# only; font rasterization and print output differ from WebView2 by design.
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found. Install the Rust toolchain first:" >&2
  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
  exit 1
fi

export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1

exec npm run tauri dev -- "$@"

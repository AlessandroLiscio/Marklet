#!/usr/bin/env bash
# One-time developer bootstrap for Linux / WSL2.
#
# Run this once per machine. It needs sudo for the system packages; the Rust
# toolchain installs into $HOME and does not.
set -euo pipefail

echo "==> System packages (sudo required)"
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  build-essential \
  curl wget file \
  pkg-config \
  libgtk-3-dev \
  xz-utils

if ! command -v rustup >/dev/null 2>&1; then
  echo "==> Rust toolchain"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

rustup target add x86_64-unknown-linux-gnu
rustup component add clippy rustfmt

echo "==> Node dependencies"
npm ci || npm install

echo
echo "Done. Start the app with:  ./scripts/dev.sh"

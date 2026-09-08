#!/usr/bin/env bash
# Type-check the Windows-only code from Linux, without sudo and without CI.
#
# WHY THIS EXISTS
#
# Everything under `#[cfg(windows)]` — the registry writes, SHChangeNotify, and
# later PrintToPdf — does not compile on a Linux dev box. Until this script,
# the only way to find out whether it compiled was to push and wait roughly six
# minutes for `code-test / Tests (windows-latest)`. Two separate round trips
# were spent on errors a type-check would have caught in seconds, one of them a
# private re-export (`windows_registry::Error`) that is *only* wrong on Windows.
#
# `cargo check --target x86_64-pc-windows-gnu` on the real crate does not work
# here: `tauri-build` runs `tauri-winres`, which needs the mingw binutils
# (`x86_64-w64-mingw32-windres`) and therefore a package install. The Windows
# platform module has no crate-internal dependencies — only `std`, `windows`
# and `windows-registry` — so it can be checked in isolation, with no
# `tauri-build` in the picture at all.
#
# WHAT IT DOES NOT DO
#
# It type-checks. It does not link, does not run, and cannot tell you whether a
# registry write actually lands or whether Explorer notices. Only
# `windows-latest` answers that, and the PowerShell round-trip step in
# `code-test.yml` is what does the asking. Treat a pass here as "the obvious
# class of failure is gone", not as verification.
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

TARGET=x86_64-pc-windows-gnu
WORK="${TMPDIR:-/tmp}/marklet-wincheck"

if ! rustup target list --installed | grep -qx "$TARGET"; then
  echo "==> adding $TARGET"
  rustup target add "$TARGET"
fi

rm -rf "$WORK"
mkdir -p "$WORK/src"

# The dependency block is COPIED from the real manifest rather than restated.
# A checker pinned to different versions than the build it stands in for is
# worse than no checker: it reports green against code the real build rejects.
python3 - "$WORK" <<'PY'
import re, sys, pathlib

work = pathlib.Path(sys.argv[1])
manifest = pathlib.Path("src-tauri/Cargo.toml").read_text()

block = re.search(
    r"^\[target\.'cfg\(windows\)'\.dependencies\]\n(.*?)(?=^\[|\Z)",
    manifest, re.S | re.M,
)
if not block:
    sys.exit("could not find the cfg(windows) dependency block in src-tauri/Cargo.toml")

work.joinpath("Cargo.toml").write_text(
    "[package]\n"
    'name = "marklet-wincheck"\n'
    'version = "0.0.0"\n'
    'edition = "2021"\n\n'
    "[lib]\n"
    'path = "src/lib.rs"\n\n'
    "[dependencies]\n" + block.group(1).strip() + "\n"
)
print("dependencies taken verbatim from src-tauri/Cargo.toml")
PY

cp src-tauri/src/platform/mod.rs "$WORK/src/lib.rs"
cp src-tauri/src/platform/windows.rs "$WORK/src/"
cp src-tauri/src/platform/linux.rs "$WORK/src/"

echo "==> cargo check --target $TARGET (platform module only)"
cargo check --manifest-path "$WORK/Cargo.toml" --target "$TARGET" --all-targets

echo
echo "Windows platform module type-checks. Linking, registry behaviour and"
echo "Explorer integration are still only verified by code-test on windows-latest."

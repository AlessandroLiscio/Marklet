---
name: release
description: How a Marklet version is cut — semver policy, the tag-to-artifact pipeline, the full/lite/offline matrix, checksums and changelog. Read before tagging a release, changing the CI build matrix, or altering what the release workflow publishes.
---

# Release

## Versioning

Semver. The version lives in three files and they must agree:

- `package.json` → `version`
- `src-tauri/Cargo.toml` → `[package] version`
- `src-tauri/tauri.conf.json` → `version`

A mismatch produces an installer whose displayed version is not the one in the tag, and it is
only noticed by a user. `.github/workflows/release.yml` checks all three and fails the tag
if they disagree.

## What a tag produces

Pushing `v*` runs `release.yml`, which builds on `windows-latest` and `ubuntu-22.04`:

| Artifact | Build | Notes |
|---|---|---|
| `marklet-setup.exe` | default features | full, includes Mermaid |
| `marklet-lite-setup.exe` | `--no-default-features` | ~750 KB smaller, no Mermaid |
| `marklet-offline-setup.exe` | `webviewInstallMode: offlineInstaller` | ~127 MB, air-gapped machines only |
| `marklet-<ver>.AppImage` | default | Linux |
| `marklet-<ver>.deb` | default | Linux |
| `SHA256SUMS` | — | covers every artifact above |

The offline installer is built **only** on tags. It is 127 MB because it embeds the whole
WebView2 runtime; it must never become the default download.

## The size gate runs first

`size-gate.yml` fails the release if either primary installer is over budget:

| Artifact | Ceiling |
|---|---|
| `marklet-setup.exe` | 5\_033\_165 B |
| `marklet-lite-setup.exe` | 3\_984\_589 B |

A release is not cut over a failing size gate. Cut the feature or ship the previous version.
See `.claude/skills/size-budget/SKILL.md`.

## Cold-start gate

Five runs of `marklet --benchmark tests/fixtures/large.md` on `windows-latest`; the median
`boot-ms` must be under 1200 ms. This is the promise the whole project rests on, so it gates
the release rather than merely reporting.

## Changelog

`CHANGELOG.md`, Keep a Changelog format, newest first. Every entry says what a user can now
do, not which file changed. Size and cold-start deltas against the previous release go in
the entry — users of a "lightweight" tool care about that number, and publishing it keeps us
honest.

## Checklist

1. `CHANGELOG.md` updated, `Unreleased` promoted to the new version and dated.
2. Version bumped in all three files.
3. `cargo test`, `npm test`, `cargo clippy -- -D warnings` green on `main`.
4. `git tag vX.Y.Z && git push --tags`.
5. Wait for `release.yml`. Verify the artifact sizes in its summary against the previous
   release before publishing the GitHub release notes.
6. Download `marklet-setup.exe` on a real Windows 11 machine, install it, double-click a
   `.md` file, uninstall it, then confirm `reg query HKCU\Software\Classes /f marklet /s`
   returns nothing.

Step 6 is not skippable. CI cannot test the Explorer double-click path.

# Changelog

All notable changes to Marklet are recorded here, following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Every entry says what a user can now do, not which files changed. Size and cold-start deltas
against the previous release belong in the entry too — users of a tool that calls itself
lightweight care about that number, and publishing it keeps us honest.

## [Unreleased]

### Added

- Project foundation: Tauri 2 shell, Rust render core contract, Svelte 5 chrome, and the
  CI matrix that builds Windows and Linux artifacts from the first commit.
- Size gate enforcing 3.5 MiB for the full installer and 2.8 MiB for the lite build, with
  the byte delta commented on every pull request. The empty shell measures **904 KiB**, so
  the ceilings leave about 2.6 MiB for every feature still to land.
- Agent configuration under `.claude/` — five skills and four agents encoding the size
  budget, the render contract, the IPC boundary, Windows integration and the release flow.

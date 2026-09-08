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
- Agent configuration under `.claude/` — six skills and four agents encoding the size
  budget, the render contract, the IPC boundary, Windows integration, the release flow and
  the release-bookkeeping audit.
- CI split into four independently required checks — quality, security, tests, size gate —
  behind an orchestrator that runs the fast three on every branch push and the expensive two
  only where a merge depends on them.
- Security scanning: `cargo-deny` (RustSec advisories, licences, banned crates), Trivy
  across both lock files, TruffleHog over full history, plus a weekly schedule so advisories
  against unchanged dependencies are still caught.
- `npm run check:imports` fails the build if Mermaid, KaTeX, highlight.js or CodeMirror ever
  reach a static import — the budget invariant a reviewer cannot see.
- Branch protection as code in `.github/rulesets/protect-main.json`, release write-targets
  in `.github/RELEASE_TARGETS.yml`, and contributor governance: PR template, three issue
  forms, commit template, `CONTRIBUTING.md`, `SECURITY.md`.

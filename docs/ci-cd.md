# CI/CD, branch protection and distribution

Marklet has no container registry and no cluster. Distribution is GitHub Releases and
nothing else, which makes the pipeline simpler than it would otherwise be — and puts all the
weight on the tag.

## The trigger model

One CI run per unit of new code. The rule that keeps it from doubling: a same-repo pull
request's required checks are satisfied by the check-runs already produced by the branch
push, so opening or updating a PR schedules nothing extra.

| Event | What runs | Why |
|---|---|---|
| push to a non-main branch | `code-quality`, `code-security`, `code-test` | The authoritative check run. Fast — no Tauri build. |
| pull request | `build` + `size-gate`; **plus** the three above only if the head repo is a fork | The gate needs both installers, which branch pushes do not produce. Fork pushes never reach this repository, so the PR event is their only chance to produce the required checks. |
| push to main (a merge) | `build` | Keeps a downloadable artifact on main. The checks are not re-run: `strict_required_status_checks_policy` guarantees the merged tree is byte-identical to the tested branch tip. |
| tag `v*` | `release.yml` | The tag *is* the release. |
| Friday 18:00 | `code-security` | An advisory published on Tuesday affects a dependency that has not changed since March. Nothing else would re-scan it. |

A full Tauri build is roughly six minutes across three matrix legs. Running it on every
intermediate commit would make the fast loop useless, so the budget is enforced where it
matters — before a merge — rather than on every push.

## The four checks

**`code-quality`** — `cargo fmt --check`, `clippy -D warnings`, `svelte-check`,
`npm run check:imports`, `npm run contrast`. Deliberately holds no tests and no build so a
mistake is reported in two minutes.

`check:imports` deserves its place here. It fails if Mermaid, KaTeX, highlight.js or
CodeMirror appear in a *static* import anywhere under `src/`. That is the budget's most
easily broken invariant and the one a human reviewer will not catch: the symptom is not an
error, it is a cold start that got slower and an installer that grew, noticed weeks later by
the size gate if at all.

**`code-security`** — three scanners, each answering a different question:

| Scanner | Question |
|---|---|
| `cargo-deny` | RustSec advisories, licence policy, banned and duplicate crates |
| Trivy (filesystem) | vulnerabilities across `Cargo.lock` **and** `package-lock.json` |
| TruffleHog | verified secrets anywhere in the git history |

Two tools are absent on purpose, both because something above already covers them.
`cargo-audit` reads the same RustSec database `cargo-deny`'s `advisories` check does — it was
in the first draft of this pipeline, scanning the same lock file against the same data twice,
and it additionally needed `checks: write` to post its own check-run, which a fork pull
request never receives. `npm audit` is covered by Trivy, which reads `package-lock.json` and
reports the same advisories with SARIF output that reaches the Security tab.

TruffleHog runs with `--results=verified`. Unverified matches on a repository full of
hex-looking test fixtures are noise, and a scanner people learn to ignore protects nothing.
It is pinned to a tag rather than `@main`: an unpinned third-party action is a supply-chain
risk anywhere, and particularly so inside the workflow whose job is to catch one.

`src-tauri/deny.toml` also encodes the size decisions as policy. `ammonia`, `syntect`,
`tantivy`, `clap` and `rusqlite` are in its `deny` list with the reason attached, so a
rejected alternative cannot quietly return through a transitive dependency.

**`code-test`** — `cargo test` and `cargo test --no-default-features` on both Ubuntu and
Windows, `vitest` on Ubuntu.

The `--no-default-features` leg is not redundant. The render core runs headless for
`MD_HTML=1` and must not acquire a dependency on a default feature; that regression compiles
fine in the normal build and shows up only there.

**`size-gate`** — measures both installers, comments the table on the pull request, fails
over budget. The two ceilings mean different things: lite's **2.8 MiB** is a promise, full's
**12 MiB** is a tripwire for a dependency added by mistake. See
[size-budget.md](size-budget.md) and [editions.md](editions.md).

## Branch protection

`.github/rulesets/protect-main.json` is the ruleset as code. Apply it:

```bash
gh api -X POST repos/AlessandroLiscio/Marklet/rulesets --input .github/rulesets/protect-main.json
```

To update an existing one, find its id with `gh api repos/AlessandroLiscio/Marklet/rulesets`
and `PUT` to `/rulesets/<id>`.

What it enforces: no deletion, no force-push, pull request required, squash merge only, the
branch must be up to date with main, and five required checks.

> **The `context` strings are load-bearing.** Each is
> `<job id in main.yml> / <name: of the job inside the reusable workflow>`. Renaming either
> side blocks every merge until this file is updated and re-applied. The comment blocks at
> the top of `main.yml` and of each reusable workflow say so too.

Two deliberate choices worth stating:

- **`required_approving_review_count: 0`.** Single-maintainer repository. The pull request
  exists so the checks run and the history stays reviewable, not to wait for an approver who
  is the same person.
- **`bypass_actors: []`.** An emergency fix goes through a branch and a PR like everything
  else. The checks take minutes, and a repository whose owner routinely bypasses its own
  gate does not have a gate.

## Releasing

The tag is the release. Everything before it is bookkeeping that has to be correct first.

`.github/RELEASE_TARGETS.yml` declares the three files a release moves — `package.json`,
`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` — and, just as importantly, the four
version-shaped locations it must **never** move. `Cargo.lock` and `package-lock.json` both
carry `marklet`'s version and look exactly like write targets; both are generated, and
hand-editing them is how a lock file stops matching its manifest.

`release.yml` on a `v*` tag:

1. verifies the three version files agree with the tag, and fails the whole release if not;
2. builds `full`, `lite` and `offline` Windows installers plus `.AppImage` and `.deb`;
3. runs the cold-start gate on both editions — nine runs each on `windows-latest`, the
   **fastest** under 1200 ms for lite and 1800 ms for full. The number gated is the one the **window**
   path prints, in `lib.rs`'s `setup`, after `WebviewWindowBuilder::build()` returns: the
   installer is installed, the app is launched on a generated 470 KB document, and each run
   is killed once it has reported. `--benchmark`, which measures the render alone, is shown
   in the same summary but not gated — it is single-digit milliseconds and would pass any
   ceiling, so letting it stand in for the windowed number would be a gate that cannot fail.

   **The fastest, not the median**, because the two sources of error here — filesystem and
   loader warming, and a shared runner's scheduling — can only make a launch slower. When the
   noise has a sign, the minimum is the honest estimator of the cost and an average measures
   the machine. Three release runs of identical bytes proved the point: medians of 625 ms,
   617 ms and 2153 ms, the last from `5802, 4391, 2153, 1723, 1222` — a warming curve rather
   than an application. The first launch after an install is reported on its own line and
   never gated; WebView2 creates its user data directory once, and that cost is not ours;
4. writes `SHA256SUMS` and a size table into the run summary;
5. opens a **draft** release.

Draft, not published. Step 6 of `.claude/skills/release/SKILL.md` is a manual
install-double-click-uninstall on real Windows, ending with
`reg query HKCU\Software\Classes /f marklet /s` returning nothing. CI cannot test the
Explorer double-click path, and that path is the product.

Use the `release` skill to cut one and `version-sync-check` to audit the bookkeeping
without changing anything.

## What is not tested

Worth stating plainly, because a green pipeline invites the assumption that it is.

**Nothing asserts what the app looks like.** No WebDriver suite, no screenshots, no pixel
comparison. What exists instead is one smoke step per build leg: the real binary is launched
on `tests/fixtures/kitchen-sink.md` and has to reach the `boot-ms` line that is printed after
the window is created — under `xvfb` on Linux, directly on Windows. A missing `dist/`, a CSP
that blocks the bundle, a webview that will not initialise and a panic before the window
exists all fail there. Whether the outline is aligned, whether dark mode is readable, whether
a table overflows: unverified, and only a person looking at it will say.

A `tauri-driver` + WebdriverIO suite is the tool for the gap and is not built. It needs
`msedgedriver` on Windows and `WebKitWebDriver` under `xvfb` on Linux, both pinned against
the runner's browser version, and it is the most failure-prone part of a Tauri pipeline. It
is worth adding when there is a UI regression it would have caught; adding it first, and
maintaining it flaky, is how a suite ends up disabled.

**`PrintToPdf` is never called.** `scripts/check-windows.sh` type-checks the module from
Linux and `code-test` compiles it on `windows-latest`, but no CI step produces a PDF. The
WebKitGTK half is worse off: it needs the GTK "file" print backend and a display, so it is
not even exercised. Both are on the manual pass in `.claude/skills/release/SKILL.md`.

**The Explorer double-click path.** The registry round trip is tested — keys written, keys
gone, nothing left behind — but no CI step double-clicks a `.md` file, and that is the
product. Also manual, step 6.

## Distribution

GitHub Releases only. No registry, no cluster, no update server.

| Artifact | Edition | For |
|---|---|---|
| `marklet-setup-<ver>.exe` | full | Windows 11, **the default download** |
| `marklet-lite-setup-<ver>.exe` | lite | Windows 11, the deliberate small choice |
| `marklet-offline-setup-<ver>.exe` | full | air-gapped Windows; embeds the WebView2 runtime, **208 MB** measured |
| `marklet-<ver>.AppImage` | full | Linux, self-contained |
| `marklet-<ver>.deb` | full | Debian and Ubuntu |
| `SHA256SUMS` | — | covers every artifact above |

The editions are two products, not a build variant. See [editions.md](editions.md).

**Not done, and each is a real decision rather than an oversight:**

- **No auto-updater.** `tauri-plugin-updater` needs a signing key, a hosted update manifest
  and roughly 200 KB in the binary. For a viewer that opens local files, checking GitHub
  Releases by hand is an acceptable cost; revisit if it becomes a complaint.
- **No code signing.** An OV certificate is a recurring cost and SmartScreen will warn on
  first run until the installer accumulates reputation. Worth doing before recommending the
  app to anyone who is not technical.
- **No winget or Scoop manifest.** Both are cheap and both are worth adding once there is a
  stable release to point them at. A manifest for a v0.x that changes weekly is churn.

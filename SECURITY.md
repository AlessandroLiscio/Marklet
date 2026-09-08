# Security policy

## Reporting a vulnerability

Report privately through a
[security advisory](https://github.com/AlessandroLiscio/Marklet/security/advisories/new).
Do not open a public issue.

Expect an acknowledgement within a few days. This is a single-maintainer project, so a fix
timeline depends on severity and on how much of the code the fix touches — you will be told
which, honestly, rather than given a date that slips.

## What is in scope

Marklet renders untrusted input. A `.md` file arrives from the internet, is parsed, and its
HTML is written into a webview inside a process that can read and write the filesystem. The
interesting failures all live on that path:

- **Rendered content escaping the sanitizer** — script execution from a markdown file,
  through raw HTML, an attribute, a URL scheme, or a construct the allowlist did not
  anticipate.
- **Path traversal** — reading or writing outside the open document's directory or the open
  vault root, through the `marklet://` asset scheme or through a command argument.
- **A privileged action reachable without validation** — anything that reaches the disk
  without passing the checks in `ipc.rs`.
- **Secrets or user file contents leaving the machine.** Marklet makes no network requests
  at all; any that appear are a finding by definition.

## What is not

- The Windows installer is unsigned, so SmartScreen warns on first run. Known, documented in
  the README, and a cost question rather than a vulnerability.
- The `-offline` installer embeds the WebView2 runtime and is therefore only as current as
  the release it shipped with. Use the normal installer unless the machine is air-gapped.
- A malicious `.md` file that renders *confusing* content — a misleading link label, say — is
  markdown behaving as designed. A malicious file that *executes* something is not.

## How the code defends itself

Four layers, each written assuming the others might fail:

1. **Allowlist sanitizing** in `src-tauri/src/render/sanitize.rs` — eight tags, six
   attributes, everything else dropped, including every `on*` handler and every
   `javascript:` URL. Tested against a fixture corpus of real payloads.
2. **A strict CSP** — `script-src 'self'`, no `unsafe-eval`. If a bundled library ever needs
   `eval`, it goes in a sandboxed iframe rather than weakening the application.
3. **Canonicalize-then-check** in `protocol.rs` and `ipc.rs`. Validation happens *after*
   path resolution; checking a raw string for `..` first is a bypass waiting to happen.
4. **No capability granted to the frontend** — no filesystem, no shell, no http, per
   `src-tauri/capabilities/default.json`. An injected script can only call what the UI can
   call.

## Supply chain

Every pull request runs `cargo-audit`, `cargo-deny` (advisories, licences, banned crates),
Trivy across both lock files, and TruffleHog over the full history. `code-security.yml` also
runs weekly on a schedule, because an advisory published on Tuesday affects a dependency
that has not changed since March.

Release artifacts ship with `SHA256SUMS`.

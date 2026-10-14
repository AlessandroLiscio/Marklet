# Marklet

Lightweight, open-source Markdown viewer. Tauri 2 (Rust core) with a Svelte 5 and TypeScript frontend, shipped as two editions (lite and full) for Windows 11 and Linux.

## Code Review Rules

Flag only P0 and P1 findings: bugs, data loss, security holes, broken builds, broken release gates. Skip style, naming and nitpicks.

Priorities:

- Tauri IPC safety: every `#[tauri::command]` validates its inputs, returns errors instead of panicking, and its argument names match the frontend `invoke` calls (`npm run check:ipc`). Capabilities in `src-tauri/capabilities/` stay minimal.
- File-system paths: no path traversal, no trusting a path from the webview, no unchecked reads of huge files. Handle Windows paths (drive letters, backslashes, UNC) as well as Linux ones.
- Size budget: lite installer ceiling 2.8 MiB, full 12 MiB. A new crate or npm package needs its edition and measured xz size. Heavy libraries (mermaid, katex, highlight.js) stay behind `import()` and never on the boot path (`npm run check:imports`).
- Windows and Linux parity: file association, registry and installer changes must not break the other platform. CI runs both.
- Version sync: `package.json`, `Cargo.toml` and `tauri.conf.json` versions must agree on any release change.
- Tests: a behaviour change or bug fix comes with a vitest or `cargo test` case that fails without the change.
- Secrets: no tokens, keys or `.env` files in the diff.

Ignore: formatting that rustfmt, clippy or svelte-check already enforce, generated files, and lockfile churn (`package-lock.json`, `Cargo.lock`) unless a dependency change is suspicious.

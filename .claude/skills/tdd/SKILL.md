---
name: tdd
description: Test-driven development in red-green-refactor cycles. Use when building a feature or fixing a bug test-first, when the user says TDD or red-green-refactor, or when new behaviour needs tests before code. Marklet: TypeScript, Tauri, Rust.
---

# Test-driven development

Copy this skill into each repo and fill in the "Repo specifics" section. Work in small vertical slices: one failing test, the smallest change that passes it, then clean up with the tests green.

## The loop

1. **Red.** Write one test for one behaviour. Run it. It must fail, and for the right reason (the behaviour is missing, not a typo, import error or broken fixture). Read the failure message before moving on.
2. **Green.** Write the smallest change that makes that test pass. No extra features, no speculative parameters, no code for the next test. Run the test, then the suite.
3. **Refactor.** With every test green, remove duplication, rename, and simplify. Run the tests after each small step. If one goes red, undo the last step. Never change behaviour and structure in the same step.
4. **Repeat** with the next behaviour. Each test responds to what the last cycle taught you.

Do not write all tests first and all code after. Bulk tests describe imagined behaviour and lock in a structure before you understand it.

For a bug fix: write a test that reproduces the bug and fails, fix it, keep the test.

## What a good test is

- It checks behaviour through the public interface: a function's return, a command's response, what a user sees. It survives a refactor because it does not care about internal structure.
- Its name reads like a spec: "rejects a file larger than the limit", not "test_validate_2".
- The expected value comes from an independent source (a literal, a worked example, the spec), never from recomputing the answer the way the code does. A test that restates the implementation passes by construction.
- One reason to fail. A failing test names the broken behaviour.

## Where tests go

Test at the public seams: the exported function, the command handler, the UI component's visible output, the file or API a user touches. Before the first test of a feature, say which seams you will test and what each one catches. Ask Alessandro only when the seam is a real design choice; for a bug fix, use the seam where the bug shows.

## Anti-patterns

- Mocking internal collaborators or testing private methods. The tell: the test breaks on a refactor with unchanged behaviour. Mock only at system edges (network, clock, filesystem, randomness, OS).
- Tautological assertions (`expect(add(a, b)).toBe(a + b)`).
- Asserting through a side channel (reading the database instead of calling the interface).
- Snapshot tests of large outputs nobody reads. Prefer a few exact assertions.
- Skipping or deleting a failing test to get green, or weakening an assertion. Fix the code or ask.
- Hard UI work: pure test-first can slow down visual layout. Test the logic and the states first, check the look by eye or by a screenshot.

## Rules

- Run the narrowest test while iterating, the whole suite before saying done. Report the pass and fail counts.
- If a command prints long output, wrap it in a script that follows the `quiet-scripts` skill (status lines, logs under `tmp/reports/`).
- Do not run git. Changes stay in the working tree and the final report lists paths and line numbers.
- If a slice fails twice for the same reason, stop and say what you learned instead of piling on edits.

## Repo specifics

Marklet: Svelte 5 and TypeScript frontend, Tauri 2 shell, Rust core in `src-tauri/`. The lite edition is the default; the full edition adds `--features full`.

- TypeScript tests: vitest. Narrow run `npx vitest run tests/unit/<file>.test.ts`; unit suite `npm run test:unit`; component suite `npm run test:components`; both with `npm test`. Unit tests live in `tests/unit/`; a test that needs a DOM sets `// @vitest-environment happy-dom`.
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml <test_name>` while iterating, without the name for the full run. CI also runs `--no-default-features`; run it when you touch feature-gated code, and add `--features full` for full-edition code. Unit tests sit in a `#[cfg(test)]` module next to the code, integration tests in `src-tauri/tests/`.
- Tauri commands: test the plain Rust function behind a `#[tauri::command]` directly and keep the command thin. Frontend code that calls `invoke` mocks `invoke` at that edge only.
- Checks before done: `npm run check` (svelte-check), `npm run check:imports`, `npm run check:ipc`, `npm run check:print`, `npm run check:shortcuts`, `npm run contrast`, and the CI Rust checks verbatim from `.github/workflows/code-quality.yml`: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. A dependency change also needs `npm run size` (see the size-budget skill).

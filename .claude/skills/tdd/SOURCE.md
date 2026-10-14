# Source

- Upstream: https://github.com/mattpocock/skills, skill `engineering/tdd/SKILL.md` (with `tests.md` and `mocking.md`, not copied). Licence MIT, copyright Matt Pocock, copied in `LICENSE`.
- Commit read: 49dd158d1076134a641b33efb035946536778336 (2026-10-09), from the local clone used by research reports B3 and V1.
- Taken: red before green, one slice at a time, behaviour through public interfaces, the anti-patterns of implementation-coupled, tautological and horizontal-slice tests, tests at seams.
- Changed: upstream says refactoring is not part of the loop and sends it to review. This version includes refactor as step 3 with tests green, as asked for this setup. Upstream requires confirming seams with the user before every test; here that is limited to real design choices, since the report (B3) found it slows autonomous runs. Added bug-fix flow, the UI caveat from the report, quiet-scripts and no-git rules, and a Marklet section (TypeScript, Tauri, Rust). Upstream references to `codebase-design`, `GLOSSARY.md` and `code-review` skills removed.
- Not checked: Marklet commands are conventions to confirm in the repo, not read from it.

<!--
The PR title becomes the squash-merge commit subject, so it follows the commit
rules: <type>(<scope>): <subject> — imperative, lowercase, no trailing period,
72 characters or fewer. See CONTRIBUTING.md.
-->

## Summary

<!-- What this changes and why, in two or three sentences that read on their own. -->

Closes #

## Type of change

<!-- Tick one. It must match the type in the PR title. -->

- [ ] `feat` — a new user-facing capability
- [ ] `fix` — a bug fix
- [ ] `perf` — performance improvement
- [ ] `refactor` — restructuring, neither fixes a bug nor adds a feature
- [ ] `docs` — documentation only
- [ ] `test` — adding or correcting tests
- [ ] `build` — build system, dependencies, packaging
- [ ] `ci` — workflows or pipeline scripts
- [ ] `chore` — maintenance touching no source or test code
- [ ] `revert` — reverting a previous commit

## What changed

<!-- The notable changes, one bullet each. Point reviewers at the files that matter. -->

-

## Size impact

<!--
Required if this adds ANY dependency, crate or npm package. The size gate will
tell you the installer delta, but say here what you measured and why it is
worth paying. "Nothing added" is a valid answer.

  npm pack <pkg> --pack-destination /tmp >/dev/null && tar -xOf /tmp/<pkg>-*.tgz | xz -9 | wc -c

See .claude/skills/size-budget/SKILL.md.
-->

Nothing added.

## How verified

<!--
The commands you actually ran and their result. "Should work" is not
verification. If you skipped a check, say which and why.
-->

```bash

```

Result:

## Breaking changes

<!--
"None" if there are none. Otherwise: what breaks, who it affects, and the
migration step. A breaking change also needs `!` in the title and a
`BREAKING CHANGE:` footer on the commit.
-->

None

## Checklist

- [ ] Title follows `<type>(<scope>): <subject>` and reads as the squash-merge subject
- [ ] Linked to an issue above, or the body explains why there isn't one
- [ ] Scoped to a single concern — a reviewer can hold the whole change in their head
- [ ] Any new dependency has its measured compressed size in **Size impact**
- [ ] Nothing heavy added to a static import (`npm run check:imports` passes)
- [ ] Tests added or updated for the changed behaviour, or not applicable
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` if this is user-visible
- [ ] No secrets, tokens or credentials in the diff

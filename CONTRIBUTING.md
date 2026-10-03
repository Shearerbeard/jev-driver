# Contributing to jev-driver

Thanks for the interest. This file is for code contributions; the
release process is maintainer-only and lives in
[README "Publishing (maintainers)"](README.md#publishing-maintainers).

## Before you open a pull request

Run the gates from the repository root - CI runs the same commands:

```sh
make check        # fmt + clippy + unit tests (offline)
make red          # acceptance harness against recorded fixtures
make rename-test  # derives must expand under a renamed dependency
```

Details on what each tier covers: README
["Testing and the live smoke"](README.md#testing-and-the-live-smoke).

## Conventions

- Conventional Commits, lowercase, under 72 characters (`feat:`,
  `fix:`, `docs:`, `chore:`).
- Workspace lints are strict (restriction lints at deny, including
  `unwrap_used`/`expect_used` outside tests). Any `#[allow]` needs a
  `reason`.
- New public surface should be exercised in `testapp` (the executable
  spec) before it lands.
- The typed answers layer is strict by design: unknown option keys,
  unasked answers, and unnormalized distributions are errors, not
  silent defaults. Keep new surface that way.
- Prose changes to README or CHANGELOG are linted with Vale; keep to
  the style already in the file (plain hyphens, no em dashes).

## Pull requests

Small, focused PRs against `master`. Describe what changed and why;
the commit message carries the detail. Expect CI (fmt, clippy, unit,
acceptance, rename, cargo-deny) to pass before review.

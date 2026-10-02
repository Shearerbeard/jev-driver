# CLAUDE.md - Project Guide

## Project Overview

`jev-driver` is a typed Rust driver for TypeSafe AI's Jev (System One)
decision model (choice / score / noul primitives), with a derive surface, a
serializable schema IR, and a strict validation layer.

`README.md` is the canonical user documentation and covers everything
user-facing, from the crate overview and authoring surfaces through the
strictness contract to the make targets. Do not restate that here.

## Commands

Make targets and the live-smoke setup are documented in `README.md`
("Testing and the live smoke"); `TYPESAFE_API_KEY` details live there too.
`make check` is the default gate before any commit.

Clippy scope note: `make check` lints the library crates only. The `ui` and
`testapp` targets intentionally trip restriction lints; use the Makefile
targets, not raw `cargo clippy --all-targets`.

## Conventions

- Conventional Commits, lowercase, under 72 chars (`feat:`, `fix:`, `docs:`,
  `chore:`).
- Workspace lints are strict (restriction lints at deny, incl.
  `expect_used`/`unwrap_used` in non-test code). `#[allow]`s need a `reason`.
- testapp is the executable spec: public-API changes must keep `make red`
  green, and new public surface should be exercised there first.
- Current state and next steps live in `NEXT-SESSION.md` (single roadmap; no
  TODO.md).

## Status

0.1.0, master branch. Remote: github.com/Shearerbeard/jev-driver
(public since 2026-09-25). Publish prep is complete (packaging,
rename-safety, consumer proof - see NEXT-SESSION.md); the crates.io
upload itself is the maintainer's to run (runbook in README
"Publishing"). What is deliberately deferred past 0.1.0 is recorded
in `NEXT-SESSION.md`.

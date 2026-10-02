# Handoff - jev-driver 0.1.0, published

State: 0.1.0 is live on crates.io (jev-driver and jev-driver-macros,
2026-10-02, uploaded by the maintainer after a gated session).
Verified post-publish: docs.rs builds both crates, and a fresh
consumer app (`cargo add jev-driver`) builds and runs the triage demo
from the real registry. The publish-prep session's work and review
record are below; earlier waves (soundness close-out through 1bcb123,
endpoint configurability, backtick serde-keys fix, dual-review
hardening) live further down and in git history.

## Publish-prep session (this file's current state)

Landed, all gates green (`make check` 31+6+ui, `make red` 11,
`make rename-test` 1, fmt/clippy/-D warnings clean, `cargo doc -D
warnings` clean, `cargo deny check` green):

- `proc-macro-crate` adopted: the derives resolve the consumer's
  dependency name at expansion (`crate_path()` in macros `util.rs`);
  a renamed dependency works. `rename-test/` member proves it with
  every derive plus both hidden helpers (`make rename-test`).
- Macro-addressed surface centralized: `#[doc(hidden)] __private`
  now hosts `score_editor_levels` (moved out of `question`),
  `validate_state_refs` (also still public IR API at `refs::`), and
  `insert_question` (dedups the three duplicate-id checks).
- Packaging: `readme = "../README.md"` + LICENSE files in both
  tarballs (verified via `cargo package --list`), `publish = false`
  on testapp and rename-test, macros pinned `=0.1.0`.
- `dotenv` feature (default on) gates dotenvy; default behavior
  unchanged, `default-features = false` drops the `.env` load.
- Prelude slimmed of `WireRequestBody`/`WireResponse`/`WireAnswer`
  (still public at `wire::`). In-repo consumers updated.
- Cheap wins: `Usage: From<WireUsage>`; `is_non_empty` on the
  structured form now judges the embedded `question` string (data-only
  maps and blank questions reject - stricter wire validation,
  acceptance fixtures unaffected); intra-doc links fixed.
- Docs: README truth-fix (repo public since 2026-09-25, not private),
  publish runbook, three test tiers, rename-test listed; CHANGELOG.md
  added (0.1.0 entry, dated at publish); CI workflow (fmt, lib-scope
  clippy, unit + acceptance + rename tests, cargo-deny).
- Consumer proof: scratch app outside the workspace resolved
  `jev-driver = "0.1"` from a local directory registry (macros from
  its real tarball, runtime from source with the exact
  path-to-version rewrite). Fully offline build + run: typed answers,
  tampered noul rejected at the boundary.

Review record: every stage dual-gated (self + rust-reviewer or
frontier-reviewer); three FAIL verdicts were returned and fixed in
closed loops (docs truth-fix assertions, crate_path error propagation,
example lint/wildcard arm). Empirical findings worth keeping:

- `cargo package`/`publish --dry-run` for the runtime CANNOT succeed
  until macros exists on the crates.io index (checked against the real
  index; `[patch.crates-io]` does not bypass it). Macros dry-run is
  green; the runbook's macros-first order is mandatory, not advisory.
- The runtime's own examples/tests expand derives via the Itself path;
  they need the macro-addressed names at their crate root (see the
  comment in `examples/triage.rs`).
- `cargo vendor` uses the flat `vendor/<name>/` layout on this
  toolchain; version subdirectories are silently ignored.
- cargo-deny's CLI takes no `--workspace` or `--all-features` flags
  (feature selection lives in deny.toml `[graph]`); a bare
  `cargo deny check` from the root covers the whole workspace. The
  CI job hit this twice - and the second flag was caught locally but
  a piped command masked its exit code, repeating the old
  "check gate exit codes directly" lesson.

## Publish - DONE 2026-10-02

The maintainer checklist ran end to end: `cargo login` (human),
macros published first, index propagated on the first retry, runtime
dry-run green, runtime published, `v0.1.0` tagged, README install
snippet swapped to the crates.io line, CHANGELOG entry dated. Next
release: bump `workspace.version`, add a dated CHANGELOG entry,
re-run the gates, and follow README "Publishing" unchanged.

## Configurable endpoint for local gateways (this session, b645166 + b25bca4)

`JevConfig` now targets any System One-compatible endpoint, cloud or
local: `api_key` is `Option` (no key = no bearer header), `base_url`
was renamed to `endpoint` (the complete URL; `JevConfig::base` joins
`v1/systemone` onto pathless bases), and `EnvNames` +
`from_env_named` take caller-chosen env var names. `from_env` keeps
`TYPESAFE_*` with the key now optional. testapp grew `--endpoint`/
`--model` (an explicit `--endpoint` never sends a key) and the Makefile
grew `live-local` (`JEV_LOCAL_ENDPOINT`/`JEV_LOCAL_MODEL`).

Gates: three rust-reviewer rounds (all PASS-WITH-CONDITIONS, all
conditions fixed: unsafe-lint discipline on edition-2024 `set_var`,
key trim/blank normalization, cloud-key leak on `--endpoint`,
flag-value guards, plus a comment-reduction pass: the join rule is
documented once, on `JevConfig::base`). `make check` 31 tests (one
parallel-env race introduced by a reduction merge was caught and fixed:
env tests need distinct variable names), `make red` 11, live smoke
green against the local gateway at `127.0.0.1:8009/v1/systemone`
(`kev-latest` 208ms, `jev-latest` 149ms, keyless) and `make live`
against the cloud. Gateway note: only `kev-latest`/`jev-latest` were
exposed by `/v1/models` at smoke time. KevK5, Nimble, and Von exist but
are untested; active experimentation is Kev 9B (local) and Jev (frontier
remote). List further models in the README's live-local note as they are
verified.

## Backtick serde-keys fix (branch fix/backtick-serde-keys)

Backtick references now validate against serde's serialized keys instead
of Rust field names. `JevInstructions` checks its data fields at compile
time (rename/rename_all resolved in-macro; skipped fields are hard
errors; flatten/conditional-skip/custom-serialize structs are exempt).
The unused `state.` prefix convention is gone: references follow the
API's path grammar verbatim, and `JevQuestions` composition root-checks
every composed question against the paired state's `StateKeys` (new
trait, emitted by `JevState`) plus option keys and structured
instruction data fields, failing with `JevError::DanglingStateRef`.
`JevChoice`/`JevScore` lost their compile-time ref rules (they only
accepted the dangling-prefix form); `JevNoul` gained compose-time
coverage it never had. Wire text is WYSIWYG; schema IR unchanged.

## What this session added (after the close-out)

A dual-reviewer audit (type soundness, DRY, code bloat) over the final
tree, then two fix rounds until both reviewers signed off:

Round 1 found three cross-verified majors; all fixed:
1. Score-level key aliases (`"00"`) passed wire validation and collapsed
   in typed conversion. Now: wire path rejects non-canonical keys, typed
   parse rejects collapsing keys.
2. `RetryConfig::delay_for` panicked on NaN multipliers and
   `Duration::MAX` overflow. Now: sub-unit/NaN multipliers sanitize to
   1.0, non-finite products clamp to `max_interval`, the duration
   conversion is checked and saturates at `max_interval`.
3. Answer strictness was construction-path only. Now: `Answers.answers`,
   `ChoiceData`, `ScoreData`, `ScoreDecision.score`, and
   `ScoreDecision.probabilities` are private behind read accessors
   (`selected()`, `probabilities()`, `confidence()`, `score()`, `legend()`,
   `level()`); validated answers cannot be mutated or hand-built outside
   the crate.

Round 2 caught the incomplete first fix attempt (three unmigrated
`.selected` reads broke acceptance-test compilation, `score: pub f64`
still injectable, residual duration panic, two doc inaccuracies); round 3
verified all closures. Lesson recorded: a piped `make red | grep | tail`
masked a compile failure; gate exit codes must be checked directly.

Gates at handoff (exit codes verified): `make check` (24 tests, clippy
clean), `make red` (11 acceptance), `make live` (202ms live round-trip,
typed answers, no false rejections from the stricter key checks).

## Heavy full-repo review (dual-family, PASS-WITH-CONDITIONS both)

Staged packet: agent-driver-rs `.review/README-heavy.md` +
`jev-driver-full-tree.diff`. Verdicts: PASS-WITH-CONDITIONS from both
reviewers; no BLOCKING findings; all recorded deferred items re-graded
(only `Limits` worse than recorded — it is a fourth copy of the
cardinality constants, not a source of truth).

P1/P2s fixed immediately after the review (uncommitted at time of
writing, gated green):

1. P1 — serde_names diverged from serde's actual rename algorithm:
   serde splits words on `_`/`-` ONLY (never case boundaries — that is
   heck's behavior, deliberately not serde's), and `lowercase` /
   `snake_case` are identity on fields (Rust convention), with
   `UPPERCASE`/screaming/kebab as whole-ident transforms. Fixed to
   mirror serde exactly; the differential test now runs adversarial
   idents (`xmlHttpRequest`, `XMLHttp`) through every rule. Verified
   against `serde_json::to_value` output.
2. P1 — `JevConfig` derived `Debug` printed the raw API key. Hand impl
   redacts; regression test asserts no leak.
3. P2 — `container_rename_all` dropped the rename rule when a sibling
   valued meta (`bound`, `crate`, `remote`) was present (syn rejects
   unconsumed values). Now drains like `field_rename`; test with
   `bound = ""`.

Remaining review conditions (do before publish, sequenced with API
testing — reviewer "first three changes"):

- Make the IR the authority at its own boundary: validated
  constructors (or per-spec validation in `into_wire`) + add
  `deny_unknown_fields` to `QuestionSpec`/`StateSpec` — closes the
  into_wire bypass AND the silent runtime->static flip on a
  `criteria_source` typo.
- One discriminant, one limits table: revive `QuestionKind::as_str()`
  as the single source for the four hand-written `kind_str()` matches;
  centralize 255 / 2..=10 (currently four sites across two crates).
- Decide the runtime-criteria artifact story: criteria channel on
  `SchemaQueryBuilder` + capture editor criteria in exports, or
  retitle the artifact "validated schema export" and drop the DMMF
  framing (today's runtime-criteria artifacts are non-executable and
  non-diffable).
- Publish-wave bundling: slim the prelude of `WireRequestBody`/
  `WireResponse`/`WireAnswer`; move macro-addressed helpers
  (`score_editor_levels`, `validate_state_refs`) to `#[doc(hidden)]
  __private` alongside `proc-macro-crate`; feature-gate `dotenvy`;
  consider renaming `Decision` (request vs per-question outcome
  overload) before the API is consumed publicly.
- Smaller accepted clusters: duplicate-id check x3 (one helper);
  `Usage`/`WireUsage` `From` impl; choice/score criteria-uniformity
  util; cross-ref comments on the two backtick extractors; broken
  intra-doc links (schema.rs `DynChoice`, macros lib.rs);
  `is_non_empty` on structured form should check the `question` string.

## Publish readiness (after API testing) - LANDED 2026-10-02

Every item on this list landed in the publish-prep session (see the
top of this file): `proc-macro-crate` adoption + rename test, readme
packaging, license texts in both tarballs, `publish = false` on
testapp, exact-pin of macros, plus the `dotenv` feature gate, prelude
slim, and the cheap-wins cluster. What remains is the upload itself
(README "Publishing").

## Deferred (recorded by both reviewers, none blocking)

- `QuestionSpec::into_wire` / hand-implemented `QuestionType` can lower
  IR that `validate()` would reject.
- Generated `schema()` export silently truncates duplicate question ids
  (compose rejects them at runtime).
- `JevInstructions` generics miss a conditional `Serialize` bound (E0277).
- `JevQuestions` state refs beyond the root segment (nested paths,
  `with_question` dynamics) are unchecked by design in 0.1.x.
- Dead public surface: `QuestionKind`/`KIND`, `choice_options()`,
  `iter_names()`, `option_null()`; unreachable static-empty-score branch.
- Malformed `#[jev]` field attrs are swallowed silently in state.rs.
- `Limits` is a constants table wearing a pub-fields struct.
- No `Retry-After`/jitter; in-flight requests not abortable.

## Maintainer scope decisions for the 0.1.0 publish (2026-10-02)

- `Decision` rename (request vs per-question outcome overload): keep
  for 0.1.0, rename in 0.2 before a broad audience consumes it.
- Reviewer hardening list (IR-as-authority, one discriminant/limits
  table, criteria channel on `SchemaQueryBuilder`): only the cheap
  wins land pre-publish; the rest stay deferred.
- Publish execution stops at a verified dry-run; the maintainer runs
  `cargo publish` (README "Publishing").

## Relationship to agent-driver-rs

Stage 5 (ADR-0008) remains a separate later wave in that repo's TODO.md.

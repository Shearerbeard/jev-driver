# Handoff - jev-driver 0.1.0 prototype, soundness-hardened

State: complete and reviewed. Master has the close-out commits through
1bcb123 plus this session's soundness pass (three dual-review rounds,
frontier-reviewer + codex, final verdict PASS from both). Previous Gate A
record and findings history: git history at f2c15c6 and earlier.

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

## Publish readiness (after API testing)

Deferred deliberately: land these once the API surface has been
consumed and validated, just before making the repo public.

- IMPORTANT — `proc-macro-crate` adoption: macros generate hardcoded
  `::jev_driver::` paths, so a renamed dependency breaks expansion.
  Resolve the real crate name at expansion (~30 lines + rename test).
- Package the README: `readme = "../README.md"` on both crates
  (verified: Cargo packages it).
- Ship the license texts: `include` both LICENSE files or copy them
  into each crate (neither tarball carries them today).
- `publish = false` on testapp.
- Pin `jev-driver-macros` exact (`=0.1.0`, thiserror pattern).
- Sequence: flip repo public -> check name availability -> publish
  macros first, then runtime -> tag v0.1.0 -> swap the README install
  snippet to the crates.io line.

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
- Publish blockers: repo is private and `proc-macro-crate` not adopted
  (remote added and `repository` field set 2026-09-22).
- No `Retry-After`/jitter; in-flight requests not abortable.

## Relationship to agent-driver-rs

Stage 5 (ADR-0008) remains a separate later wave in that repo's TODO.md.

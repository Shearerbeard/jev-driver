# Handoff - jev-driver 0.1.0 prototype, soundness-hardened

State: complete and reviewed. Master has the close-out commits through
1bcb123 plus this session's soundness pass (three dual-review rounds,
frontier-reviewer + codex, final verdict PASS from both). Previous Gate A
record and findings history: git history at f2c15c6 and earlier.

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

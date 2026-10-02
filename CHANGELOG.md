# Changelog

Notable changes to jev-driver. Format: Keep a Changelog. Before 1.0 a
minor bump may break anything; read the entry before upgrading.

## [0.1.0] - 2026-10-02 - first release

### Added

- Derive surface: `JevChoice`, `JevScore`, `JevNoul`, `JevState`,
  `JevQuestions`, `JevInstructions` - questions declared as Rust types,
  typed answers generated per question set, and compile-time
  backtick-reference validation against serde's serialized keys.
- Runtime criteria: any derive-level question can defer its rubric text
  to call time via `customize`, without changing types.
- Schema IR: `DecisionSchema` artifacts (load, author, export), ad-hoc
  queries next to derived ones (`DynChoice`, `with_question`), and the
  strict `Answers::parse_as` bridge back into derived types.
- `JevClient`: cloud or local System One-compatible endpoints, optional
  bearer key, caller-chosen env var names (`EnvNames`), capped
  exponential backoff on 429/529, cancellation-token checks, cheap
  clone.
- Strictness contract on every path: exact question coverage, fully
  enumerated distributions that sum to 1, canonical score keys, finite
  bounded probabilities and confidence - invariant-bearing fields are
  private behind read accessors.
- `testapp` acceptance harness: offline recorded fixtures (`make red`)
  plus live smoke modes (`make live`, `make live-local`).

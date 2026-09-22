# Gate A review — jev-driver full implementation

Canonical durable copy of this handoff: `/home/shearerbeard/dev/jev-driver/NEXT-SESSION.md`
(committed in that repo). This `.review/` directory is untracked by design —
if it is lost, use the canonical copy.

Diff anchors, to avoid misreads:
- `.review/jev-driver-full.diff` = empty tree .. c507dc3 (pre-fix implementation)
- `.review/jev-driver-fix.diff` = c507dc3 .. a905b68 (the gate fixes; ALREADY
  staged — regenerate only if the repo moved)
- Test counts: 26 green = pre-fix state (packet README); 31 green = post-fix
  re-verification (same code after the fixes below). Not a contradiction.

Reviewer: frontier-reviewer (kimi-code-plan-global/k3) — dispatched because the
rust-reviewer pin (kimi-for-coding/kimi-for-coding) was unavailable at the time.
The user has since fixed the rust-reviewer pin; the NEXT session should use
rust-reviewer for the fix-diff verification (verify the pin resolves a model
before dispatch — the claim is not checkable from inside this workspace).

## Verdict (original)

PASS — ready for 0.1.0 prototype publish once finding #1 is resolved.
No blockers. #1 was the one finding that matters for publish.

## Findings table (as reported)

| # | Severity | Where | Finding | Status |
|---|----------|-------|---------|--------|
| 1 | MAJOR | answer.rs parse_one vs typed parsers | Dynamic layer tolerated missing zero-probability option/level entries while typed layer hard-errored; live API omissions would fail parse_as with no test coverage | FIXED: completeness now enforced in parse_one (both layers uniform) + regression test |
| 2 | MINOR | macros/questions.rs compose | Duplicate question ids across field types silently overwrote | FIXED: checked insert at compose + regression test |
| 3 | MINOR | client.rs | Cancellation token only polled during backoff; pre-cancelled token still sent first request | FIXED: is_cancelled check before loop + regression test (no request leaves) |
| 4 | MINOR | client.rs delay_for | f64::clamp panics when max_interval < 1ms | FIXED: floor = min(0.001, ceiling) + regression test |
| 5 | MINOR | client.rs | Doc claimed cheap clone; no Clone impl | FIXED: #[derive(Clone)] |
| 6 | MINOR | macros/choice.rs | Duplicate option keys across variants produced rustc unreachable_patterns warning instead of macro error | FIXED: seen-key set, syn::Error on collision |
| 7 | MINOR | 5 macros | Generic types failed with raw rustc errors | FIXED: reject_generics with clear message (JevInstructions keeps generics) |
| 8 | MINOR | answer.rs | score value never range/NaN-checked | FIXED: finite + within 0..=levels-1 + regression test |
| 9 | MINOR | schema.rs SchemaQueryBuilder | Runtime-criteria questions unexecutable from IR surface | DOCUMENTED: derive-surface-only note on the builder |
| 10 | MINOR | Cargo.toml | Missing publish metadata; dotenvy side effect in lib | PARTIAL: keywords/categories added; repository URL deferred (no remote yet); dotenvy documented in from_env docs. proc-macro-crate deferred |
| 11 | NIT | wire.rs from_question | Dead map.clear() work | FIXED: iterate by value |
| 12 | NIT | client.rs | No Retry-After / no jitter | DOCUMENTED on JevClient |
| 13 | NIT | question.rs trait | parse arg order inconsistent (answer,id vs id,answer) | FIXED: id-first everywhere (trait + macros) |
| 14 | NIT | schema.rs | Choice/Score criteria lacked serde(default) | FIXED |
| 15 | NIT | fake.rs requests() | Poisoned lock silently swallowed | FIXED: expect with reasoned allow |

## Unverified-by-reviewer claims (packet was diff-only)

- fmt/clippy/test green — since re-verified locally after fixes: 31 tests
  green (6 client + 13 core + 1 ui + 11 acceptance), clippy clean with
  `--features test-support`.
- Wire shapes vs the live TypeSafe API — verified against the API docs and
  golden fixtures only; LIVE SMOKE STILL PENDING (see next-session notes).

## State at pause

Commits in /home/shearerbeard/dev/jev-driver (master):
- b8e17bc chore: scaffold jev-driver workspace (includes red testapp)
- c507dc3 feat: turn the red harness green (core, client, derive layer)
- a905b68 fix: address gate review findings (strictness parity, retry hardening, macro guards)

## Next session (in order)

1. Verify the rust-reviewer pin works (user fixed it): dispatch rust-reviewer
   on the fix diff (c507dc3..a905b68) — packet: regenerate
   .review/jev-driver-fix.diff via `git -C /home/shearerbeard/dev/jev-driver diff c507dc3 a905b68`
   plus this file for context.
2. Gate M — live smoke: `cd /home/shearerbeard/dev/jev-driver && make live`
   (TYPESAFE_API_KEY already extracted to .env, chmod 600, gitignored).
   Expected: typed answers in ~70-500ms; failure signatures: 401 key,
   429 backoff retries logged, 529 overloaded.
3. Full README (bus-test quality: quickstart both surfaces, design rationale,
   strictness contract, non-goals, live-smoke instructions).
4. Final commit + close-out report to user. Stage 5 (ADR-0008 in
   agent-driver-rs) remains a separate later wave as planned.

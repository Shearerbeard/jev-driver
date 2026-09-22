# jev-driver

A typed Rust driver for TypeSafe AI's Jev (System One) decision model.

Jev answers three kinds of questions about any JSON state you send it:

- **choice** - pick one option from a set you define
- **score** - rate the state against an ordered rubric (2..=10 levels)
- **noul** - estimate P(yes) for a statement

jev-driver turns those into Rust types: your option set becomes an enum, your
rubric becomes an enum's variants, your thresholds become methods on
probability types. Unknown option keys, unasked answers, missing criteria, and
unnormalized distributions are all errors, not silent defaults.

Status: 0.1.0 prototype. Workspace layout at the bottom.

## Quickstart: the derive surface

Declare your state and questions as Rust types:

```rust
use jev_driver::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JevState)]
#[jev(describe = "A single inbound support ticket")]
struct TicketState {
    ticket: String,
    #[jev(describe = "Hours left on the customer's SLA clock")]
    sla_hours: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JevChoice)]
#[jev(id = "department", instructions = "Which team should handle this ticket?")]
enum Department {
    Billing,
    Technical,
    #[jev(option = "other")]
    Other,
}

#[derive(Debug, Clone, Copy, JevNoul)]
#[jev(
    id = "is_urgent",
    instructions = "Does this ticket convey time pressure?",
    yes = "Explicit deadline or costly delay",
    no = "No time pressure expressed"
)]
struct IsUrgent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, JevScore)]
#[jev(id = "frustration", instructions = "How frustrated does the customer appear?")]
enum Frustration {
    #[jev(criteria = "Calm and neutral")]
    Calm,
    #[jev(criteria = "Concerned but civil")]
    Concerned,
    #[jev(criteria = "Very angry or strong language")]
    Angry,
}

#[derive(Debug, Clone, JevQuestions)]
#[jev(state = TicketState)]
struct TicketTriage {
    department: Department,
    is_urgent: IsUrgent,
    frustration: Frustration,
}
```

Build a decision, evaluate it, and match on your own enums:

```rust
#[tokio::main]
async fn main() -> JevResult<()> {
    let decision = TicketTriage::request()
        .state(&state)?
        .build()?;

    let client = JevClient::from_env()?;
    let answers: TicketTriageAnswers = client.evaluate(&decision).await?;

    // Plain match on your enum; confidence decides auto-action vs. review.
    let lane = match answers.department.selected {
        Department::Billing if answers.department.confidence.act(0.80) => "billing (auto)",
        Department::Technical if answers.department.confidence.act(0.80) => "technical (auto)",
        _ => "human review (low confidence)",
    };

    if answers.is_urgent.probability.yes(0.50) {
        // page on-call
    }
    Ok(())
}
```

`JevQuestions` generates `TicketTriageAnswers`: one typed field per question,
with `selected` (your enum), the full `ProbabilityMap`, `Confidence`, and for
scores a `score()` accessor plus `nearest()`, the rubric level with the
highest probability.

Runtime criteria: any derive-level question can defer its rubric text to call
time via `customize`, so tenant-specific wording doesn't change your types:

```rust
let decision = TicketTriage::request()
    .state(&state)?
    .customize(|c| {
        c.department = Some(DepartmentCriteria {
            billing: Instructions::text("Payments, invoicing, refunds"),
            technical: Instructions::text("Bugs and outages"),
            other: Instructions::text("None of the above"),
        });
    })
    .build()?;
```

## The IR surface

Two ways to go untyped, both producing the same `Decision` the derive surface
builds:

Ad-hoc questions next to derived ones (`DynChoice`):

```rust
let owner = DynChoice::new("owner", instructions)
    .option("dana", Instructions::text("Payments specialist"))
    .option("kai", Instructions::text("Integrations specialist"))
    .build()?;

let decision = TicketTriage::request().state(&state)?.build()?
    .with_question(owner)?;
```

Whole schemas as serializable artifacts (`DecisionSchema`, the DMMF analog):
load or author a JSON schema, compose a query against it, and bridge the
raw answers back into derived types with `Answers::parse_as`:

```rust
let schema = DecisionSchema::load("schemas/triage.json")?;
let decision = schema.query().state(&state)?.ask("department")?.build()?;

let raw = client.evaluate_raw(&decision).await?;
let typed: TicketTriageAnswers = raw.parse_as()?;   // strict bridge
let owner = raw.choice("owner")?;                    // or stay dynamic
```

Note: questions marked `criteria_source: "runtime"` are a derive-surface
feature in 0.1.0. They need a criteria editor and cannot be executed from
`SchemaQueryBuilder` alone.

## Strictness contract

Every answer, on every path, is validated against the question that asked it:

- The answer set must cover exactly the asked questions: no missing, no extra.
- A choice answer's distribution must name every option in the criteria,
  including zero-probability ones, and sum to 1 (within tolerance).
- A score answer's distribution must cover every rubric level with
  canonical keys (`"0"`, `"1"` - aliases like `"00"` are rejected); the
  score must be finite and inside the rubric's range.
- Noul probabilities and confidence values must be finite and in [0, 1].

The untyped layer (`Answers::from_wire`) and the typed layer (`parse_as` and
the generated `parse` impls) enforce the same rules; the wire path re-checks
what the typed layer would have caught, so a malformed response fails
at the boundary with a typed error naming the question.

The guarantees are type-level: the answer types (`Answers`, `ChoiceData`,
`ScoreData`, and the typed decisions' raw-`f64`/map fields) hold their
invariant-bearing fields privately and offer read accessors, so a validated
answer cannot be mutated or hand-built into an invalid one outside the
crate.

## Client behavior and limits

`JevClient` (via `JevConfig::from_env`, reading `TYPESAFE_API_KEY`; loads a
workspace `.env` if present) retries rate-limit (429) and overload (529)
responses with capped exponential backoff, and is cheaply cloneable.
Cancellation tokens are checked before the first attempt and between retries;
an already-in-flight HTTP request is not abortable in 0.1.0. Backoff honors
neither `Retry-After` headers nor jitter in 0.1.0. See `JevClient` docs for
the full list.

## Non-goals (0.1.0)

- Single-shot requests: no streaming, batching, or multi-model orchestration.
- No criteria editor on the IR surface (derive-only, see above).
- No retry-after/jitter-aware backoff.
- No schema migration tooling between artifact versions.

## Workspace layout

- `jev-driver/` - runtime crate: wire types, validation, client, schema IR
- `jev-driver-macros/` - the five derive macros (`JevChoice`, `JevNoul`,
  `JevScore`, `JevState`, `JevQuestions`) plus `JevInstructions`
- `testapp/` - ticket-triage acceptance harness; the executable spec for the
  public API

## Testing and the live smoke

```sh
make check   # fmt + clippy + tests (offline)
make red     # testapp acceptance harness (offline, recorded fixtures)
make live    # live round-trip against api.typesafe.ai
```

Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`); `make check` is
the gate before any commit. `CLAUDE.md` records the agent-facing conventions.

`make live` runs the triage demo against the real API. It needs
`TYPESAFE_API_KEY` in the environment or a gitignored `.env` at the workspace
root (`chmod 600`). Expect typed answers in roughly 70-500ms and a usage line
with input/output token counts. Failure signatures: a 401 means the key;
logged backoff retries mean rate limiting (429) or overload (529). The run
succeeds if a retry lands.

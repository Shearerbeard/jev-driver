//! # jev-driver
//!
//! A typed Rust driver for TypeSafe AI's Jev (System One) decision model.
//!
//! Two authoring surfaces, one canonical, serializable schema IR
//! (the DMMF analog):
//!
//! * **Derive surface** — declare questions as Rust types:
//!   `JevChoice`/`JevScore`/`JevNoul` on enums/structs, `JevQuestions` on
//!   a question-set struct (compile-time paired with a `JevState` state
//!   type), `JevInstructions` for struct-to-string question contracts.
//! * **IR surface** — author or load `DecisionSchema` JSON artifacts,
//!   compose dynamic queries, and bridge back into derived types via
//!   `Answers::parse_as`.
//!
//! Strict by default: unknown option keys, unasked answers, missing
//! criteria, and unnormalized distributions are all errors.

pub mod answer;
pub mod client;
pub mod error;
pub mod prelude;
pub mod question;
pub mod refs;
pub mod schema;
pub mod wire;

#[cfg(feature = "test-support")]
pub mod fake;

pub use error::{JevError, JevResult};

pub use jev_driver_macros::{
    JevChoice, JevInstructions, JevNoul, JevQuestions, JevScore, JevState,
};

// Root re-exports: derive-generated code addresses these via `::jev_driver::`
// and must not depend on the user's imports.
pub use answer::{Answer, Answers, ChoiceDecision, FromAnswers, NoulDecision, ScoreDecision};
pub use question::{
    ChoiceOptions, QuestionKind, QuestionSet, QuestionType, RequestBuilder, ScoreLevels,
};
pub use refs::{StateKeys, validate_state_refs};
pub use schema::{CriteriaSource, DecisionSchema, QuestionSpec, StateSpec};
pub use wire::{Instructions, NoulCriteria, WireQuestion};

pub use serde_json::Value as JsonValue;

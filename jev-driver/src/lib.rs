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

// Root re-exports: derive-generated code addresses these through the
// resolved crate path (`crate` inside jev-driver's own targets, the
// consumer's dependency name otherwise) and must not depend on the
// user's imports.
pub use answer::{Answer, Answers, ChoiceDecision, FromAnswers, NoulDecision, ScoreDecision};
pub use question::{
    ChoiceOptions, QuestionKind, QuestionSet, QuestionType, RequestBuilder, ScoreLevels,
};
pub use refs::{StateKeys, validate_state_refs};
pub use schema::{CriteriaSource, DecisionSchema, QuestionSpec, StateSpec};
pub use wire::{Instructions, NoulCriteria, WireQuestion};

pub use serde_json::Value as JsonValue;

/// Macro-addressed surface: the derives resolve this crate by the
/// consumer's dependency name, so the items they call live at one
/// hidden, stable path. Not public API; ignore it.
#[doc(hidden)]
pub mod __private {
    use std::collections::BTreeMap;

    use crate::error::{JevError, JevResult};
    use crate::wire::Instructions;
    use crate::wire::WireQuestion;

    pub use crate::refs::validate_state_refs;

    /// Inserts a composed question into the decision's map, rejecting
    /// duplicate ids without mutating the map on rejection. Shared by
    /// `Decision::with_question`, `DecisionBuilder::with_question`,
    /// and the `JevQuestions` composition the derive generates.
    pub fn insert_question(
        questions: &mut BTreeMap<String, WireQuestion>,
        id: &str,
        question: WireQuestion,
    ) -> JevResult<()> {
        use std::collections::btree_map::Entry;

        match questions.entry(id.to_owned()) {
            Entry::Vacant(slot) => {
                slot.insert(question);
                Ok(())
            }
            Entry::Occupied(_) => Err(JevError::Schema(format!("duplicate question id `{id}`"))),
        }
    }

    /// Helper used by the score derive's `compose_wire` when an editor
    /// is given: validates the editor carries exactly `levels` entries.
    pub fn score_editor_levels(id: &str, levels: u8, editor: &[Instructions]) -> JevResult<()> {
        if editor.len() != levels as usize {
            return Err(JevError::Schema(format!(
                "score `{id}` needs {levels} rubric levels in its criteria editor, got {}",
                editor.len()
            )));
        }
        for (index, entry) in editor.iter().enumerate() {
            if !entry.is_non_empty() {
                return Err(JevError::Schema(format!(
                    "score `{id}` criteria editor level {index} is empty"
                )));
            }
        }
        Ok(())
    }
}

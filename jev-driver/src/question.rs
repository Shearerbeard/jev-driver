//! Question composition: the `QuestionType`/`QuestionSet` contracts the
//! derives implement, the [`Decision`] value both layers produce, and the
//! runtime builders for fully-dynamic questions.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use serde::Serialize;
use serde_json::Value;

use crate::error::{JevError, JevResult};
use crate::schema::{DecisionSchema, QuestionSpec};
use crate::wire::{Instructions, WireQuestion};

/// The three System One primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionKind {
    /// Pick one option from a set.
    Choice,
    /// Rate against an ordered rubric.
    Score,
    /// Estimate P(yes).
    Noul,
}

/// Implemented by enums deriving `JevChoice`: option keys both ways.
pub trait ChoiceOptions: Sized {
    /// Wire keys for every variant, in declaration order.
    const OPTIONS: &'static [&'static str];

    /// Parses a wire option key into the variant (strict).
    fn from_option(option: &str) -> JevResult<Self>;

    /// This variant's wire key.
    fn option_name(&self) -> &'static str;
}

/// Implemented by enums deriving `JevScore`: ordered rubric levels.
pub trait ScoreLevels: Sized {
    /// This question's wire id (for error context).
    const ID: &'static str;

    /// Number of rubric levels (declaration order = level order).
    const LEVELS: u8;

    /// The variant at rubric level `level`, if in range.
    fn from_level(level: u8) -> Option<Self>;
}

/// The contract every derived question type fulfills. Hand-implementable
/// (like `Serialize`) for types from other crates.
pub trait QuestionType: Sized {
    /// Which primitive this question is.
    const KIND: QuestionKind;

    /// The typed decision this question parses into.
    type Decision;

    /// The criteria editor type (`()` when criteria are static).
    type CriteriaEditor;

    /// This question's wire id.
    fn question_id() -> &'static str;

    /// The static IR fragment for this question.
    fn spec() -> QuestionSpec;

    /// Composes the wire question, applying a runtime criteria editor if
    /// one was supplied. Default: lower the static spec (fails loudly for
    /// runtime-criteria questions when no editor was given).
    fn compose_wire(editor: Option<&Self::CriteriaEditor>) -> JevResult<WireQuestion> {
        let _ = editor;
        Self::spec().into_wire(Self::question_id())
    }

    /// Parses one validated answer into the typed decision.
    fn parse(id: &str, answer: &crate::answer::Answer) -> JevResult<Self::Decision>;
}

/// Implemented by structs deriving `JevQuestions`: the full question set.
pub trait QuestionSet {
    /// The state type this set is compile-time paired with.
    type State: Serialize;

    /// The generated criteria-editor container.
    type Customization: Default;

    /// The canonical schema IR for the whole set.
    fn schema() -> DecisionSchema;

    /// Composes every question's wire form from the customization.
    fn compose(customization: &Self::Customization) -> JevResult<BTreeMap<String, WireQuestion>>;
}

/// A complete evaluation: state + questions. What the client sends.
#[derive(Debug, Clone)]
pub struct Decision {
    state: Value,
    questions: BTreeMap<String, WireQuestion>,
}

impl Decision {
    /// Starts a dynamic decision builder.
    #[must_use]
    pub fn builder() -> DecisionBuilder {
        DecisionBuilder::default()
    }

    pub(crate) fn from_parts(state: Value, questions: BTreeMap<String, WireQuestion>) -> Self {
        Self { state, questions }
    }

    /// The serialized state payload.
    #[must_use]
    pub fn state(&self) -> &Value {
        &self.state
    }

    /// The composed wire questions.
    #[must_use]
    pub fn questions(&self) -> &BTreeMap<String, WireQuestion> {
        &self.questions
    }

    /// Adds a dynamically-built question (e.g. from [`DynChoice`]).
    pub fn with_question(mut self, built: BuiltQuestion) -> JevResult<Self> {
        if self.questions.contains_key(&built.id) {
            return Err(JevError::Schema(format!(
                "duplicate question id `{}`",
                built.id
            )));
        }
        self.questions.insert(built.id, built.question);
        Ok(self)
    }
}

/// Dynamic decision composition without any derived types.
#[derive(Debug, Default)]
pub struct DecisionBuilder {
    state: Option<Value>,
    questions: BTreeMap<String, WireQuestion>,
}

impl DecisionBuilder {
    /// Sets the state payload (any serializable value).
    pub fn state(mut self, state: impl Serialize) -> JevResult<Self> {
        self.state = Some(serde_json::to_value(state)?);
        Ok(self)
    }

    /// Adds a built question.
    pub fn with_question(mut self, built: BuiltQuestion) -> JevResult<Self> {
        if self.questions.contains_key(&built.id) {
            return Err(JevError::Schema(format!(
                "duplicate question id `{}`",
                built.id
            )));
        }
        self.questions.insert(built.id, built.question);
        Ok(self)
    }

    /// Finishes the decision.
    pub fn build(self) -> JevResult<Decision> {
        let state = self.state.ok_or(JevError::MissingState)?;
        if self.questions.is_empty() {
            return Err(JevError::Schema(
                "a decision must carry at least one question".to_owned(),
            ));
        }
        Ok(Decision {
            state,
            questions: self.questions,
        })
    }
}

/// A question built at runtime, ready to add to a [`Decision`].
#[derive(Debug, Clone)]
pub struct BuiltQuestion {
    /// The question id.
    pub id: String,
    /// The composed wire question.
    pub question: WireQuestion,
}

/// Runtime-built choice question: the option set is supplied at build
/// time and answers are strictly validated against exactly that set.
#[derive(Debug, Clone)]
pub struct DynChoice {
    id: String,
    instructions: Instructions,
    options: BTreeMap<String, Option<Instructions>>,
}

impl DynChoice {
    /// Starts a dynamic choice.
    pub fn new(id: &str, instructions: impl Into<Instructions>) -> Self {
        Self {
            id: id.to_owned(),
            instructions: instructions.into(),
            options: BTreeMap::new(),
        }
    }

    /// Adds an option with a rubric description.
    #[must_use]
    pub fn option(mut self, name: &str, criteria: Instructions) -> Self {
        self.options.insert(name.to_owned(), Some(criteria));
        self
    }

    /// Adds an option that needs no extra description (wire `null`).
    #[must_use]
    pub fn option_null(mut self, name: &str) -> Self {
        self.options.insert(name.to_owned(), None);
        self
    }

    /// Validates cardinality and produces the wire question.
    pub fn build(self) -> JevResult<BuiltQuestion> {
        if self.id.trim().is_empty() {
            return Err(JevError::Schema(
                "dynamic choice id must be non-empty".to_owned(),
            ));
        }
        if !self.instructions.is_non_empty() {
            return Err(JevError::Schema(format!(
                "dynamic choice `{}` has empty instructions",
                self.id
            )));
        }
        if self.options.is_empty() {
            return Err(JevError::Schema(format!(
                "dynamic choice `{}` needs at least one option",
                self.id
            )));
        }
        if self.options.len() > 255 {
            return Err(JevError::Schema(format!(
                "dynamic choice `{}` has {} options; max is 255",
                self.id,
                self.options.len()
            )));
        }
        Ok(BuiltQuestion {
            id: self.id,
            question: WireQuestion::Choice {
                instructions: self.instructions,
                criteria: self.options,
            },
        })
    }
}

/// Typed request builder for derived question sets. `state()` only
/// accepts the paired state type — the wrong shape is a compile error.
pub struct RequestBuilder<Q: QuestionSet> {
    state: Option<Value>,
    customization: Q::Customization,
    _marker: PhantomData<fn() -> Q>,
}

impl<Q: QuestionSet> RequestBuilder<Q> {
    /// Creates the builder with default (empty) customization.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: None,
            customization: Q::Customization::default(),
            _marker: PhantomData,
        }
    }

    /// Sets the state payload; only the paired `Q::State` compiles here.
    pub fn state(mut self, state: &Q::State) -> JevResult<Self> {
        self.state = Some(serde_json::to_value(state)?);
        Ok(self)
    }

    /// Applies runtime criteria via the generated editor.
    #[must_use]
    pub fn customize(mut self, f: impl FnOnce(&mut Q::Customization)) -> Self {
        f(&mut self.customization);
        self
    }

    /// Composes the decision.
    pub fn build(self) -> JevResult<Decision> {
        let state = self.state.ok_or(JevError::MissingState)?;
        let questions = Q::compose(&self.customization)?;
        Ok(Decision { state, questions })
    }
}

impl<Q: QuestionSet> Default for RequestBuilder<Q> {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper used by the score derive's `compose_wire` when an editor is
/// given: validates the editor carries exactly `levels` entries.
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

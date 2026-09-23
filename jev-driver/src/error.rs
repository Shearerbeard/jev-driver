//! Error type for jev-driver. Errors-first: everything else depends on this.

use thiserror::Error;

/// The single error type for every fallible jev-driver operation.
#[derive(Debug, Error)]
pub enum JevError {
    /// HTTP transport failure (connect, timeout, malformed status line).
    #[error("transport failure: {0}")]
    Transport(String),

    /// `TYPESAFE_API_KEY` is not set or the server rejected it (401).
    #[error("missing or invalid TYPESAFE_API_KEY")]
    Unauthorized,

    /// The API rejected the request body (422): invalid question shape.
    #[error("request rejected (422): {0}")]
    InvalidRequest(String),

    /// Rate limited (429) and retries are exhausted.
    #[error("rate limited (429) after {retries} retries")]
    RateLimited {
        /// Retry attempts made before giving up.
        retries: u32,
    },

    /// Service overloaded (529) and retries are exhausted.
    #[error("service overloaded (529) after {retries} retries")]
    Overloaded {
        /// Retry attempts made before giving up.
        retries: u32,
    },

    /// An answer arrived for a question that was not asked.
    #[error("unknown question id `{id}`")]
    UnknownQuestion {
        /// The offending question id.
        id: String,
    },

    /// An option key that the caller's type did not declare.
    #[error("unknown option `{option}` for question `{id}`")]
    UnknownOption {
        /// The offending option key.
        option: String,
        /// The question it arrived under.
        id: String,
    },

    /// A asked question received no answer.
    #[error("missing answer for question `{id}`")]
    MissingAnswer {
        /// The question that went unanswered.
        id: String,
    },

    /// The answer's primitive type does not match the question's.
    #[error("answer type mismatch for `{id}`: expected {expected}, got {got}")]
    AnswerTypeMismatch {
        /// The question id.
        id: String,
        /// What the question declared.
        expected: &'static str,
        /// What the answer contained.
        got: &'static str,
    },

    /// Runtime-supplied criteria were never provided via `customize`.
    #[error(
        "missing criteria for question `{id}`: criteria are runtime-supplied but no editor value was given"
    )]
    MissingCriteria {
        /// The question left unfilled.
        id: String,
    },

    /// A decision was built without a state payload.
    #[error("state was not provided")]
    MissingState,

    /// An instruction backtick reference whose root segment is neither a
    /// serialized state key nor the question's own option/data key.
    #[error(
        "question `{question}` references `{reference}` but the serialized state has no such key"
    )]
    DanglingStateRef {
        /// The question whose instructions carry the reference.
        question: String,
        /// The dangling reference token.
        reference: String,
    },

    /// Schema invariant violation (bad IR, cardinality, editor shape).
    #[error("invalid schema: {0}")]
    Schema(String),

    /// A probability outside [0, 1].
    #[error("invalid probability {value}: outside [0, 1]")]
    InvalidProbability {
        /// The offending value.
        value: f64,
    },

    /// A probability distribution that does not sum to ~1.
    #[error("probabilities for `{id}` sum to {sum}, expected ~1")]
    NotNormalized {
        /// The question id.
        id: String,
        /// The observed sum.
        sum: f64,
    },

    /// An answer that is structurally well-typed JSON but semantically wrong.
    #[error("malformed answer for `{id}`: {detail}")]
    MalformedAnswer {
        /// The question id.
        id: String,
        /// What precisely was wrong.
        detail: String,
    },

    /// Serialization failure.
    #[error("serialization failure: {0}")]
    Json(#[from] serde_json::Error),

    /// The caller cancelled via `CancellationToken`.
    #[error("request cancelled")]
    Cancelled,

    /// Filesystem failure (schema load/save).
    #[error("io failure: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience alias used throughout the crate.
pub type JevResult<T> = Result<T, JevError>;

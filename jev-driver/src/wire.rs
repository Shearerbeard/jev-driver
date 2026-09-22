//! Wire types: the only module that knows the HTTP shape of TypeSafe's
//! `/v1/systemone` endpoint. Requests serialize strictly (we construct
//! them); response parsing is lenient about unknown fields (forward
//! tolerance) but strict about every field we consume.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{JevError, JevResult};

/// `instructions` / `criteria` payloads: the API accepts a string, a
/// structured object (data fields plus a `question` field), or an array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Instructions {
    /// Plain text form.
    Text(String),
    /// Structured form: backtick-referenceable data fields.
    Structured(BTreeMap<String, Value>),
    /// Array form.
    Parts(Vec<Value>),
}

impl Instructions {
    /// Build the text form.
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// True when the payload carries actual content (used by schema
    /// validation to reject blank instructions).
    pub fn is_non_empty(&self) -> bool {
        match self {
            Self::Text(s) => !s.trim().is_empty(),
            Self::Structured(m) => !m.is_empty(),
            Self::Parts(p) => !p.is_empty(),
        }
    }

    /// Struct-to-string contract: serialize `data` (which must be a map)
    /// and embed `question` alongside it, producing the API's structured
    /// instructions form. Used by the `JevInstructions` derive.
    pub fn from_question(question: &str, data: &impl Serialize) -> JevResult<Self> {
        let value = serde_json::to_value(data)?;
        let Some(map) = value.as_object().cloned() else {
            return Err(JevError::Schema(
                "JevInstructions types must serialize to a JSON object".to_owned(),
            ));
        };
        let mut out = BTreeMap::new();
        for (k, v) in map {
            if k == "question" {
                return Err(JevError::Schema(
                    "JevInstructions field named `question` collides with the question slot"
                        .to_owned(),
                ));
            }
            out.insert(k, v);
        }
        out.insert("question".to_owned(), Value::String(question.to_owned()));
        Ok(Self::Structured(out))
    }
}

impl From<&str> for Instructions {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<String> for Instructions {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

/// Noul criteria: descriptions of what yes and no mean. Serializes with
/// the API's `"true"` / `"false"` keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoulCriteria {
    /// What a yes (value near 1) means.
    #[serde(rename = "true")]
    pub yes: Instructions,
    /// What a no (value near 0) means.
    #[serde(rename = "false")]
    pub no: Instructions,
}

/// A question exactly as it appears on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WireQuestion {
    /// Pick one option from `criteria` (max 255 options).
    Choice {
        /// What the model should decide.
        instructions: Instructions,
        /// Option key to rubric description; `None` means "no extra detail".
        criteria: BTreeMap<String, Option<Instructions>>,
    },
    /// Rate the state against an ordered rubric (2..=10 levels).
    Score {
        /// What the model should rate.
        instructions: Instructions,
        /// Ordered level descriptions.
        criteria: Vec<Instructions>,
    },
    /// Estimate P(yes) for a statement.
    Noul {
        /// The yes/no question.
        instructions: Instructions,
        /// Optional pole descriptions.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
}

impl WireQuestion {
    /// The primitive's wire name.
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
            Self::Noul { .. } => "noul",
        }
    }

    /// Choice option keys, if this is a choice question.
    pub fn choice_options(&self) -> Option<&BTreeMap<String, Option<Instructions>>> {
        match self {
            Self::Choice { criteria, .. } => Some(criteria),
            Self::Score { .. } | Self::Noul { .. } => None,
        }
    }
}

/// One answer exactly as it arrives from the wire.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WireAnswer {
    /// P(yes) in [0, 1].
    Noul {
        /// The yes/no probability.
        noul: f64,
    },
    /// The chosen option plus the full distribution.
    Choice {
        /// The highest-probability option.
        choice: String,
        /// Every option mapped to its probability.
        probabilities: BTreeMap<String, f64>,
        /// Certainty derived from the distribution.
        confidence: f64,
    },
    /// Probability-weighted level (may land between levels).
    Score {
        /// The weighted score.
        score: f64,
        /// Level number back to description.
        legend: BTreeMap<String, String>,
        /// Each level to its probability.
        probabilities: BTreeMap<String, f64>,
        /// Certainty derived from the distribution.
        confidence: f64,
    },
}

impl WireAnswer {
    /// The primitive's wire name.
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
        }
    }
}

/// Token usage returned with every response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct WireUsage {
    /// Billable input tokens.
    pub input_tokens: u64,
    /// Output tokens (currently free).
    pub output_tokens: u64,
}

/// The full response body.
#[derive(Debug, Clone, Deserialize)]
pub struct WireResponse {
    /// The model that performed the evaluation.
    pub model: String,
    /// One answer per asked question, under the caller's ids.
    pub answers: BTreeMap<String, WireAnswer>,
    /// Token accounting.
    pub usage: WireUsage,
}

/// The request body assembled by the client at send time.
#[derive(Debug, Serialize)]
pub struct WireRequestBody<'a> {
    /// The state payload.
    pub state: &'a Value,
    /// Pinned model id (e.g. `jev-1.13.0`).
    pub model: &'a str,
    /// The question set.
    pub questions: &'a BTreeMap<String, WireQuestion>,
}

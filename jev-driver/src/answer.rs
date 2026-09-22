//! Answers: validation-on-parse (dynamic layer) and the typed decision
//! wrappers the derive layer produces. Strict by default — unknown keys,
//! missing options, and unnormalized distributions are errors.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use crate::error::{JevError, JevResult};
use crate::question::{ChoiceOptions, Decision, ScoreLevels};
use crate::wire::{WireAnswer, WireQuestion, WireResponse};

/// Tolerance for probability-distribution sums (float rounding headroom).
pub const SUM_TOLERANCE: f64 = 0.05;

/// A probability in [0, 1]. Constructed only through validation.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Probability(f64);

impl Probability {
    /// Zero, the safe fallback for structurally-impossible lookups.
    pub const ZERO: Self = Self(0.0);

    /// Validates and wraps.
    pub fn new(value: f64) -> JevResult<Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(JevError::InvalidProbability { value })
        }
    }

    /// The wrapped value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// Gate: is P at or above `threshold`?
    #[must_use]
    pub const fn yes(self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

/// Model-reported confidence in [0, 1], derived from the distribution.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Confidence(f64);

impl Confidence {
    /// Validates and wraps.
    pub fn new(value: f64) -> JevResult<Self> {
        Probability::new(value).map(|p| Self(p.get()))
    }

    /// The wrapped value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// Gate: act automatically at or above `threshold`?
    #[must_use]
    pub const fn act(self, threshold: f64) -> bool {
        self.0 >= threshold
    }
}

fn checked_prob(id: &str, value: f64) -> JevResult<Probability> {
    Probability::new(value).map_err(|_| JevError::MalformedAnswer {
        id: id.to_owned(),
        detail: format!("probability {value} outside [0, 1]"),
    })
}

fn checked_conf(id: &str, value: f64) -> JevResult<Confidence> {
    Confidence::new(value).map_err(|_| JevError::MalformedAnswer {
        id: id.to_owned(),
        detail: format!("confidence {value} outside [0, 1]"),
    })
}

/// Dynamic choice answer: option keys as strings, validated against the
/// option set the request actually sent. Built only by validation; read
/// through the accessors so the checked invariants survive.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceData {
    /// The selected option key.
    selected: String,
    /// Every sent option to its probability.
    probabilities: BTreeMap<String, Probability>,
    /// Derived certainty.
    confidence: Confidence,
}

impl ChoiceData {
    /// The selected option key.
    #[must_use]
    pub fn selected(&self) -> &str {
        &self.selected
    }

    /// Every sent option mapped to its probability.
    #[must_use]
    pub fn probabilities(&self) -> &BTreeMap<String, Probability> {
        &self.probabilities
    }

    /// Derived certainty.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }
}

/// Dynamic score answer. Built only by validation; read through the
/// accessors.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreData {
    /// Probability-weighted level; may land between levels.
    score: f64,
    /// Level number back to description.
    legend: BTreeMap<String, String>,
    /// Level number to probability.
    probabilities: BTreeMap<String, Probability>,
    /// Derived certainty.
    confidence: Confidence,
}

impl ScoreData {
    /// The probability-weighted level; may land between levels.
    #[must_use]
    pub const fn score(&self) -> f64 {
        self.score
    }

    /// Level number back to description.
    #[must_use]
    pub fn legend(&self) -> &BTreeMap<String, String> {
        &self.legend
    }

    /// Each level mapped to its probability.
    #[must_use]
    pub fn probabilities(&self) -> &BTreeMap<String, Probability> {
        &self.probabilities
    }

    /// Derived certainty.
    #[must_use]
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }
}

/// A parsed, validated answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Choice answer.
    Choice(ChoiceData),
    /// Score answer.
    Score(ScoreData),
    /// Noul answer.
    Noul(Probability),
}

impl Answer {
    /// The primitive's wire name.
    #[must_use]
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Choice(_) => "choice",
            Self::Score(_) => "score",
            Self::Noul(_) => "noul",
        }
    }
}

/// Token usage for a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Billable input tokens.
    pub input_tokens: u64,
    /// Output tokens (currently free).
    pub output_tokens: u64,
}

/// The parsed response: validated against the decision that produced it.
/// The answer map is private: values enter only through
/// [`Answers::from_wire`], so the strictness contract is a property of
/// the type, not of any one construction path.
#[derive(Debug, Clone)]
pub struct Answers {
    /// The model that answered.
    pub model: String,
    /// Validated answers, keyed by question id.
    answers: BTreeMap<String, Answer>,
    /// Token accounting.
    pub usage: Usage,
}

#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "the catch-all arms report the mismatching answer kind with a typed error"
)]
impl Answers {
    /// Parses a wire response and validates every answer against the
    /// questions that were actually asked (ids, primitive types, option
    /// membership, normalization).
    pub fn from_wire(response: WireResponse, decision: &Decision) -> JevResult<Self> {
        let WireResponse {
            model,
            answers: raw,
            usage,
        } = response;
        let mut parsed = BTreeMap::new();
        for (id, answer) in raw {
            let question = decision
                .questions()
                .get(&id)
                .ok_or_else(|| JevError::UnknownQuestion { id: id.clone() })?;
            let parsed_answer = parse_one(&id, question, answer)?;
            parsed.insert(id, parsed_answer);
        }
        for id in decision.questions().keys() {
            if !parsed.contains_key(id) {
                return Err(JevError::MissingAnswer { id: id.clone() });
            }
        }
        Ok(Self {
            model,
            answers: parsed,
            usage: Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            },
        })
    }

    /// Fetches one answer by id (strict: missing is an error).
    pub fn get(&self, id: &str) -> JevResult<&Answer> {
        self.answers
            .get(id)
            .ok_or_else(|| JevError::MissingAnswer { id: id.to_owned() })
    }

    /// Fetches a dynamic choice answer.
    pub fn choice(&self, id: &str) -> JevResult<&ChoiceData> {
        match self.get(id)? {
            Answer::Choice(data) => Ok(data),
            other => Err(JevError::AnswerTypeMismatch {
                id: id.to_owned(),
                expected: "choice",
                got: other.kind_str(),
            }),
        }
    }

    /// Fetches a dynamic score answer.
    pub fn score(&self, id: &str) -> JevResult<&ScoreData> {
        match self.get(id)? {
            Answer::Score(data) => Ok(data),
            other => Err(JevError::AnswerTypeMismatch {
                id: id.to_owned(),
                expected: "score",
                got: other.kind_str(),
            }),
        }
    }

    /// Fetches a noul probability.
    pub fn noul(&self, id: &str) -> JevResult<Probability> {
        match self.get(id)? {
            Answer::Noul(p) => Ok(*p),
            other => Err(JevError::AnswerTypeMismatch {
                id: id.to_owned(),
                expected: "noul",
                got: other.kind_str(),
            }),
        }
    }

    /// Bridge: re-parse these dynamic answers into a derived typed set,
    /// cross-validating ids and option keys against your enums. The same
    /// completeness invariant holds in both layers: every option/level of
    /// the asked question must carry a probability, zero-valued included.
    pub fn parse_as<T: FromAnswers>(&self) -> JevResult<T> {
        T::from_answers(self)
    }
}

fn parse_one(id: &str, question: &WireQuestion, answer: WireAnswer) -> JevResult<Answer> {
    match (question, answer) {
        (WireQuestion::Noul { .. }, WireAnswer::Noul { noul }) => {
            Ok(Answer::Noul(checked_prob(id, noul)?))
        }
        (
            WireQuestion::Choice { criteria, .. },
            WireAnswer::Choice {
                choice,
                probabilities,
                confidence,
            },
        ) => {
            if !criteria.contains_key(&choice) {
                return Err(JevError::UnknownOption {
                    option: choice,
                    id: id.to_owned(),
                });
            }
            for key in probabilities.keys() {
                if !criteria.contains_key(key) {
                    return Err(JevError::UnknownOption {
                        option: key.clone(),
                        id: id.to_owned(),
                    });
                }
            }
            for option in criteria.keys() {
                if !probabilities.contains_key(option) {
                    return Err(JevError::MalformedAnswer {
                        id: id.to_owned(),
                        detail: format!("missing probability for option `{option}`"),
                    });
                }
            }
            let sum: f64 = probabilities.values().sum();
            if (sum - 1.0).abs() > SUM_TOLERANCE {
                return Err(JevError::NotNormalized {
                    id: id.to_owned(),
                    sum,
                });
            }
            let mut checked = BTreeMap::new();
            for (key, value) in probabilities {
                checked.insert(key, checked_prob(id, value)?);
            }
            Ok(Answer::Choice(ChoiceData {
                selected: choice,
                probabilities: checked,
                confidence: checked_conf(id, confidence)?,
            }))
        }
        (
            WireQuestion::Score { criteria, .. },
            WireAnswer::Score {
                score,
                legend,
                probabilities,
                confidence,
            },
        ) => {
            if legend.len() != probabilities.len()
                || !legend.keys().all(|key| probabilities.contains_key(key))
            {
                return Err(JevError::MalformedAnswer {
                    id: id.to_owned(),
                    detail: "legend and probabilities keys disagree".to_owned(),
                });
            }
            for key in probabilities.keys() {
                let level = key
                    .parse::<usize>()
                    .map_err(|_| JevError::MalformedAnswer {
                        id: id.to_owned(),
                        detail: format!("non-numeric level key `{key}`"),
                    })?;
                if level >= criteria.len() {
                    return Err(JevError::MalformedAnswer {
                        id: id.to_owned(),
                        detail: format!(
                            "level key `{key}` outside the {}-level rubric",
                            criteria.len()
                        ),
                    });
                }
                if *key != level.to_string() {
                    return Err(JevError::MalformedAnswer {
                        id: id.to_owned(),
                        detail: format!("non-canonical level key `{key}` (expected `{level}`)"),
                    });
                }
            }
            for level in 0..criteria.len() {
                if !probabilities.contains_key(&level.to_string()) {
                    return Err(JevError::MalformedAnswer {
                        id: id.to_owned(),
                        detail: format!("missing probability for level {level}"),
                    });
                }
            }
            let top_level = u32::try_from(criteria.len().saturating_sub(1))
                .map(f64::from)
                .unwrap_or(f64::INFINITY);
            if !score.is_finite() || !(0.0..=top_level).contains(&score) {
                return Err(JevError::MalformedAnswer {
                    id: id.to_owned(),
                    detail: format!("score {score} outside the {}-level rubric", criteria.len()),
                });
            }
            let sum: f64 = probabilities.values().sum();
            if (sum - 1.0).abs() > SUM_TOLERANCE {
                return Err(JevError::NotNormalized {
                    id: id.to_owned(),
                    sum,
                });
            }
            let mut checked = BTreeMap::new();
            for (key, value) in probabilities {
                checked.insert(key, checked_prob(id, value)?);
            }
            Ok(Answer::Score(ScoreData {
                score,
                legend,
                probabilities: checked,
                confidence: checked_conf(id, confidence)?,
            }))
        }
        (question, answer) => Err(JevError::AnswerTypeMismatch {
            id: id.to_owned(),
            expected: question.kind_str(),
            got: answer.kind_str(),
        }),
    }
}

/// Implemented by generated answer sets (`TicketTriageAnswers`).
pub trait FromAnswers: Sized {
    /// Validates and converts the dynamic answers into the typed set.
    fn from_answers(answers: &Answers) -> JevResult<Self>;
}

/// Probability distribution keyed by a choice enum's variants.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbabilityMap<T> {
    inner: BTreeMap<String, Probability>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: ChoiceOptions> ProbabilityMap<T> {
    /// Probability of a specific variant (typed lookup).
    #[must_use]
    pub fn get(&self, option: &T) -> Probability {
        self.inner
            .get(option.option_name())
            .copied()
            .unwrap_or(Probability::ZERO)
    }

    /// Iterate (option key, probability) pairs in key order.
    pub fn iter_names(&self) -> impl Iterator<Item = (&str, Probability)> + '_ {
        self.inner.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

/// Typed choice decision: your enum, its distribution, its confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceDecision<T> {
    /// The selected variant.
    pub selected: T,
    /// Per-variant probabilities.
    pub probabilities: ProbabilityMap<T>,
    /// Derived certainty.
    pub confidence: Confidence,
}

impl<T: ChoiceOptions> ChoiceDecision<T> {
    /// Strict parse: unknown option keys and missing probabilities are
    /// errors, even though the dynamic layer already checked membership.
    pub fn parse(id: &str, answer: &Answer) -> JevResult<Self> {
        let Answer::Choice(data) = answer else {
            return Err(JevError::AnswerTypeMismatch {
                id: id.to_owned(),
                expected: "choice",
                got: answer.kind_str(),
            });
        };
        for key in data.probabilities.keys() {
            if !T::OPTIONS.contains(&key.as_str()) {
                return Err(JevError::UnknownOption {
                    option: key.clone(),
                    id: id.to_owned(),
                });
            }
        }
        for option in T::OPTIONS {
            if !data.probabilities.contains_key(*option) {
                return Err(JevError::MalformedAnswer {
                    id: id.to_owned(),
                    detail: format!("missing probability for option `{option}`"),
                });
            }
        }
        Ok(Self {
            selected: T::from_option(&data.selected)?,
            probabilities: ProbabilityMap {
                inner: data.probabilities.clone(),
                _marker: PhantomData,
            },
            confidence: data.confidence,
        })
    }
}

/// Typed score decision: the weighted score plus the level distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreDecision<T> {
    /// Probability-weighted level; may land between levels. Private: the
    /// finite-and-in-range invariant must survive mutation.
    score: f64,
    /// Level index to probability. Private: parse guarantees every level
    /// of the rubric is present, and that invariant must survive.
    probabilities: BTreeMap<u8, Probability>,
    /// Derived certainty.
    pub confidence: Confidence,
    _marker: PhantomData<fn() -> T>,
}

impl<T: ScoreLevels> ScoreDecision<T> {
    /// Strict parse: level keys must be numeric and inside the rubric.
    pub fn parse(id: &str, answer: &Answer) -> JevResult<Self> {
        let Answer::Score(data) = answer else {
            return Err(JevError::AnswerTypeMismatch {
                id: id.to_owned(),
                expected: "score",
                got: answer.kind_str(),
            });
        };
        let mut probabilities = BTreeMap::new();
        for (key, value) in &data.probabilities {
            let level: u8 = key.parse().map_err(|_| JevError::MalformedAnswer {
                id: id.to_owned(),
                detail: format!("non-numeric level key `{key}`"),
            })?;
            if level >= T::LEVELS {
                return Err(JevError::MalformedAnswer {
                    id: id.to_owned(),
                    detail: format!("level {level} outside the {}-level rubric", T::LEVELS),
                });
            }
            probabilities.insert(level, *value);
        }
        if probabilities.len() != data.probabilities.len() {
            return Err(JevError::MalformedAnswer {
                id: id.to_owned(),
                detail: "level keys alias the same level (e.g. `0` and `00`)".to_owned(),
            });
        }
        for level in 0..T::LEVELS {
            if !probabilities.contains_key(&level) {
                return Err(JevError::MalformedAnswer {
                    id: id.to_owned(),
                    detail: format!("missing probability for level {level}"),
                });
            }
        }
        Ok(Self {
            score: data.score,
            probabilities,
            confidence: data.confidence,
            _marker: PhantomData,
        })
    }

    /// The probability-weighted level; may land between levels.
    #[must_use]
    pub const fn score(&self) -> f64 {
        self.score
    }

    /// Probability of a specific rubric level (typed lookup). Parse
    /// guarantees every level is present, so a miss is a logic bug and
    /// reads as zero, mirroring [`ProbabilityMap::get`].
    #[must_use]
    pub fn level(&self, level: u8) -> Probability {
        self.probabilities
            .get(&level)
            .copied()
            .unwrap_or(Probability::ZERO)
    }

    /// The rubric level with the highest probability, as your enum.
    pub fn nearest(&self) -> JevResult<T> {
        let mut best: u8 = 0;
        let mut best_p = Probability::ZERO;
        for level in 0..T::LEVELS {
            let p = self.level(level);
            if p.get() > best_p.get() {
                best = level;
                best_p = p;
            }
        }
        T::from_level(best).ok_or_else(|| JevError::MalformedAnswer {
            id: T::ID.to_owned(),
            detail: format!("rubric level {best} has no matching variant"),
        })
    }
}

/// Typed noul decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoulDecision {
    /// P(yes).
    pub probability: Probability,
}

impl NoulDecision {
    /// Parse: only shape-checks; range was validated at construction.
    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "the catch-all arm reports the mismatching answer kind with a typed error"
    )]
    pub fn parse(id: &str, answer: &Answer) -> JevResult<Self> {
        match answer {
            Answer::Noul(p) => Ok(Self { probability: *p }),
            other => Err(JevError::AnswerTypeMismatch {
                id: id.to_owned(),
                expected: "noul",
                got: other.kind_str(),
            }),
        }
    }
}

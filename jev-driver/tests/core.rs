//! Core-library tests: instructions forms, newtypes, schema invariants,
//! and spec-to-wire lowering.

use std::collections::BTreeMap;

use jev_driver::prelude::*;
use serde_json::json;

#[test]
fn instructions_round_trip_all_three_forms() -> Result<(), Box<dyn std::error::Error>> {
    let text: Instructions = serde_json::from_str("\"plain\"")?;
    assert_eq!(
        text,
        Instructions::text("plain"),
        "text form must round-trip"
    );

    let structured: Instructions = serde_json::from_value(json!({ "a": 1, "b": [true] }))?;
    assert!(
        matches!(structured, Instructions::Structured(_)),
        "object form parses as structured"
    );
    assert!(structured.is_non_empty(), "structured form is non-empty");

    let parts: Instructions = serde_json::from_value(json!(["x", 2]))?;
    assert!(
        matches!(parts, Instructions::Parts(_)),
        "array form parses as parts"
    );
    Ok(())
}

#[test]
fn from_question_embeds_data_and_rejects_non_maps() -> Result<(), Box<dyn std::error::Error>> {
    let instructions = Instructions::from_question("Same as `who`?", &json!({ "who": "ada" }))?;
    let Instructions::Structured(map) = &instructions else {
        panic!("from_question must produce the structured form");
    };
    assert_eq!(
        map["question"],
        json!("Same as `who`?"),
        "question text is embedded"
    );
    assert_eq!(map["who"], json!("ada"), "data fields ride along");

    let rejected = Instructions::from_question("q", &json!([1, 2]));
    assert!(
        matches!(rejected, Err(JevError::Schema(_))),
        "non-map payloads must be rejected"
    );
    Ok(())
}

#[test]
fn probability_and_confidence_reject_out_of_range() -> Result<(), Box<dyn std::error::Error>> {
    assert!(Probability::new(0.0).is_ok(), "0.0 is a valid probability");
    assert!(Probability::new(1.0).is_ok(), "1.0 is a valid probability");
    for bad in [1.5, -0.1, f64::NAN] {
        assert!(Probability::new(bad).is_err(), "{bad} must be rejected");
    }
    assert!(Confidence::new(0.42).is_ok(), "0.42 is valid confidence");
    assert!(Confidence::new(1.4).is_err(), "1.4 must be rejected");
    assert!(Probability::new(0.75)?.yes(0.5), "0.75 clears a 0.5 gate");
    assert!(
        Confidence::new(0.75)?.act(0.9) == false,
        "0.75 does not clear a 0.9 gate"
    );
    Ok(())
}

#[test]
fn schema_rejects_empty_question_sets() -> Result<(), Box<dyn std::error::Error>> {
    let schema = DecisionSchema::new(None);
    let err = schema
        .validate()
        .expect_err("empty schemas must not validate");
    assert!(
        matches!(err, JevError::Schema(_)),
        "expected a Schema error, got {err:?}"
    );
    Ok(())
}

#[test]
fn schema_rejects_unknown_versions() -> Result<(), Box<dyn std::error::Error>> {
    let json = json!({ "jev": 99, "questions": {} });
    let schema: DecisionSchema = serde_json::from_value(json)?;
    let err = schema
        .validate()
        .expect_err("unknown IR versions must not validate");
    let JevError::Schema(detail) = err else {
        panic!("expected a Schema error, got {err:?}");
    };
    assert!(
        detail.contains("99"),
        "error names the bad version: {detail}"
    );
    Ok(())
}

#[test]
fn schema_rejects_runtime_marker_with_static_criteria() -> Result<(), Box<dyn std::error::Error>> {
    let spec = QuestionSpec::Choice {
        instructions: Instructions::text("pick"),
        criteria: BTreeMap::from([("a".to_owned(), Some(Instructions::text("A")))]),
        criteria_source: CriteriaSource::Runtime,
    };
    let schema = DecisionSchema::new(None).with_question("pick", spec);
    let err = schema
        .validate()
        .expect_err("runtime marker plus static criteria is contradictory");
    assert!(
        matches!(err, JevError::Schema(_)),
        "expected a Schema error, got {err:?}"
    );
    Ok(())
}

#[test]
fn schema_rejects_wrong_score_level_counts() -> Result<(), Box<dyn std::error::Error>> {
    let one_level = QuestionSpec::Score {
        instructions: Instructions::text("rate"),
        criteria: vec![Instructions::text("only")],
        criteria_source: CriteriaSource::Static,
    };
    let schema = DecisionSchema::new(None).with_question("rate", one_level);
    assert!(
        schema.validate().is_err(),
        "a one-level rubric must not validate"
    );

    let eleven_levels = QuestionSpec::Score {
        instructions: Instructions::text("rate"),
        criteria: (0..11)
            .map(|i| Instructions::text(format!("level {i}")))
            .collect(),
        criteria_source: CriteriaSource::Static,
    };
    let schema = DecisionSchema::new(None).with_question("rate", eleven_levels);
    assert!(
        schema.validate().is_err(),
        "an eleven-level rubric must not validate"
    );
    Ok(())
}

#[test]
fn runtime_spec_lowering_fails_loudly_without_editor() -> Result<(), Box<dyn std::error::Error>> {
    let spec = QuestionSpec::Choice {
        instructions: Instructions::text("pick"),
        criteria: BTreeMap::new(),
        criteria_source: CriteriaSource::Runtime,
    };
    let err = spec
        .into_wire("pick")
        .expect_err("lowering a runtime spec without an editor must fail");
    match err {
        JevError::MissingCriteria { id } => assert_eq!(id, "pick", "error names the question"),
        other => panic!("expected MissingCriteria, got {other:?}"),
    }
    Ok(())
}

#[test]
fn dynamic_schema_queries_compose_decisions() -> Result<(), Box<dyn std::error::Error>> {
    let schema = DecisionSchema::new(None)
        .with_question(
            "ok",
            QuestionSpec::Noul {
                instructions: Instructions::text("is it ok"),
                criteria: None,
            },
        )
        .with_question(
            "kind",
            QuestionSpec::Choice {
                instructions: Instructions::text("which kind"),
                criteria: BTreeMap::from([
                    ("a".to_owned(), Some(Instructions::text("A"))),
                    ("b".to_owned(), None),
                ]),
                criteria_source: CriteriaSource::Static,
            },
        );
    schema.validate()?;

    let decision = schema
        .query()
        .state(json!({ "s": 1 }))?
        .ask("kind")?
        .build()?;

    assert_eq!(
        decision.questions().len(),
        1,
        "only the asked question is sent"
    );
    assert!(
        decision.questions().contains_key("kind"),
        "the asked id is present"
    );

    let ghost = schema.query().ask("missing");
    assert!(
        matches!(ghost, Err(JevError::UnknownQuestion { .. })),
        "asking an id that is not in the schema fails immediately"
    );
    Ok(())
}

fn choice_decision() -> Decision {
    DynChoice::new("kind", Instructions::text("which kind"))
        .option("a", Instructions::text("A"))
        .option("b", Instructions::text("B"))
        .build()
        .and_then(|q| {
            Decision::builder()
                .state(json!({ "s": 1 }))
                .and_then(|builder| builder.with_question(q))
                .and_then(DecisionBuilder::build)
        })
        .unwrap_or_else(|e| panic!("decision must build: {e}"))
}

#[test]
fn missing_zero_probability_entry_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let response: WireResponse = serde_json::from_value(json!({
        "model": "jev-1.13.0",
        "answers": {
            "kind": {
                "type": "choice",
                "choice": "a",
                "probabilities": { "a": 1.0 },
                "confidence": 0.9
            }
        },
        "usage": { "input_tokens": 10, "output_tokens": 2 }
    }))?;
    let err = Answers::from_wire(response, &choice_decision())
        .expect_err("omitting a zero-probability option must fail even though the sum is 1");
    match err {
        JevError::MalformedAnswer { id, detail } => {
            assert_eq!(id, "kind", "error names the question");
            assert!(
                detail.contains("`b`"),
                "error names the missing option: {detail}"
            );
        }
        other => panic!("expected MalformedAnswer, got {other:?}"),
    }
    Ok(())
}

#[test]
fn missing_zero_probability_level_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let question = QuestionSpec::Score {
        instructions: Instructions::text("rate"),
        criteria: vec![Instructions::text("low"), Instructions::text("high")],
        criteria_source: CriteriaSource::Static,
    };
    let decision = DecisionSchema::new(None)
        .with_question("rate", question)
        .query()
        .state(json!({ "s": 1 }))?
        .ask("rate")?
        .build()?;

    let response: WireResponse = serde_json::from_value(json!({
        "model": "jev-1.13.0",
        "answers": {
            "rate": {
                "type": "score",
                "score": 0.0,
                "legend": { "0": "low" },
                "probabilities": { "0": 1.0 },
                "confidence": 0.9
            }
        },
        "usage": { "input_tokens": 10, "output_tokens": 2 }
    }))?;
    let err = Answers::from_wire(response, &decision)
        .expect_err("omitting a zero-probability rubric level must fail even though the sum is 1");
    match err {
        JevError::MalformedAnswer { id, detail } => {
            assert_eq!(id, "rate", "error names the question");
            assert!(
                detail.contains("level 1"),
                "error names the missing level: {detail}"
            );
        }
        other => panic!("expected MalformedAnswer, got {other:?}"),
    }
    Ok(())
}

#[test]
fn out_of_range_score_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let question = QuestionSpec::Score {
        instructions: Instructions::text("rate"),
        criteria: vec![Instructions::text("low"), Instructions::text("high")],
        criteria_source: CriteriaSource::Static,
    };
    let decision = DecisionSchema::new(None)
        .with_question("rate", question)
        .query()
        .state(json!({ "s": 1 }))?
        .ask("rate")?
        .build()?;

    let response: WireResponse = serde_json::from_value(json!({
        "model": "jev-1.13.0",
        "answers": {
            "rate": {
                "type": "score",
                "score": 17.0,
                "legend": { "0": "low", "1": "high" },
                "probabilities": { "0": 0.5, "1": 0.5 },
                "confidence": 0.5
            }
        },
        "usage": { "input_tokens": 10, "output_tokens": 2 }
    }))?;
    let err = Answers::from_wire(response, &decision)
        .expect_err("a score of 17 on a 2-level rubric must fail");
    assert!(
        matches!(err, JevError::MalformedAnswer { .. }),
        "expected MalformedAnswer, got {err:?}"
    );
    Ok(())
}

#[test]
fn duplicate_question_ids_fail_at_compose() -> Result<(), Box<dyn std::error::Error>> {
    use jev_driver::{JevNoul, JevQuestions};

    #[derive(JevNoul)]
    #[jev(id = "clash", instructions = "first")]
    struct First;

    #[derive(JevNoul)]
    #[jev(id = "clash", instructions = "second")]
    struct Second;

    #[derive(JevQuestions)]
    #[allow(dead_code, reason = "compose reads the field types, not the values")]
    struct Set {
        first: First,
        second: Second,
    }

    let err = <Set as QuestionSet>::compose(&SetCustomization::default())
        .expect_err("two field types sharing a question id must fail at compose");
    match err {
        JevError::Schema(detail) => {
            assert!(
                detail.contains("clash"),
                "error names the duplicated id: {detail}"
            );
        }
        other => panic!("expected Schema error, got {other:?}"),
    }
    Ok(())
}

#[test]
fn undersized_max_interval_never_panics() {
    let retry = RetryConfig {
        max_retries: 1,
        initial_interval: std::time::Duration::from_secs(5),
        max_interval: std::time::Duration::from_millis(1),
        multiplier: 2.0,
    };
    for attempt in 0..4 {
        let delay = retry.delay_for(attempt);
        assert!(
            delay <= std::time::Duration::from_millis(1),
            "delay must floor at max_interval, got {delay:?} at attempt {attempt}"
        );
    }
}

#[test]
fn nan_or_pathological_multiplier_never_panics() {
    let nan_retry = RetryConfig {
        multiplier: f64::NAN,
        ..Default::default()
    };
    let base = nan_retry.delay_for(0);
    for attempt in 0..4 {
        assert_eq!(
            nan_retry.delay_for(attempt),
            base,
            "NaN multiplier must sanitize to a constant interval"
        );
    }

    let overflow_retry = RetryConfig {
        initial_interval: std::time::Duration::ZERO,
        multiplier: f64::INFINITY,
        ..Default::default()
    };
    let delay = overflow_retry.delay_for(2);
    assert_eq!(
        delay,
        std::time::Duration::from_secs(30),
        "0 * inf must clamp to max_interval, got {delay:?}"
    );
}

#[test]
fn extreme_durations_saturate_instead_of_panicking() {
    let extreme = RetryConfig {
        initial_interval: std::time::Duration::MAX,
        max_interval: std::time::Duration::MAX,
        multiplier: 1.0,
        ..Default::default()
    };
    assert_eq!(
        extreme.delay_for(0),
        std::time::Duration::MAX,
        "Duration::MAX intervals must saturate, not panic"
    );
}

#[test]
fn aliasing_level_keys_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let question = QuestionSpec::Score {
        instructions: Instructions::text("rate"),
        criteria: vec![Instructions::text("low"), Instructions::text("high")],
        criteria_source: CriteriaSource::Static,
    };
    let decision = DecisionSchema::new(None)
        .with_question("rate", question)
        .query()
        .state(json!({ "s": 1 }))?
        .ask("rate")?
        .build()?;

    let response: WireResponse = serde_json::from_value(json!({
        "model": "jev-1.13.0",
        "answers": {
            "rate": {
                "type": "score",
                "score": 0.5,
                "legend": { "0": "low", "00": "low", "1": "high" },
                "probabilities": { "0": 0.2, "00": 0.3, "1": 0.5 },
                "confidence": 0.7
            }
        },
        "usage": { "input_tokens": 10, "output_tokens": 2 }
    }))?;
    let err = Answers::from_wire(response, &decision)
        .expect_err("level keys aliasing the same level must fail even though the sum is 1");
    match err {
        JevError::MalformedAnswer { id, detail } => {
            assert_eq!(id, "rate", "error names the question");
            assert!(
                detail.contains("non-canonical"),
                "error names the aliasing key: {detail}"
            );
        }
        other => panic!("expected MalformedAnswer, got {other:?}"),
    }
    Ok(())
}

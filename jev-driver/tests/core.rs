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

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

#[test]
fn state_keys_resolve_serde_names() {
    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "camelCase")]
    struct CamelState {
        ticket_text: String,
        #[serde(skip)]
        internal: String,
        #[serde(rename = "sla")]
        sla_hours: f64,
    }
    assert_eq!(
        <CamelState as StateKeys>::STATE_KEYS,
        Some(&["ticketText", "sla"][..]),
        "keys must be serde-resolved, declaration order, skips excluded"
    );
}

#[test]
fn compose_rejects_dangling_state_refs() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Clone, Copy, JevNoul)]
    #[jev(id = "urgent", instructions = "Does `ticket` convey urgency?")]
    struct Urgent;

    #[derive(serde::Serialize, JevState)]
    struct PlainState {
        message: String,
    }

    #[derive(JevQuestions)]
    #[jev(state = PlainState)]
    struct Qs {
        urgent: Urgent,
    }

    let err = Qs::request()
        .state(&PlainState {
            message: "now".to_owned(),
        })?
        .build()
        .expect_err("a reference rooted at a missing key must fail composition");
    match err {
        JevError::DanglingStateRef {
            question,
            reference,
        } => {
            assert_eq!(question, "urgent", "error names the question");
            assert_eq!(reference, "ticket", "error names the reference");
        }
        other => panic!("expected DanglingStateRef, got {other:?}"),
    }
    Ok(())
}

#[test]
fn compose_accepts_serde_renamed_roots() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Clone, Copy, JevNoul)]
    #[jev(id = "urgent", instructions = "Does `ticketText` convey urgency?")]
    struct Urgent;

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "camelCase")]
    struct CamelState {
        ticket_text: String,
    }

    #[derive(JevQuestions)]
    #[jev(state = CamelState)]
    struct Qs {
        urgent: Urgent,
    }

    let decision = Qs::request()
        .state(&CamelState {
            ticket_text: "refund today".to_owned(),
        })?
        .build()?;
    let instructions = decision
        .questions()
        .get("urgent")
        .expect("question present");
    assert_eq!(
        instructions.instructions().question_text(),
        Some("Does `ticketText` convey urgency?"),
        "references ride the wire verbatim"
    );
    Ok(())
}

#[test]
fn validate_state_refs_local_roots_and_exemptions() -> Result<(), Box<dyn std::error::Error>> {
    // Option keys and structured data fields are local roots, valid even
    // when the state carries no such key.
    let structured =
        Instructions::from_question("Pick `billing` for `payload`?", &json!({ "payload": 1 }))?;
    let mut questions = BTreeMap::new();
    questions.insert(
        "pick".to_owned(),
        WireQuestion::Choice {
            instructions: structured,
            criteria: BTreeMap::from([("billing".to_owned(), Some(Instructions::text("B")))]),
        },
    );
    assert!(
        validate_state_refs(&questions, Some(&["unrelated"])).is_ok(),
        "option-key and data-field roots must pass"
    );

    // A genuinely dangling root fails with the question named.
    questions.insert(
        "check".to_owned(),
        WireQuestion::Noul {
            instructions: Instructions::text("Is `ghost` present?"),
            criteria: None,
        },
    );
    let err = validate_state_refs(&questions, Some(&["real"]))
        .expect_err("a root matching no namespace must fail");
    match err {
        JevError::DanglingStateRef {
            question,
            reference,
        } => {
            assert_eq!(question, "check", "error names the question");
            assert_eq!(reference, "ghost", "error names the reference");
        }
        other => panic!("expected DanglingStateRef, got {other:?}"),
    }

    // Exemptions: opaque state keys and parts-form instructions.
    assert!(
        validate_state_refs(&questions, None).is_ok(),
        "opaque state keys exempt the set"
    );
    questions.insert(
        "parts".to_owned(),
        WireQuestion::Noul {
            instructions: Instructions::Parts(vec![json!("no single question text")]),
            criteria: None,
        },
    );
    questions.remove("check");
    assert!(
        validate_state_refs(&questions, Some(&[])).is_ok(),
        "parts-form instructions carry no refs to check"
    );

    // Index roots split at `[`; empty backtick pairs are not references.
    questions.insert(
        "idx".to_owned(),
        WireQuestion::Noul {
            instructions: Instructions::text("Is `items[0]` fresh? Yes `` indeed"),
            criteria: None,
        },
    );
    assert!(
        validate_state_refs(&questions, Some(&["items"])).is_ok(),
        "indexed roots and empty pairs must pass"
    );
    Ok(())
}

#[test]
fn state_keys_match_serde_serialization_across_rules() -> Result<(), Box<dyn std::error::Error>> {
    // Differential: whatever the macro resolves must equal what serde
    // actually emits, per rename rule, including acronym-run idents.
    fn assert_keys_match<T: serde::Serialize + StateKeys>(
        state: &T,
    ) -> Result<(), serde_json::Error> {
        let serde_json::Value::Object(map) = serde_json::to_value(state)? else {
            panic!("state must serialize to an object");
        };
        let mut serde_keys: Vec<String> = map.keys().cloned().collect();
        let mut resolved: Vec<String> = <T as StateKeys>::STATE_KEYS
            .unwrap_or_default()
            .iter()
            .map(|k| (*k).to_owned())
            .collect();
        serde_keys.sort_unstable();
        resolved.sort_unstable();
        assert_eq!(
            resolved, serde_keys,
            "STATE_KEYS must equal serde's emitted keys"
        );
        Ok(())
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "camelCase")]
    struct Camel {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "snake_case")]
    struct Snake {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    struct ScreamingSnake {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "kebab-case")]
    struct Kebab {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "SCREAMING-KEBAB-CASE")]
    struct ScreamingKebab {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "PascalCase")]
    struct Pascal {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "lowercase")]
    struct Lower {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "UPPERCASE")]
    struct Upper {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all(serialize = "camelCase", deserialize = "kebab-case"))]
    struct SplitForm {
        ticket_text: String,
        http_url: String,
        xmlHttpRequest: String,
        XMLHttp: String,
    }

    let text = "t".to_owned();
    assert_keys_match(&Camel {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&Snake {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&ScreamingSnake {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&Kebab {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&ScreamingKebab {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&Pascal {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&Lower {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&Upper {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    assert_keys_match(&SplitForm {
        ticket_text: text.clone(),
        http_url: text.clone(),
        xmlHttpRequest: text.clone(),
        XMLHttp: text.clone(),
    })?;
    Ok(())
}

#[test]
fn state_keys_skip_precedence_and_exemption_granularity() {
    // Skip wins over rename regardless of attribute order.
    #[derive(serde::Serialize, JevState)]
    struct SkipWins {
        live: String,
        #[serde(rename = "hidden", skip)]
        internal: String,
    }
    assert_eq!(
        <SkipWins as StateKeys>::STATE_KEYS,
        Some(&["live"][..]),
        "skip must win over rename"
    );

    // Value-shaping attrs keep the key static: the struct stays checked.
    #[derive(serde::Serialize, JevState)]
    struct Conditional {
        #[serde(skip_serializing_if = "Option::is_none")]
        maybe: Option<String>,
        live: String,
    }
    assert_eq!(
        <Conditional as StateKeys>::STATE_KEYS,
        Some(&["maybe", "live"][..]),
        "conditional presence must not exempt the struct"
    );

    // Valued sibling attrs (bound) must not derail rename_all resolution.
    #[derive(serde::Serialize, JevState)]
    #[serde(rename_all = "camelCase", bound = "")]
    struct WithBound {
        ticket_text: String,
    }
    assert_eq!(
        <WithBound as StateKeys>::STATE_KEYS,
        Some(&["ticketText"][..]),
        "sibling valued metas must not swallow the rename rule"
    );

    // Only flatten makes the key set itself unknowable.
    #[derive(serde::Serialize)]
    struct Extra {
        bonus: u32,
    }
    #[derive(serde::Serialize, JevState)]
    struct WithFlatten {
        live: String,
        #[serde(flatten)]
        extra: Extra,
    }
    assert_eq!(
        <WithFlatten as StateKeys>::STATE_KEYS,
        None,
        "flatten must exempt the struct"
    );
}

#[test]
fn jev_config_debug_redacts_the_api_key() -> Result<(), Box<dyn std::error::Error>> {
    let config = JevConfig {
        api_key: Some("sk-super-secret".to_owned()),
        endpoint: "https://api.typesafe.ai/v1/systemone".parse()?,
        model: "jev-1.13.0".to_owned(),
        timeout: std::time::Duration::from_secs(30),
        retry: RetryConfig::default(),
    };
    let debug = format!("{config:?}");
    assert!(
        !debug.contains("sk-super-secret"),
        "Debug must not leak the key: {debug}"
    );
    assert!(
        debug.contains("<redacted>"),
        "Debug marks the redaction: {debug}"
    );
    let keyless = format!("{:?}", JevConfig::cloud()?);
    assert!(
        keyless.contains("None"),
        "keyless Debug shows None: {keyless}"
    );
    Ok(())
}

#[test]
fn cloud_config_targets_the_full_default_endpoint() -> JevResult<()> {
    let config = JevConfig::cloud()?;
    assert_eq!(
        config.endpoint.as_str(),
        "https://api.typesafe.ai/v1/systemone",
        "cloud() must pin the complete endpoint"
    );
    assert!(config.api_key.is_none(), "cloud() ships no key");
    JevClient::new(config)?;
    Ok(())
}

#[test]
fn pathless_base_urls_get_the_systemone_path_joined() -> Result<(), Box<dyn std::error::Error>> {
    let bare = JevConfig::base("http://localhost:8080".parse()?)?;
    assert_eq!(
        bare.endpoint.as_str(),
        "http://localhost:8080/v1/systemone",
        "a bare host is a base URL"
    );
    let slashed = JevConfig::base("http://localhost:8080/".parse()?)?;
    assert_eq!(
        slashed.endpoint.as_str(),
        "http://localhost:8080/v1/systemone",
        "a lone slash is still a base URL"
    );
    let cloud_base = JevConfig::base("https://api.typesafe.ai".parse()?)?;
    assert_eq!(
        cloud_base.endpoint.as_str(),
        "https://api.typesafe.ai/v1/systemone",
        "the old TYPESAFE_BASE_URL shape must resolve identically"
    );
    Ok(())
}

#[test]
fn url_with_path_is_the_complete_endpoint() -> Result<(), Box<dyn std::error::Error>> {
    let full = JevConfig::base("http://localhost:9999/api/jev".parse()?)?;
    assert_eq!(
        full.endpoint.as_str(),
        "http://localhost:9999/api/jev",
        "a URL with a path must be used verbatim"
    );
    Ok(())
}

#[test]
fn from_env_named_defaults_when_no_variables_are_named() -> JevResult<()> {
    let config = JevConfig::from_env_named(EnvNames::default())?;
    assert!(config.api_key.is_none(), "no key variable means no key");
    assert_eq!(
        config.endpoint.as_str(),
        "https://api.typesafe.ai/v1/systemone",
        "no URL variable means the cloud endpoint"
    );
    assert_eq!(
        config.model, DEFAULT_MODEL,
        "no model variable means default"
    );
    Ok(())
}

fn set_test_var(name: &str, value: &str) {
    // SAFETY: test-only env write; every name passed here is
    // collision-proof within this binary and read by exactly one test.
    unsafe {
        std::env::set_var(name, value);
    }
}

#[test]
fn from_env_named_reads_caller_named_variables() -> JevResult<()> {
    set_test_var("JEV_TEST_KEY", "local-token");
    set_test_var("JEV_TEST_URL", "http://localhost:1234");
    set_test_var("JEV_TEST_MODEL", "kev-k5");
    let config = JevConfig::from_env_named(EnvNames {
        api_key: Some("JEV_TEST_KEY"),
        url: Some("JEV_TEST_URL"),
        model: Some("JEV_TEST_MODEL"),
    })?;
    assert_eq!(
        config.api_key.as_deref(),
        Some("local-token"),
        "the caller-named key variable is read"
    );
    assert_eq!(
        config.endpoint.as_str(),
        "http://localhost:1234/v1/systemone",
        "the caller-named URL variable is read and joined"
    );
    assert_eq!(
        config.model, "kev-k5",
        "the caller-named model variable is read"
    );
    Ok(())
}

#[test]
fn from_env_named_normalizes_keys() -> JevResult<()> {
    set_test_var("JEV_TEST_BLANK_KEY", "  ");
    let blank = JevConfig::from_env_named(EnvNames {
        api_key: Some("JEV_TEST_BLANK_KEY"),
        ..EnvNames::default()
    })?;
    assert!(
        blank.api_key.is_none(),
        "a blank key must not become a bearer token"
    );
    set_test_var("JEV_TEST_PAD_KEY", " local-token ");
    let padded = JevConfig::from_env_named(EnvNames {
        api_key: Some("JEV_TEST_PAD_KEY"),
        ..EnvNames::default()
    })?;
    assert_eq!(
        padded.api_key.as_deref(),
        Some("local-token"),
        "padded keys are normalized, not sent verbatim"
    );
    Ok(())
}

#[test]
fn from_env_named_names_the_offending_variable_on_a_bad_url() {
    set_test_var("JEV_TEST_BAD_URL", "not a url");
    match JevConfig::from_env_named(EnvNames {
        url: Some("JEV_TEST_BAD_URL"),
        ..EnvNames::default()
    }) {
        Err(err) => assert!(
            err.to_string().contains("JEV_TEST_BAD_URL"),
            "error must name the variable: {err}"
        ),
        Ok(config) => panic!("malformed URL must error, got {config:?}"),
    }
}

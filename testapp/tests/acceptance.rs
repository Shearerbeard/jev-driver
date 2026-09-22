//! Red-first acceptance suite: the executable spec for jev-driver.
//! Every test here was authored before the implementation existed and is
//! expected to fail (red) until Stages 1-3 land.

use jev_driver::prelude::*;
use serde_json::json;
use testapp::questions::*;

// Reuse the fixture files that main.rs replays.
const RESPONSE: &str = include_str!("../fixtures/triage.response.json");
const REQUEST: &str = include_str!("../fixtures/triage.request.json");
const SCHEMA: &str = include_str!("../fixtures/triage.schema.json");

fn fixture_response() -> WireResponse {
    serde_json::from_str(RESPONSE).unwrap_or_else(|e| panic!("fixture response must parse: {e}"))
}

#[test]
fn schema_artifact_matches_derive() -> Result<(), Box<dyn std::error::Error>> {
    let artifact: DecisionSchema = serde_json::from_str(SCHEMA)?;
    artifact.validate()?;
    assert_eq!(
        artifact,
        TicketTriage::schema(),
        "schema artifact drifted from the derive"
    );
    Ok(())
}

#[test]
fn request_body_is_golden() -> Result<(), Box<dyn std::error::Error>> {
    let decision = build_demo_decision()?;
    let body = WireRequestBody {
        state: decision.state(),
        model: DEFAULT_MODEL,
        questions: decision.questions(),
    };
    let actual = serde_json::to_string_pretty(&body)?;
    assert_eq!(
        actual.trim(),
        REQUEST.trim(),
        "request wire JSON must match the golden fixture byte-for-byte"
    );
    Ok(())
}

#[test]
fn fixture_parses_into_typed_answers() -> Result<(), Box<dyn std::error::Error>> {
    let decision = build_demo_decision()?;
    let answers = Answers::from_wire(fixture_response(), &decision)?;
    let typed: TicketTriageAnswers = answers.parse_as()?;

    assert_eq!(
        typed.department.selected,
        Department::Billing,
        "selected option round-trips into the enum"
    );
    let p_technical = typed
        .department
        .probabilities
        .get(&Department::Technical)
        .get();
    assert!(
        (p_technical - 0.12).abs() < 1e-9,
        "per-variant probability is typed and indexed, got {p_technical}"
    );
    assert!(
        typed.department.confidence.act(0.80),
        "confidence 0.81 clears the 0.80 act gate"
    );
    assert!(
        !typed.department.confidence.act(0.99),
        "confidence 0.81 must not clear the 0.99 gate"
    );

    assert_eq!(
        typed.frustration.nearest()?,
        Frustration::Concerned,
        "score nearest level maps back to the rubric enum"
    );
    assert!(
        (typed.frustration.score() - 1.05).abs() < 1e-9,
        "probability-weighted score round-trips, got {}",
        typed.frustration.score()
    );

    assert!(
        typed.is_urgent.probability.yes(0.90),
        "noul 0.95 clears the 0.90 yes gate"
    );
    Ok(())
}

#[test]
fn dynamic_and_typed_layers_agree() -> Result<(), Box<dyn std::error::Error>> {
    let decision = build_demo_decision()?;
    let answers = Answers::from_wire(fixture_response(), &decision)?;
    let typed: TicketTriageAnswers = answers.parse_as()?;

    let dyn_department = answers.choice("department")?;
    assert_eq!(
        dyn_department.selected(),
        "billing",
        "dynamic layer sees the raw option key"
    );
    let typed_key = typed.department.selected.option_name();
    assert_eq!(
        dyn_department.selected(),
        typed_key,
        "typed enum maps to the same wire key"
    );

    let owner = answers.choice("owner")?;
    assert_eq!(
        owner.selected(),
        "dana",
        "dynamic DynChoice answer validates against its supplied option set"
    );
    Ok(())
}

#[test]
fn unknown_option_is_strictly_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let decision = build_demo_decision()?;
    let mut value = serde_json::from_str::<serde_json::Value>(RESPONSE)?;
    value["answers"]["owner"]["choice"] = json!("mallory");
    let mutated: WireResponse = serde_json::from_value(value)?;

    let err = Answers::from_wire(mutated, &decision)
        .expect_err("an option outside the supplied set must be rejected");
    match err {
        JevError::UnknownOption { option, id } => {
            assert_eq!(option, "mallory", "error names the offending option");
            assert_eq!(id, "owner", "error names the offending question");
        }
        other => panic!("expected UnknownOption, got {other:?}"),
    }
    Ok(())
}

#[test]
fn unknown_answer_id_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let decision = build_demo_decision()?;
    let mut value = serde_json::from_str::<serde_json::Value>(RESPONSE)?;
    value["answers"]["ghost"] = json!({ "type": "noul", "noul": 0.5 });
    let mutated: WireResponse = serde_json::from_value(value)?;

    let err = Answers::from_wire(mutated, &decision)
        .expect_err("an answer for an unasked question must be rejected");
    match err {
        JevError::UnknownQuestion { id } => {
            assert_eq!(id, "ghost", "error names the ghost question id")
        }
        other => panic!("expected UnknownQuestion, got {other:?}"),
    }
    Ok(())
}

#[test]
fn runtime_criteria_must_be_supplied() -> Result<(), Box<dyn std::error::Error>> {
    let err = TicketTriage::request()
        .state(&demo_state())?
        .build()
        .expect_err("department has runtime criteria; omitting customize() must fail loudly");
    match err {
        JevError::MissingCriteria { id } => {
            assert_eq!(id, "department", "error names the unfilled question")
        }
        other => panic!("expected MissingCriteria, got {other:?}"),
    }
    Ok(())
}

#[test]
fn state_is_required() -> Result<(), Box<dyn std::error::Error>> {
    let err = TicketTriage::request()
        .customize(|c| {
            c.department = Some(DepartmentCriteria {
                billing: Instructions::text(TENANT.billing_desc),
                technical: Instructions::text(TENANT.technical_desc),
                other: Instructions::text(TENANT.other_desc),
            });
        })
        .build()
        .expect_err("building without state must fail");
    assert!(
        matches!(err, JevError::MissingState),
        "expected MissingState, got {err:?}"
    );
    Ok(())
}

#[test]
fn typed_state_pairing_rejects_wrong_shapes() -> Result<(), Box<dyn std::error::Error>> {
    // TicketTriage::request().state(...) only accepts &TicketState; the
    // compile-time half of this contract is covered by the trybuild suite
    // in jev-driver. Here we verify the runtime half: serialization must
    // produce exactly the state object in the golden request.
    let decision = build_demo_decision()?;
    assert_eq!(
        decision.state()["account_tier"],
        json!("pro"),
        "typed state serializes through its serde impl"
    );
    Ok(())
}

#[test]
fn dyn_choice_builder_enforces_cardinality() -> Result<(), Box<dyn std::error::Error>> {
    let empty = DynChoice::new("nobody", Instructions::text("Who?")).build();
    assert!(
        matches!(empty, Err(JevError::Schema(_))),
        "zero options must be a schema error, got {empty:?}"
    );

    let mut builder = DynChoice::new("everyone", Instructions::text("Who?"));
    for i in 0..256 {
        builder = builder.option(&format!("opt_{i}"), Instructions::text("someone"));
    }
    let too_many = builder.build();
    assert!(
        matches!(too_many, Err(JevError::Schema(_))),
        "256 options must exceed the 255 cardinality cap"
    );
    Ok(())
}

#[test]
fn schema_export_round_trips_through_json() -> Result<(), Box<dyn std::error::Error>> {
    let schema = TicketTriage::schema();
    let json_str = schema.to_json_pretty()?;
    let reparsed: DecisionSchema = serde_json::from_str(&json_str)?;
    assert_eq!(
        reparsed, schema,
        "DecisionSchema IR must round-trip losslessly through JSON"
    );
    assert_eq!(
        json_str.trim(),
        SCHEMA.trim(),
        "exported artifact matches the committed fixture"
    );
    Ok(())
}

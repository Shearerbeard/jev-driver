//! Ticket-triage demo: declare questions as Rust types, build the
//! decision, and parse a recorded response into typed answers.
//! Offline and keyless - run with `cargo run --example triage`. For
//! the live path (JevClient, async) see `testapp/`; the validation
//! below is the same one a live evaluation applies.

// Root re-exports: inside the jev-driver package itself, the derives
// resolve paths against this example's crate root, so the
// macro-addressed names must be in scope here. External consumers
// don't need this line.
use jev_driver::__private;
use jev_driver::StateSpec;
use jev_driver::prelude::*;
use jev_driver::wire::WireResponse;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JevState)]
#[jev(describe = "A single inbound support ticket")]
struct TicketState {
    ticket: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JevChoice)]
#[jev(id = "department", instructions = "Which team should handle `ticket`?")]
enum Department {
    #[jev(criteria = "Payments, invoicing, refunds")]
    Billing,
    #[jev(criteria = "Bugs and outages")]
    Technical,
}

#[derive(Debug, Clone, Copy, JevNoul)]
#[jev(
    id = "is_urgent",
    instructions = "Does `ticket` convey time pressure?",
    yes = "Explicit deadline or costly delay",
    no = "No time pressure expressed"
)]
struct IsUrgent;

#[derive(Debug, Clone, JevQuestions)]
#[jev(state = TicketState)]
// The fields carry the question set for the derive; generated code
// reads them, which dead-code analysis cannot see.
#[allow(dead_code, reason = "fields are consumed by the derive expansion")]
struct TicketTriage {
    department: Department,
    is_urgent: IsUrgent,
}

const RECORDED: &str = r#"
{
  "model": "jev-1.13.0",
  "answers": {
    "department": {
      "type": "choice",
      "choice": "billing",
      "probabilities": { "billing": 0.88, "technical": 0.12 },
      "confidence": 0.81
    },
    "is_urgent": { "type": "noul", "noul": 0.91 }
  },
  "usage": { "input_tokens": 180, "output_tokens": 24 }
}
"#;

fn main() -> JevResult<()> {
    let state = TicketState {
        ticket: "Refund my last invoice today or I escalate".to_owned(),
    };
    let decision = TicketTriage::request().state(&state)?.build()?;

    // Offline: a recorded response, validated exactly like a live one.
    let raw: WireResponse = serde_json::from_str(RECORDED)?;
    let validated = Answers::from_wire(raw, &decision)?;
    let answers: TicketTriageAnswers = validated.parse_as()?;

    // Plain match on your enum; confidence decides auto-action vs review.
    let lane = match answers.department.selected {
        Department::Billing if answers.department.confidence.act(0.80) => "billing (auto)",
        Department::Technical if answers.department.confidence.act(0.80) => "technical (auto)",
        Department::Billing | Department::Technical => "human review (low confidence)",
    };
    println!("department: {lane}");
    if answers.is_urgent.probability.yes(0.50) {
        println!("is_urgent: page on-call");
    }
    println!(
        "usage: {} in / {} out tokens",
        validated.usage.input_tokens, validated.usage.output_tokens
    );
    Ok(())
}

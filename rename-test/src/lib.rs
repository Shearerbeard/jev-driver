//! Consumer that renames its jev-driver dependency
//! (`jev = { package = "jev-driver" }`): the derives must resolve the
//! real crate name at expansion time, not a hardcoded `::jev_driver::`
//! path. Exercises every derive and both macro-addressed helpers.
#![cfg(test)]

use jev::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JevState)]
#[jev(describe = "One inbound support ticket")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, JevScore)]
#[jev(
    id = "frustration",
    instructions = "How frustrated is the writer of `ticket`?"
)]
enum Frustration {
    #[jev(criteria = "Calm and neutral")]
    Calm,
    #[jev(criteria = "Strong language")]
    Angry,
}

#[derive(Debug, Clone, Copy, JevNoul)]
#[jev(id = "is_urgent", instructions = "Does `ticket` convey time pressure?")]
struct IsUrgent;

#[derive(Debug, Clone, Serialize, JevInstructions)]
#[jev(question = "Does `detail` mention a payment problem?")]
struct PaymentProblem {
    detail: String,
}

#[derive(Debug, Clone, JevQuestions)]
#[jev(state = TicketState)]
struct Triage {
    department: Department,
    frustration: Frustration,
    is_urgent: IsUrgent,
}

#[test]
fn derives_expand_under_a_renamed_dependency() {
    let payment = PaymentProblem {
        detail: "card was charged twice".to_owned(),
    };
    let instructions = payment
        .to_instructions()
        .expect("structured instructions serialize");
    assert!(instructions.is_non_empty());

    // Runtime score criteria route through the hidden helper, proving
    // the macro resolved `__private` under the rename too.
    let customization = TriageCustomization {
        frustration: Some(FrustrationCriteria(vec![
            Instructions::text("Calm"),
            Instructions::text("Angry"),
        ])),
        ..TriageCustomization::default()
    };
    let questions = <Triage as QuestionSet>::compose(&customization).expect("composes");

    assert_eq!(questions.len(), 3);
    let department = questions.get("department").expect("department present");
    let options = department.choice_options().expect("choice question");
    assert_eq!(options.len(), 2);
}

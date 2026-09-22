//! The ticket-triage question set: the executable spec for jev-driver's
//! public API. Everything used here is required surface area.

use jev_driver::prelude::*;
use serde::{Deserialize, Serialize};

/// Choice with runtime criteria: option keys come from the enum, the
/// rubric text is supplied per call (see [`TENANT`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, JevChoice)]
#[jev(id = "department", instructions = "Which team should handle this ticket?")]
pub enum Department {
    Billing,
    Technical,
    #[jev(option = "other")]
    Other,
}

/// Noul: P(yes) with calibrated criteria for both poles.
#[derive(Debug, Clone, Copy, JevNoul)]
#[jev(
    id = "is_urgent",
    instructions = "Does this ticket convey time pressure?",
    yes = "Explicit deadline or costly delay",
    no = "No time pressure expressed"
)]
pub struct IsUrgent;

/// Score: ordered enum variants become the rubric levels 0, 1, 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JevScore)]
#[jev(id = "frustration", instructions = "How frustrated does the customer appear?")]
pub enum Frustration {
    #[jev(criteria = "Calm and neutral")]
    Calm,
    #[jev(criteria = "Concerned but civil")]
    Concerned,
    #[jev(criteria = "Very angry or strong language")]
    Angry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Enterprise,
}

/// Typed state: paired with [`TicketTriage`] at compile time; field
/// descriptions land in the exported schema artifact.
#[derive(Debug, Clone, Serialize, JevState)]
#[jev(describe = "A single inbound support ticket")]
pub struct TicketState {
    pub ticket: String,
    #[jev(describe = "Hours left on the customer's SLA clock")]
    pub sla_hours: f64,
    pub account_tier: Tier,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateRef {
    pub name: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationRef {
    pub id: String,
    pub stated_employer: String,
}

/// Struct-to-string contract: backtick references in the question text are
/// checked against this struct's fields at compile time (`candidate`).
#[derive(Debug, Clone, Serialize, JevInstructions)]
#[jev(question = "Is this application for the same person as `candidate`?")]
pub struct SamePersonQ {
    pub candidate: CandidateRef,
    pub application: ApplicationRef,
}

/// The question set. One derive generates the schema IR, the typed request
/// builder (state-locked to [`TicketState`]), the criteria editor plumbing,
/// and the typed answers struct `TicketTriageAnswers`.
#[derive(Debug, Clone, JevQuestions)]
#[jev(state = TicketState)]
pub struct TicketTriage {
    pub department: Department,
    pub is_urgent: IsUrgent,
    pub frustration: Frustration,
}

/// Demo tenant: proves criteria text can be resolved at call time.
pub struct Tenant {
    pub product: &'static str,
    pub billing_desc: &'static str,
    pub technical_desc: &'static str,
    pub other_desc: &'static str,
}

pub const TENANT: Tenant = Tenant {
    product: "Loom",
    billing_desc: "Payments, invoicing, and refunds (Acme Corp)",
    technical_desc: "Bugs, outages, and Loom integrations",
    other_desc: "None of the above clearly fits",
};

pub fn demo_state() -> TicketState {
    TicketState {
        ticket: "I was charged twice and need the duplicate refunded today.".to_owned(),
        sla_hours: 47.5,
        account_tier: Tier::Pro,
    }
}

/// Builds the full demo decision: derived question set with runtime
/// criteria, typed state, structured instructions feeding a dynamic choice.
pub fn build_demo_decision() -> JevResult<Decision> {
    let same_person = SamePersonQ {
        candidate: CandidateRef {
            name: "John Smith".to_owned(),
            location: "Oakland, California".to_owned(),
        },
        application: ApplicationRef {
            id: "app_913".to_owned(),
            stated_employer: "Alphabet".to_owned(),
        },
    };

    let owner = DynChoice::new("owner", same_person.to_instructions()?)
        .option(
            "dana",
            Instructions::text("Payments specialist, 4 years tenure"),
        )
        .option(
            "kai",
            Instructions::text("Integrations specialist, 2 years tenure"),
        )
        .build()?;

    TicketTriage::request()
        .state(&demo_state())?
        .customize(|c| {
            c.department = Some(DepartmentCriteria {
                billing: Instructions::text(TENANT.billing_desc),
                technical: Instructions::text(TENANT.technical_desc),
                other: Instructions::text(TENANT.other_desc),
            });
        })
        .build()?
        .with_question(owner)
}

/// Confidence-gated routing over the typed answer: plain match on our own
/// enum, with confidence deciding auto-action vs. human review.
pub fn route(t: &TicketTriageAnswers) -> &'static str {
    match t.department.selected {
        Department::Billing if t.department.confidence.act(0.80) => "billing lane (auto)",
        Department::Technical if t.department.confidence.act(0.80) => "technical lane (auto)",
        _ => "human review (low confidence)",
    }
}

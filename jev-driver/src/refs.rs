//! Backtick reference validation: the state-key contract and the
//! compose-time root check. The model resolves `` `ticket.messages[0]` ``
//! style paths against the state it was sent, so every reference's root
//! segment must be a key serde actually emits.

use std::collections::BTreeMap;

use crate::error::{JevError, JevResult};
use crate::wire::{Instructions, WireQuestion};

/// State contract for reference validation, implemented by `JevState`.
///
/// `STATE_KEYS` lists the top-level keys serde emits, in declaration
/// order, or is `None` when the serialized shape is not statically
/// knowable (`flatten`, conditional skips, custom serialization) and
/// root checking is exempt. Hand-implement to validate dynamic states.
pub trait StateKeys {
    /// Serialized top-level keys, or `None` to exempt the type.
    const STATE_KEYS: Option<&'static [&'static str]>;
}

/// Validates every question's instruction references: each backtick
/// token's root segment must be a state key, an option key of that
/// question (choice questions), or a data field of the structured
/// instructions payload itself. `None` state keys exempt the set.
///
/// Called automatically by `JevQuestions` composition when a state type
/// is paired; public for IR-surface callers assembling decisions by hand.
pub fn validate_state_refs(
    questions: &BTreeMap<String, WireQuestion>,
    state_keys: Option<&[&str]>,
) -> JevResult<()> {
    let Some(state_keys) = state_keys else {
        return Ok(());
    };
    for (id, question) in questions {
        for token in refs_of(question.instructions()) {
            let root = root_segment(&token);
            let allowed = state_keys.contains(&root)
                || question
                    .choice_options()
                    .is_some_and(|options| options.contains_key(root))
                || data_root(question.instructions(), root);
            if !allowed {
                return Err(JevError::DanglingStateRef {
                    question: id.clone(),
                    reference: token,
                });
            }
        }
    }
    Ok(())
}

/// Reference tokens of the instructions' question text: the non-empty
/// segments between backtick pairs (empty pairs are prose artifacts,
/// not references). Parts-form instructions carry no single question
/// text and are exempt.
fn refs_of(instructions: &Instructions) -> Vec<String> {
    instructions
        .question_text()
        .map(|text| {
            text.split('`')
                .skip(1)
                .step_by(2)
                .filter(|token| !token.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The root segment of a reference path: the text before the first `.`
/// or `[`.
fn root_segment(token: &str) -> &str {
    token.split(['.', '[']).next().unwrap_or(token)
}

/// Whether `root` names a data field of the structured payload (the
/// fields riding alongside the embedded `question`).
fn data_root(instructions: &Instructions, root: &str) -> bool {
    match instructions {
        Instructions::Structured(map) => map.keys().any(|key| key == root && key != "question"),
        Instructions::Text(_) | Instructions::Parts(_) => false,
    }
}

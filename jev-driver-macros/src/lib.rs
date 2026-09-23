//! Derive macros for jev-driver.
//!
//! * `JevChoice` — enum variants become choice options (keys default to
//!   snake_case, override with `#[jev(option = "...")]`).
//! * `JevScore` — ordered enum variants become rubric levels.
//! * `JevNoul` — a unit struct carrying a yes/no question.
//! * `JevState` — documents a state payload type in the schema IR.
//! * `JevInstructions` — a struct-to-string question contract with
//!   compile-time backtick validation.
//! * `JevQuestions` — a struct of questions becomes the schema, the typed
//!   request builder (paired with a `JevState` type), the criteria
//!   customization container, and the typed answers struct.

mod choice;
mod instructions;
mod noul;
mod questions;
mod score;
mod serde_names;
mod state;
mod util;

use proc_macro::TokenStream;

fn derive(
    input: TokenStream,
    expand: fn(&syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream>,
) -> TokenStream {
    util::guarded(input.into(), expand).into()
}

/// Derives [`jev_driver::ChoiceOptions`] and [`jev_driver::QuestionType`]
/// for a choice question.
#[proc_macro_derive(JevChoice, attributes(jev))]
pub fn derive_jev_choice(input: TokenStream) -> TokenStream {
    derive(input, choice::expand)
}

/// Derives [`jev_driver::ScoreLevels`] and [`jev_driver::QuestionType`]
/// for a score question.
#[proc_macro_derive(JevScore, attributes(jev))]
pub fn derive_jev_score(input: TokenStream) -> TokenStream {
    derive(input, score::expand)
}

/// Derives [`jev_driver::QuestionType`] for a noul question.
#[proc_macro_derive(JevNoul, attributes(jev))]
pub fn derive_jev_noul(input: TokenStream) -> TokenStream {
    derive(input, noul::expand)
}

/// Derives `state_spec()` for a state payload type.
#[proc_macro_derive(JevState, attributes(jev))]
pub fn derive_jev_state(input: TokenStream) -> TokenStream {
    derive(input, state::expand)
}

/// Derives `to_instructions()` for a struct-to-string question contract.
#[proc_macro_derive(JevInstructions, attributes(jev))]
pub fn derive_jev_instructions(input: TokenStream) -> TokenStream {
    derive(input, instructions::expand)
}

/// Derives the question set: schema IR, typed request builder,
/// customization container, and typed answers struct.
#[proc_macro_derive(JevQuestions, attributes(jev))]
pub fn derive_jev_questions(input: TokenStream) -> TokenStream {
    derive(input, questions::expand)
}

//! JevInstructions references must use serde's serialized keys, not
//! Rust field names.

use jev_driver::JevInstructions;
use serde::Serialize;

#[derive(Serialize, JevInstructions)]
#[serde(rename_all = "camelCase")]
#[jev(question = "Is this the same person as `stated_employer`?")]
struct SameEmployer {
    stated_employer: String,
}

fn main() {}

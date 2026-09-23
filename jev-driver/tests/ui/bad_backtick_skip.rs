//! JevInstructions cannot reference fields serde never serializes.

use jev_driver::JevInstructions;
use serde::Serialize;

#[derive(Serialize, JevInstructions)]
#[jev(question = "Is the `internal_note` favorable?")]
struct WithSkip {
    public: String,
    #[serde(skip)]
    internal_note: String,
}

fn main() {}

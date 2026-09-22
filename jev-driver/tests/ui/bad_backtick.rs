//! JevInstructions backtick references must name real fields.

use jev_driver::JevInstructions;

#[derive(JevInstructions)]
#[jev(question = "Is this the same person as `nam`?")]
struct SamePerson {
    name: String,
}

fn main() {}

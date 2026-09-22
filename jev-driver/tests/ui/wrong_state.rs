//! RequestBuilder::state only accepts the paired state type.

use jev_driver::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, JevChoice)]
#[jev(id = "pick", instructions = "Pick one")]
enum Pick {
    #[jev(criteria = "option a")]
    A,
    #[jev(criteria = "option b")]
    B,
}

#[derive(Serialize, JevState)]
#[jev(describe = "the real state")]
struct RealState {
    payload: String,
}

#[derive(Serialize)]
struct WrongState {
    different: u32,
}

#[derive(JevQuestions)]
#[jev(state = RealState)]
struct Qs {
    pick: Pick,
}

fn main() {
    let wrong = WrongState { different: 3 };
    let _ = Qs::request().state(&wrong);
}

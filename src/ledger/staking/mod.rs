//!
//! # Data structures of staking
//!

use ruc::*;
use serde::{Deserialize, Serialize};
use vsdb::{BranchName, Vs};

#[derive(Vs, Deserialize, Serialize)]
pub(crate) struct State {
    // TODO
}

impl State {
    pub(crate) fn new() -> Self {
        // TODO
        Self {}
    }
}

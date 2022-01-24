//!
//! # Data structures of staking
//!

use serde::{Deserialize, Serialize};
use vsdb::Vs;

#[derive(Vs, Default, Clone, Debug, Deserialize, Serialize)]
pub(crate) struct State {
    // TODO
}

// impl State {
//     pub(crate) fn new() -> Self {
//         // TODO
//         Self {}
//     }
// }

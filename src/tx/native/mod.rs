//!
//! # Functiosn of native transactions
//!
//! eg:
//! - Staking
//! - System management
//!

use crate::ledger::StateBranch;
use ruc::*;
use serde::{Deserialize, Serialize};
use vsdb::BranchName;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    // TODO
}

impl Tx {
    pub(crate) fn apply(self, _sb: &mut StateBranch, _b: BranchName) -> Result<()> {
        todo!()
    }
}

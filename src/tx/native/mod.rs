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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    // TODO
}

impl Tx {
    pub(crate) fn apply(self, _sb: &mut StateBranch) -> Result<()> {
        todo!()
    }
}

//!
//! # Functiosn of native transactions
//!
//! eg:
//! - Staking
//! - System management
//!

use crate::ledger::StateBranch;
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use std::{fmt, result::Result as StdResult};
use vsdb::BranchName;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    mock: [[u8; 16]; 16], // TODO
}

impl Tx {
    pub(crate) fn apply(
        self,
        _sb: &mut StateBranch,
        _b: BranchName,
    ) -> StdResult<ExecRet, Option<ExecRet>> {
        // TODO
        Err(None)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ExecRet {
    pub(crate) success: bool,
    pub(crate) caller: H160,
    pub(crate) fee_used: U256,
    pub(crate) log: String,
}

impl fmt::Display for ExecRet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

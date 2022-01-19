//!
//! # Transaction logic
//!

pub mod native;

use crate::ethvm;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Tx {
    Evm(ethvm::tx::Tx),
    Native(native::Tx),
}

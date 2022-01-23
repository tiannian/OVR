//!
//! # Transaction logic
//!

pub mod native;

use crate::{common::HashValue, ethvm};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Tx {
    Evm(ethvm::tx::Tx),
    Native(native::Tx),
}

impl Tx {
    pub(crate) fn hash(&self) -> HashValue {
        todo!()
    }
}

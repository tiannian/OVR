//!
//! # Transaction logic
//!

pub mod native;

use crate::{
    common::{hash_sha3_256, HashValue},
    ethvm,
};
use serde::{Deserialize, Serialize};
use vsdb::ValueEn;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Tx {
    Evm(ethvm::tx::Tx),
    Native(native::Tx),
}

impl Tx {
    #[inline(always)]
    pub(crate) fn hash(&self) -> HashValue {
        hash_sha3_256(&[&self.encode_value()])
    }
}

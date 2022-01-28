//!
//! # Transaction logic
//!

pub mod native;

use crate::{
    common::{hash_sha3_256, HashValue},
    ethvm,
};
use ruc::*;
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

    #[inline(always)]
    pub(crate) fn deserialize(bytes: &[u8]) -> Result<Tx> {
        serde_json::from_slice(bytes).c(d!())
    }

    // TODO
    #[inline(always)]
    pub(crate) fn valid_in_abci(&self) -> bool {
        true
    }
}

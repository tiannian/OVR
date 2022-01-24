//!
//! # Precompiled components(contracts)
//!

use evm::executor::stack::PrecompileFn;
use fevm::Precompile;
use fevm_precompile_modexp::Modexp;
use fevm_precompile_sha3fips::Sha3FIPS256;
use fevm_precompile_simple::{
    ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256,
};
use once_cell::sync::Lazy;
use primitive_types::H160;
use ruc::*;
use std::collections::BTreeMap;

pub(crate) static PRECOMPILE_SET: Lazy<BTreeMap<H160, PrecompileFn>> = Lazy::new(|| {
    map! {B
        idx_to_h160(1) => ECRecover::execute as PrecompileFn,
        idx_to_h160(2) => Sha256::execute,
        idx_to_h160(3) => Ripemd160::execute,
        idx_to_h160(4) => Identity::execute,
        idx_to_h160(5) => Modexp::execute,
        idx_to_h160(6) => ECRecoverPublicKey::execute, // Compitable with Findora
        idx_to_h160(7) => Sha3FIPS256::execute, // Compitable with Findora
    }
});

#[inline(always)]
pub(crate) fn idx_to_h160(i: u64) -> H160 {
    H160::from_low_u64_be(i)
}

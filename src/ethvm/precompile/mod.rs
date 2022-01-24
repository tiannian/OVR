//!
//! # Precompiled components(contracts)
//!
//! - ERC20 tokens
//!     - OVR
//!     - OVRG
//! - ...
//!

pub(crate) mod token;

use evm::executor::stack::PrecompileFn;
use once_cell::sync::Lazy;
use primitive_types::H160;
use ruc::*;
use std::collections::BTreeMap;
use token::{Erc20Like, Precompile, TOKENS};

pub(crate) static PRECOMPILE_SET: Lazy<BTreeMap<H160, PrecompileFn>> = Lazy::new(|| {
    let mut res = TOKENS
        .keys()
        .map(|addr| (*addr, <Erc20Like as Precompile>::runner as PrecompileFn))
        .collect();
    // TODO
    res
});

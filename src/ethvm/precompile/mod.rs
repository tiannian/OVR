//!
//! # Precompiled components(contracts)
//!
//! - ERC20 tokens
//!     - OVR
//!     - OVRG
//! - ...
//!

use evm::executor::stack::PrecompileFn;
use once_cell::sync::Lazy;
use primitive_types::H160;
use std::collections::BTreeMap;

pub(crate) static PRECOMPILE_SET: Lazy<BTreeMap<H160, PrecompileFn>> = Lazy::new(|| {
    // TODO
    BTreeMap::new()
});

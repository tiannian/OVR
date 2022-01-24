use crate::ethvm::OvrAccount;
use evm::{
    executor::stack::{PrecompileFailure, PrecompileOutput},
    Context,
};
use once_cell::sync::Lazy;
use primitive_types::{H160, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use slices::u8_slice;
use std::collections::BTreeMap;
use std::result::Result as StdResult;
use vsdb::{MapxVs, OrphanVs, Vs};

pub(crate) static OVR: Lazy<Erc20Like> = Lazy::new(Erc20Like::ovr_token);
pub(crate) static OVRG: Lazy<Erc20Like> = Lazy::new(Erc20Like::ovrg_token);

pub(crate) static TOKENS: Lazy<BTreeMap<H160, &'static Erc20Like>> = Lazy::new(|| {
    map! {B
        OVR.contract_addr => &*OVR,
        OVRG.contract_addr => &*OVRG
    }
});

// The gas used value is obtained according to the standard erc20 call.
// https://github.com/OpenZeppelin/openzeppelin-contracts/blob/v4.3.2/contracts/token/ERC20/ERC20.sol
const GAS_NAME: u64 = 3283;
const GAS_SYMBOL: u64 = 3437;
const GAS_DECIMALS: u64 = 243;
const GAS_TOTAL_SUPPLY: u64 = 1003;
const GAS_BALANCE_OF: u64 = 1350;
const GAS_TRANSFER: u64 = 23661;
const GAS_ALLOWANCE: u64 = 1624;
const GAS_APPROVE: u64 = 20750;
const GAS_TRANSFER_FROM: u64 = 6610;

#[derive(Vs, Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Erc20Like {
    pub(crate) name: Vec<u8>,
    pub(crate) symbol: Vec<u8>,
    pub(crate) issue_cap: Option<U256>,
    pub(crate) total_supply: OrphanVs<U256>,
    pub(crate) accounts: MapxVs<H160, OvrAccount>,
    pub(crate) contract_addr: H160,
}

impl Erc20Like {
    fn new(
        name: Vec<u8>,
        symbol: Vec<u8>,
        issue_cap: Option<U256>,
        contract_addr: H160,
    ) -> Self {
        let ver: &[u8; 0] = &[];
        Self {
            name,
            symbol,
            issue_cap,
            total_supply: OrphanVs::new(ver[..].into(), 0u8.into()),
            accounts: MapxVs::new(),
            contract_addr,
        }
    }

    pub(crate) fn ovr_token() -> Self {
        todo!()
    }

    pub(crate) fn ovrg_token() -> Self {
        todo!()
    }
}

pub(crate) trait Precompile {
    fn runner(
        input: &[u8],
        gas_limit: Option<u64>,
        context: &Context,
        is_static: bool,
    ) -> StdResult<PrecompileOutput, PrecompileFailure>;
}

impl Precompile for Erc20Like {
    fn runner(
        input: &[u8],
        gas_limit: Option<u64>,
        context: &Context,
        is_static: bool,
    ) -> StdResult<PrecompileOutput, PrecompileFailure> {
        todo!()
    }
}

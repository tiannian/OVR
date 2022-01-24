mod impls;
mod precompile;
pub mod tx;

use crate::common::BlockHeight;
use impls::backend::OvrBackend;
use precompile::token::Erc20Like;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use vsdb::{BranchName, Mapx, MapxOrd, OrphanVs, Vs};

#[allow(non_snake_case)]
#[derive(Vs, Deserialize, Serialize)]
pub(crate) struct State {
    pub(crate) chain_id: U256,
    pub(crate) gas_price: OrphanVs<U256>,
    pub(crate) block_gas_limit: OrphanVs<U256>,
    pub(crate) block_base_fee_per_gas: OrphanVs<U256>,
    pub(crate) block_coinbase: H160,
    pub(crate) block_timestamp: U256,

    pub(crate) OVR: Erc20Like,
    pub(crate) OVRG: Erc20Like,

    // `BlockHeight => H256(original block hash)`
    pub(crate) block_hashes: MapxOrd<BlockHeight, H256>,

    // Vivinity values in oneshot mode.
    vicinity: OvrVicinity,
}

impl State {
    #[inline(always)]
    fn get_backend_hdr<'a>(&'a self, branch: BranchName<'a>) -> OvrBackend<'a> {
        OvrBackend::new(self.OVRG.accounts.clone(), &self.vicinity, branch)
    }

    // update with each new block
    #[inline(always)]
    pub(crate) fn update_vicinity(&mut self) {
        self.vicinity = OvrVicinity {
            gas_price: self.gas_price.get_value(),
            origin: H160::zero(),
            chain_id: self.chain_id,
            // this is a lightweight copy
            block_hashes: self.block_hashes,
            block_number: U256::from(
                self.block_hashes.last().map(|(h, _)| h).unwrap_or(0),
            ),
            block_coinbase: self.block_coinbase,
            block_timestamp: self.block_timestamp,
            block_difficulty: U256::zero(),
            block_gas_limit: self.block_gas_limit.get_value(),
            block_base_fee_per_gas: self.block_base_fee_per_gas.get_value(),
        };
    }
}

// Account information of a vsdb backend.
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct OvrAccount {
    // Account nonce.
    pub(crate) nonce: U256,
    // Account balance, OVRG token.
    pub(crate) balance: U256,
    // Full account storage used by evm.
    pub(crate) storage: Mapx<H256, H256>,
    // Account code.
    pub(crate) code: Vec<u8>,
}

#[derive(Vs, Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct OvrVicinity {
    pub(crate) gas_price: U256,
    pub(crate) origin: H160,
    pub(crate) chain_id: U256,
    // Environmental block hashes.
    // `BlockHeight => H256(original block hash)`
    pub(crate) block_hashes: MapxOrd<BlockHeight, H256>,
    // Environmental block number.
    pub(crate) block_number: U256,
    // Environmental coinbase.
    // `H160(original proposer address)`
    pub(crate) block_coinbase: H160,
    // Environmental block timestamp.
    pub(crate) block_timestamp: U256,
    // Environmental block difficulty.
    pub(crate) block_difficulty: U256,
    // Environmental block gas limit.
    pub(crate) block_gas_limit: U256,
    // Environmental base fee per gas.
    pub(crate) block_base_fee_per_gas: U256,
}

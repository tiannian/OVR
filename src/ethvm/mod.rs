mod impls;
mod precompile;
pub mod tx;

use crate::ledger::BlockHeight;
use evm::backend::Log;
use impls::backend::OvrBackend;
use primitive_types::{H160, H256, U256};
use serde::{Deserialize, Serialize};
use vsdb::{Mapx, MapxOrd, MapxVs, OrphanVs, Vecx, Vs};

pub(crate) struct State {
    chain_id: OrphanVs<U256>,
    gas_price: OrphanVs<U256>,
    block_gas_limit: OrphanVs<U256>,
    block_base_fee_per_gas: OrphanVs<U256>,
    block_coinbase: OrphanVs<H160>,
    block_timestamp: OrphanVs<U256>,

    // accounts data of evm side
    accounts: MapxVs<H160, OvrAccount>,

    // `BlockHeight => H256(original block hash)`
    block_hashes: MapxOrd<BlockHeight, H256>,

    // Vivinity values in oneshot mode.
    vicinity: OvrVicinity,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

// Account information of a vsdb backend.
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct OvrAccount {
    // Account nonce.
    pub(crate) nonce: U256,
    // Account balance, OVRG token.
    pub(crate) balance: U256,
    // Full account storage used by evm.
    pub(crate) storage: MapxVs<H256, H256>,
    // Account code.
    pub(crate) code: Vec<u8>,
}

impl State {
    fn gen_backend_hdr(&mut self) -> OvrBackend {
        self.update_vicinity();
        OvrBackend {
            // a copy actually
            state: self.accounts.clone(),
            vicinity: &self.vicinity,
        }
    }

    fn update_vicinity(&mut self) {
        self.vicinity = OvrVicinity {
            gas_price: self.gas_price.get_value(),
            origin: H160::zero(),
            chain_id: self.chain_id.get_value(),
            // a copy actually
            block_hashes: self.block_hashes.clone(),
            block_number: U256::from(
                self.block_hashes.last().map(|(h, _)| h).unwrap_or(0),
            ),
            block_coinbase: self.block_coinbase.get_value(),
            block_timestamp: self.block_timestamp.get_value(),
            block_difficulty: U256::zero(),
            block_gas_limit: self.block_gas_limit.get_value(),
            block_base_fee_per_gas: self.block_base_fee_per_gas.get_value(),
        };
    }
}

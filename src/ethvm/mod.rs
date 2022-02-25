mod impls;
mod precompile;
pub mod tx;

use crate::common::BlockHeight;
use impls::backend::OvrBackend;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use tx::meta_tokens::Erc20Like;
use vsdb::{BranchName, MapxOrd, OrphanVs, Vs};

#[allow(non_snake_case)]
#[derive(Vs, Clone, Debug, Deserialize, Serialize)]
pub struct State {
    pub gas_price: OrphanVs<U256>,
    pub block_gas_limit: OrphanVs<U256>,
    pub block_base_fee_per_gas: OrphanVs<U256>,

    pub OFUEL: Erc20Like,

    // Environmental block hashes.
    pub block_hashes: MapxOrd<BlockHeight, H256>,

    // Oneshot values for each evm transaction.
    pub vicinity: OvrVicinity,
}

impl State {
    #[inline(always)]
    fn get_backend_hdr<'a>(&self, branch: BranchName<'a>) -> OvrBackend<'a> {
        OvrBackend {
            branch,
            state: self.OFUEL.accounts.clone(),
            storages: self.OFUEL.storages.clone(),
            block_hashes: self.block_hashes,
            vicinity: self.vicinity.clone(),
        }
    }

    // update with each new block
    #[inline(always)]
    pub fn update_vicinity(
        &mut self,
        chain_id: U256,
        block_coinbase: H160,
        block_timestamp: U256,
    ) {
        self.vicinity = OvrVicinity {
            gas_price: self.gas_price.get_value(),
            origin: H160::zero(),
            chain_id,
            block_number: U256::from(
                self.block_hashes.last().map(|(h, _)| h).unwrap_or(0),
            ),
            block_coinbase,
            block_timestamp,
            block_difficulty: U256::zero(),
            block_gas_limit: self.block_gas_limit.get_value(),
            block_base_fee_per_gas: self.block_base_fee_per_gas.get_value(),
        };
    }

    // fn get_token_hdr(&self, contract_addr: H160) -> &Erc20Like {
    //     if Erc20Like::addr_is_ofuel(contract_addr) {
    //         &self.OFUEL
    //     } else {
    //         unreachable!()
    //     }
    // }

    #[inline(always)]
    fn get_token_hdr_mut(&mut self, contract_addr: H160) -> &mut Erc20Like {
        if Erc20Like::addr_is_ofuel(contract_addr) {
            &mut self.OFUEL
        } else {
            unreachable!()
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            gas_price: OrphanVs::default(),
            block_gas_limit: OrphanVs::default(),
            block_base_fee_per_gas: OrphanVs::default(),
            OFUEL: Erc20Like::ofuel_token(),
            block_hashes: MapxOrd::default(),
            vicinity: OvrVicinity::default(),
        }
    }
}

// Account information of a vsdb backend.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct OvrAccount {
    pub nonce: U256,
    pub balance: U256,
    pub code: Vec<u8>,
}

#[derive(Vs, Default, Clone, Debug, Serialize, Deserialize)]
pub struct OvrVicinity {
    pub gas_price: U256,
    pub origin: H160,
    pub chain_id: U256,
    // Environmental block number.
    pub block_number: U256,
    // Environmental coinbase.
    // `H160(original proposer address)`
    pub block_coinbase: H160,
    // Environmental block timestamp.
    pub block_timestamp: U256,
    // Environmental block difficulty.
    pub block_difficulty: U256,
    // Environmental block gas limit.
    pub block_gas_limit: U256,
    // Environmental base fee per gas.
    pub block_base_fee_per_gas: U256,
}

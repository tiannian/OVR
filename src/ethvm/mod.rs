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
pub(crate) struct State {
    pub(crate) chain_id: U256,
    pub(crate) gas_price: OrphanVs<U256>,
    pub(crate) block_gas_limit: OrphanVs<U256>,
    pub(crate) block_base_fee_per_gas: OrphanVs<U256>,
    pub(crate) block_coinbase: H160,
    pub(crate) block_timestamp: U256,

    pub(crate) OVR: Erc20Like,
    pub(crate) OVRG: Erc20Like,

    // Environmental block hashes.
    pub(crate) block_hashes: MapxOrd<BlockHeight, H256>,

    // Oneshot values for each evm transaction.
    pub(crate) vicinity: OvrVicinity,
}

impl State {
    #[inline(always)]
    fn get_backend_hdr<'a>(&self, branch: BranchName<'a>) -> OvrBackend<'a> {
        OvrBackend {
            branch,
            state: self.OVRG.accounts.clone(),
            storages: self.OVRG.storages.clone(),
            block_hashes: self.block_hashes,
            vicinity: self.vicinity.clone(),
        }
    }

    // update with each new block
    #[inline(always)]
    pub(crate) fn update_vicinity(&mut self) {
        self.vicinity = OvrVicinity {
            gas_price: self.gas_price.get_value(),
            origin: H160::zero(),
            chain_id: self.chain_id,
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

    // fn get_token_hdr(&self, contract_addr: H160) -> &Erc20Like {
    //     if Erc20Like::addr_is_ovr(contract_addr) {
    //         &self.OVR
    //     } else if Erc20Like::addr_is_ovrg(contract_addr) {
    //         &self.OVRG
    //     } else {
    //         unreachable!()
    //     }
    // }

    #[inline(always)]
    fn get_token_hdr_mut(&mut self, contract_addr: H160) -> &mut Erc20Like {
        if Erc20Like::addr_is_ovr(contract_addr) {
            &mut self.OVR
        } else if Erc20Like::addr_is_ovrg(contract_addr) {
            &mut self.OVRG
        } else {
            unreachable!()
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            OVR: Erc20Like::ovr_token(),
            OVRG: Erc20Like::ovrg_token(),
            ..Default::default()
        }
    }
}

// Account information of a vsdb backend.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct OvrAccount {
    pub(crate) nonce: U256,
    pub(crate) balance: U256,
    pub(crate) code: Vec<u8>,
}

#[derive(Vs, Default, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct OvrVicinity {
    pub(crate) gas_price: U256,
    pub(crate) origin: H160,
    pub(crate) chain_id: U256,
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

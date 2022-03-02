mod impls;
mod precompile;
pub mod tx;

use crate::{
    common::{block_number_to_height, rollback_to_height, BlockHeight},
    ethvm::{impls::stack::OvrStackState, precompile::PRECOMPILE_SET},
};
use evm::{
    executor::stack::{StackExecutor, StackSubstateMetadata},
    ExitReason,
};
use impls::backend::OvrBackend;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use tx::meta_tokens::Erc20Like;
use vsdb::{BranchName, MapxOrd, OrphanVs, Vs, VsMgmt};
use web3_rpc_core::types::{BlockNumber, CallRequest};

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

#[derive(Debug, Deserialize, Serialize)]
pub struct CallContractResp {
    pub evm_resp: ExitReason,
    pub data: Vec<u8>,
    pub gas_used: u64,
}

impl State {
    pub fn call_contract(
        &self,
        branch_name: BranchName,
        req: CallRequest,
        bn: Option<BlockNumber>,
    ) -> Result<CallContractResp> {
        let caller = if let Some(from) = req.from {
            from
        } else {
            return Err(eg!("caller cannot be empty"));
        };
        let address = if let Some(addr) = req.to {
            addr
        } else {
            return Err(eg!("to cannot be empty"));
        };
        let value = if let Some(val) = req.value {
            val
        } else {
            return Err(eg!("value cannot be empty"));
        };
        let data = if let Some(data) = req.data {
            data
        } else {
            return Err(eg!("data cannot be empty"));
        };
        let gas_price = if let Some(gas_price) = req.gas_price {
            gas_price
        } else {
            return Err(eg!("gas_price cannot be empty"));
        };
        let gas = if let Some(gas) = req.gas {
            gas
        } else {
            return Err(eg!("gas cannot be empty"));
        };
        let gas_limit = gas.checked_div(gas_price).unwrap().as_u64();

        let height = block_number_to_height(bn, None, Some(self));

        let new_branch_name =
            rollback_to_height(height, None, Some(self), "call_contract")?;

        let backend = OvrBackend {
            branch: branch_name,
            state: self.OFUEL.accounts.clone(),
            storages: self.OFUEL.storages.clone(),
            block_hashes: self.block_hashes,
            vicinity: self.vicinity.clone(),
        };

        let cfg = evm::Config::istanbul();
        let metadata = StackSubstateMetadata::new(u64::MAX, &cfg);

        let ovr_stack_state = OvrStackState::new(metadata, &backend);
        let precompiles = PRECOMPILE_SET.clone();
        let mut executor =
            StackExecutor::new_with_precompiles(ovr_stack_state, &cfg, &precompiles);

        let resp =
            executor.transact_call(caller, address, value, data.0, gas_limit, vec![]);

        ruc::d!(format!("{:?}", resp));

        let cc_resp = CallContractResp {
            evm_resp: resp.0,
            data: resp.1,
            gas_used: executor.used_gas(),
        };

        self.branch_remove(BranchName::from(new_branch_name.as_str()))?;
        Ok(cc_resp)
    }

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

impl OvrAccount {
    pub fn from_balance(balance: U256) -> Self {
        Self {
            balance,
            ..Default::default()
        }
    }
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

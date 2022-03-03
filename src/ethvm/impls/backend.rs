//!
//! # Implementations for evm backend
//!
//! Ported from [evm](evm::executor::stack::memory).
//!

use crate::{
    common::BlockHeight,
    ethvm::{OvrAccount, OvrVicinity},
};
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use primitive_types::{H160, H256, U256};
use ruc::*;
use vsdb::{BranchName, MapxDkVs, MapxOrd, MapxVs};

// Ovr backend, storing all state values in vsdb.
#[derive(Clone, Debug)]
pub struct OvrBackend<'a> {
    pub(crate) branch: BranchName<'a>,
    pub(crate) state: MapxVs<H160, OvrAccount>,
    pub(crate) storages: MapxDkVs<H160, H256, H256>,
    pub(crate) block_hashes: MapxOrd<BlockHeight, H256>,
    pub(crate) vicinity: OvrVicinity,
}

impl<'a> OvrBackend<'a> {
    #[inline(always)]
    fn reset_storage(&self, target: H160, b: BranchName) {
        pnk!(self.storages.remove_by_branch(&(&target, None), b));
    }
}

impl<'a> Backend for OvrBackend<'a> {
    #[inline(always)]
    fn gas_price(&self) -> U256 {
        self.vicinity.gas_price
    }

    #[inline(always)]
    fn origin(&self) -> H160 {
        self.vicinity.origin
    }

    #[inline(always)]
    fn block_hash(&self, number: U256) -> H256 {
        self.block_hashes.get(&number.as_u64()).unwrap_or_default()
    }

    #[inline(always)]
    fn block_number(&self) -> U256 {
        self.vicinity.block_number
    }

    #[inline(always)]
    fn block_coinbase(&self) -> H160 {
        self.vicinity.block_coinbase
    }

    #[inline(always)]
    fn block_timestamp(&self) -> U256 {
        self.vicinity.block_timestamp
    }

    #[inline(always)]
    fn block_difficulty(&self) -> U256 {
        self.vicinity.block_difficulty
    }

    #[inline(always)]
    fn block_gas_limit(&self) -> U256 {
        self.vicinity.block_gas_limit
    }

    #[inline(always)]
    fn block_base_fee_per_gas(&self) -> U256 {
        self.vicinity.block_base_fee_per_gas
    }

    #[inline(always)]
    fn chain_id(&self) -> U256 {
        self.vicinity.chain_id
    }

    #[inline(always)]
    fn exists(&self, address: H160) -> bool {
        self.state.contains_key_by_branch(&address, self.branch)
    }

    #[inline(always)]
    fn basic(&self, address: H160) -> Basic {
        self.state
            .get_by_branch(&address, self.branch)
            .map(|a| Basic {
                balance: a.balance,
                nonce: a.nonce,
            })
            .unwrap_or_default()
    }

    #[inline(always)]
    fn code(&self, address: H160) -> Vec<u8> {
        self.state
            .get_by_branch(&address, self.branch)
            .map(|v| v.code)
            .unwrap_or_default()
    }

    #[inline(always)]
    fn storage(&self, address: H160, index: H256) -> H256 {
        self.storages
            .get_by_branch(&(&address, &index), self.branch)
            .unwrap_or_default()
    }

    #[inline(always)]
    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        Some(self.storage(address, index))
    }
}

impl<'a> ApplyBackend for OvrBackend<'a> {
    fn apply<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool)
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
        L: IntoIterator<Item = Log>,
    {
        for apply in values {
            match apply {
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage,
                } => {
                    let is_empty = {
                        let mut account = self
                            .state
                            .get_by_branch(&address, self.branch)
                            .unwrap_or_default();
                        account.balance = basic.balance;
                        account.nonce = basic.nonce;
                        if let Some(code) = code {
                            account.code = code;
                        }

                        if reset_storage {
                            self.reset_storage(address, self.branch);
                        }

                        for (index, value) in storage {
                            if value != H256::default() {
                                self.storages.insert((address, index), value).unwrap();
                            }
                        }

                        let ret = account.balance == U256::zero()
                            && account.nonce == U256::zero()
                            && account.code.is_empty();

                        self.state
                            .insert_by_branch(address, account, self.branch)
                            .unwrap();

                        ret
                    };

                    if is_empty && delete_empty {
                        self.state.remove_by_branch(&address, self.branch).unwrap();
                    }
                }
                Apply::Delete { address } => {
                    self.state.remove_by_branch(&address, self.branch).unwrap();
                }
            }
        }

        for log in logs {
            ruc::pd!(serde_json::to_string(&log).unwrap());
        }
    }
}

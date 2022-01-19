//!
//! # Implementations for evm backend
//!
//! Ported from [evm](evm::executor::stack::memory).
//!

use crate::ethvm::{OvrAccount, OvrVicinity};
use evm::backend::{Apply, ApplyBackend, Backend, Basic, Log};
use primitive_types::{H160, H256, U256};
use serde::{Deserialize, Serialize};
use vsdb::{Mapx, MapxVs, Vecx};

// Ovr backend, storing all state values in vsdb.
#[derive(Clone, Debug)]
pub(crate) struct OvrBackend<'vicinity> {
    pub(crate) state: MapxVs<H160, OvrAccount>,
    pub(crate) vicinity: &'vicinity OvrVicinity,
}

impl<'vicinity> OvrBackend<'vicinity> {
    // Create a new vsdb backend.
    pub(crate) fn new(
        vicinity: &'vicinity OvrVicinity,
        state: MapxVs<H160, OvrAccount>,
    ) -> Self {
        Self { vicinity, state }
    }

    // Get the underlying `MapxVs` storing the state.
    pub(crate) fn state(&self) -> &MapxVs<H160, OvrAccount> {
        &self.state
    }

    // Get a mutable reference to the underlying `MapxVs` storing the state.
    pub(crate) fn state_mut(&mut self) -> &mut MapxVs<H160, OvrAccount> {
        &mut self.state
    }
}

impl<'vicinity> Backend for OvrBackend<'vicinity> {
    fn gas_price(&self) -> U256 {
        self.vicinity.gas_price
    }
    fn origin(&self) -> H160 {
        self.vicinity.origin
    }
    fn block_hash(&self, number: U256) -> H256 {
        self.vicinity
            .block_hashes
            .get(&number.as_u64())
            .unwrap_or_default()
    }
    fn block_number(&self) -> U256 {
        self.vicinity.block_number
    }
    fn block_coinbase(&self) -> H160 {
        self.vicinity.block_coinbase
    }
    fn block_timestamp(&self) -> U256 {
        self.vicinity.block_timestamp
    }
    fn block_difficulty(&self) -> U256 {
        self.vicinity.block_difficulty
    }
    fn block_gas_limit(&self) -> U256 {
        self.vicinity.block_gas_limit
    }
    fn block_base_fee_per_gas(&self) -> U256 {
        self.vicinity.block_base_fee_per_gas
    }

    fn chain_id(&self) -> U256 {
        self.vicinity.chain_id
    }

    fn exists(&self, address: H160) -> bool {
        self.state.contains_key(&address)
    }

    fn basic(&self, address: H160) -> Basic {
        self.state
            .get(&address)
            .map(|a| Basic {
                balance: a.balance,
                nonce: a.nonce,
            })
            .unwrap_or_default()
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.state
            .get(&address)
            .map(|v| v.code.clone())
            .unwrap_or_default()
    }

    fn storage(&self, address: H160, index: H256) -> H256 {
        self.state
            .get(&address)
            .map(|v| v.storage.get(&index).map(|v| v.clone()).unwrap_or_default())
            .unwrap_or_default()
    }

    fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
        Some(self.storage(address, index))
    }
}

impl<'vicinity> ApplyBackend for OvrBackend<'vicinity> {
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
                            .entry_ref(&address)
                            .or_insert_ref(&Default::default());
                        account.balance = basic.balance;
                        account.nonce = basic.nonce;
                        if let Some(code) = code {
                            account.code = code;
                        }

                        if reset_storage {
                            account.storage.clear();
                        }

                        let zeros = account
                            .storage
                            .iter()
                            .filter(|(_, v)| v == &H256::default())
                            .map(|(k, _)| k)
                            .collect::<Vec<H256>>();

                        for zero in zeros {
                            account.storage.remove(&zero).unwrap();
                        }

                        for (index, value) in storage {
                            if value == H256::default() {
                                account.storage.remove(&index).unwrap();
                            } else {
                                account.storage.insert(index, value).unwrap();
                            }
                        }

                        account.balance == U256::zero()
                            && account.nonce == U256::zero()
                            && account.code.is_empty()
                    };

                    if is_empty && delete_empty {
                        self.state.remove(&address).unwrap();
                    }
                }
                Apply::Delete { address } => {
                    self.state.remove(&address).unwrap();
                }
            }
        }

        for log in logs {
            ruc::pd!(serde_json::to_string(&log).unwrap());
        }
    }
}

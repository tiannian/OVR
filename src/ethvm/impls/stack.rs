//!
//! # Implementations for evm stack
//!
//! Ported from [evm](evm::executor::stack::memory).
//!

use evm::{
    backend::{Apply, Backend, Basic, Log},
    executor::stack::{Accessed, StackState, StackSubstateMetadata},
    ExitError, Transfer,
};
use primitive_types::{H160, H256, U256};
use std::{
    collections::{BTreeMap, BTreeSet},
    mem,
};

#[derive(Clone, Debug)]
pub(crate) struct OvrStackAccount {
    pub(crate) basic: Basic,
    pub(crate) code: Option<Vec<u8>>,
    pub(crate) reset: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct OvrStackSubstate<'config> {
    metadata: StackSubstateMetadata<'config>,
    parent: Option<Box<OvrStackSubstate<'config>>>,
    accounts: BTreeMap<H160, OvrStackAccount>,
    storages: BTreeMap<(H160, H256), H256>,
    deletes: BTreeSet<H160>,
    logs: Vec<Log>,
}

impl<'config> OvrStackSubstate<'config> {
    pub(crate) fn new(metadata: StackSubstateMetadata<'config>) -> Self {
        Self {
            metadata,
            parent: None,
            logs: Vec::new(),
            accounts: BTreeMap::new(),
            storages: BTreeMap::new(),
            deletes: BTreeSet::new(),
        }
    }

    // pub(crate) fn logs(&self) -> &[Log] {
    //     &self.logs
    // }

    // pub(crate) fn logs_mut(&mut self) -> &mut Vec<Log> {
    //     &mut self.logs
    // }

    pub(crate) fn metadata(&self) -> &StackSubstateMetadata<'config> {
        &self.metadata
    }

    pub(crate) fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
        &mut self.metadata
    }

    // Deconstruct the executor, return state to be applied. Panic if the
    // executor is not in the top-level substate.
    #[must_use]
    pub(crate) fn deconstruct<B: Backend>(
        mut self,
        backend: &B,
    ) -> (Vec<Apply<BTreeMap<H256, H256>>>, Vec<Log>) {
        assert!(self.parent.is_none());

        let mut applies = Vec::<Apply<BTreeMap<H256, H256>>>::new();

        let mut addresses = BTreeSet::new();

        for address in self.accounts.keys() {
            addresses.insert(*address);
        }

        for (address, _) in self.storages.keys() {
            addresses.insert(*address);
        }

        for address in addresses {
            if self.deletes.contains(&address) {
                continue;
            }

            let apply = {
                let OvrStackAccount { basic, code, reset } =
                    self.account_mut(address, backend).clone();
                let storage = self
                    .storages
                    .iter()
                    .filter(|((a, _), _)| a == &address)
                    .fold(BTreeMap::new(), |mut acc, ((_, k), v)| {
                        acc.insert(*k, *v);
                        acc
                    });
                Apply::Modify {
                    address,
                    basic,
                    code,
                    storage,
                    reset_storage: reset,
                }
            };

            applies.push(apply);
        }

        for address in self.deletes {
            applies.push(Apply::Delete { address });
        }

        (applies, self.logs)
    }

    pub(crate) fn enter(&mut self, gas_limit: u64, is_static: bool) {
        let mut entering = Self {
            metadata: self.metadata.spit_child(gas_limit, is_static),
            parent: None,
            logs: Vec::new(),
            accounts: BTreeMap::new(),
            storages: BTreeMap::new(),
            deletes: BTreeSet::new(),
        };
        mem::swap(&mut entering, self);

        self.parent = Some(Box::new(entering));
    }

    pub(crate) fn exit_commit(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot commit on root substate");
        mem::swap(&mut exited, self);

        self.metadata.swallow_commit(exited.metadata)?;
        self.logs.append(&mut exited.logs);

        let mut resets = BTreeSet::new();
        for (address, account) in &exited.accounts {
            if account.reset {
                resets.insert(*address);
            }
        }
        let mut reset_keys = BTreeSet::new();
        for (address, key) in self.storages.keys() {
            if resets.contains(address) {
                reset_keys.insert((*address, *key));
            }
        }
        for (address, key) in reset_keys {
            self.storages.remove(&(address, key));
        }

        self.accounts.append(&mut exited.accounts);
        self.storages.append(&mut exited.storages);
        self.deletes.append(&mut exited.deletes);

        Ok(())
    }

    pub(crate) fn exit_revert(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot discard on root substate");
        mem::swap(&mut exited, self);
        self.metadata.swallow_revert(exited.metadata)
    }

    pub(crate) fn exit_discard(&mut self) -> Result<(), ExitError> {
        let mut exited = *self.parent.take().expect("Cannot discard on root substate");
        mem::swap(&mut exited, self);
        self.metadata.swallow_discard(exited.metadata)
    }

    pub(crate) fn known_account(&self, address: H160) -> Option<&OvrStackAccount> {
        if let Some(account) = self.accounts.get(&address) {
            Some(account)
        } else if let Some(parent) = self.parent.as_ref() {
            parent.known_account(address)
        } else {
            None
        }
    }

    pub(crate) fn known_basic(&self, address: H160) -> Option<Basic> {
        self.known_account(address).map(|acc| acc.basic.clone())
    }

    pub(crate) fn known_code(&self, address: H160) -> Option<Vec<u8>> {
        self.known_account(address).and_then(|acc| acc.code.clone())
    }

    pub(crate) fn known_empty(&self, address: H160) -> Option<bool> {
        if let Some(account) = self.known_account(address) {
            if account.basic.balance != U256::zero() {
                return Some(false);
            }
            if account.basic.nonce != U256::zero() {
                return Some(false);
            }
            if let Some(code) = &account.code {
                return Some(
                    account.basic.balance == U256::zero()
                        && account.basic.nonce == U256::zero()
                        && code.is_empty(),
                );
            }
        }
        None
    }

    pub(crate) fn known_storage(&self, address: H160, key: H256) -> Option<H256> {
        if let Some(value) = self.storages.get(&(address, key)) {
            return Some(*value);
        }
        if let Some(account) = self.accounts.get(&address) {
            if account.reset {
                return Some(H256::default());
            }
        }
        if let Some(parent) = self.parent.as_ref() {
            return parent.known_storage(address, key);
        }
        None
    }

    pub(crate) fn known_original_storage(
        &self,
        address: H160,
        key: H256,
    ) -> Option<H256> {
        if let Some(account) = self.accounts.get(&address) {
            if account.reset {
                return Some(H256::default());
            }
        }
        if let Some(parent) = self.parent.as_ref() {
            return parent.known_original_storage(address, key);
        }
        None
    }

    pub(crate) fn is_cold(&self, address: H160) -> bool {
        self.recursive_is_cold(&|a| a.accessed_addresses.contains(&address))
    }

    pub(crate) fn is_storage_cold(&self, address: H160, key: H256) -> bool {
        self.recursive_is_cold(&|a: &Accessed| {
            a.accessed_storage.contains(&(address, key))
        })
    }

    fn recursive_is_cold<F: Fn(&Accessed) -> bool>(&self, f: &F) -> bool {
        let local_is_accessed =
            self.metadata.accessed().as_ref().map(f).unwrap_or(false);
        if local_is_accessed {
            false
        } else {
            self.parent
                .as_ref()
                .map(|p| p.recursive_is_cold(f))
                .unwrap_or(true)
        }
    }

    pub(crate) fn deleted(&self, address: H160) -> bool {
        if self.deletes.contains(&address) {
            return true;
        }
        if let Some(parent) = self.parent.as_ref() {
            return parent.deleted(address);
        }
        false
    }

    fn account_mut<B: Backend>(
        &mut self,
        address: H160,
        backend: &B,
    ) -> &mut OvrStackAccount {
        let a = self.known_account(address).cloned();
        self.accounts.entry(address).or_insert_with(|| {
            a.map(|mut v| {
                v.reset = false;
                v
            })
            .unwrap_or_else(|| OvrStackAccount {
                basic: backend.basic(address),
                code: None,
                reset: false,
            })
        })
    }

    pub(crate) fn inc_nonce<B: Backend>(&mut self, address: H160, backend: &B) {
        self.account_mut(address, backend).basic.nonce += U256::one();
    }

    pub(crate) fn set_storage(&mut self, address: H160, key: H256, value: H256) {
        self.storages.insert((address, key), value);
    }

    pub(crate) fn reset_storage<B: Backend>(&mut self, address: H160, backend: &B) {
        self.storages
            .keys()
            .filter(|(a, _)| a == &address)
            .fold(vec![], |mut acc, new| {
                acc.push(*new);
                acc
            })
            .iter()
            .for_each(|k| {
                self.storages.remove(k);
            });

        self.account_mut(address, backend).reset = true;
    }

    pub(crate) fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.logs.push(Log {
            address,
            topics,
            data,
        });
    }

    pub(crate) fn set_deleted(&mut self, address: H160) {
        self.deletes.insert(address);
    }

    pub(crate) fn set_code<B: Backend>(
        &mut self,
        address: H160,
        code: Vec<u8>,
        backend: &B,
    ) {
        self.account_mut(address, backend).code = Some(code);
    }

    pub(crate) fn transfer<B: Backend>(
        &mut self,
        transfer: Transfer,
        backend: &B,
    ) -> Result<(), ExitError> {
        let source = self.account_mut(transfer.source, backend);
        if source.basic.balance < transfer.value {
            return Err(ExitError::OutOfFund);
        }
        source.basic.balance -= transfer.value;

        let target = self.account_mut(transfer.target, backend);
        target.basic.balance = target.basic.balance.saturating_add(transfer.value);

        Ok(())
    }

    // // Only needed for jsontests.
    // pub(crate) fn withdraw<B: Backend>(
    //     &mut self,
    //     address: H160,
    //     value: U256,
    //     backend: &B,
    // ) -> Result<(), ExitError> {
    //     let source = self.account_mut(address, backend);
    //     if source.basic.balance < value {
    //         return Err(ExitError::OutOfFund);
    //     }
    //     source.basic.balance -= value;
    //     Ok(())
    // }

    // // Only needed for jsontests.
    // pub(crate) fn deposit<B: Backend>(
    //     &mut self,
    //     address: H160,
    //     value: U256,
    //     backend: &B,
    // ) {
    //     let target = self.account_mut(address, backend);
    //     target.basic.balance = target.basic.balance.saturating_add(value);
    // }

    pub(crate) fn reset_balance<B: Backend>(&mut self, address: H160, backend: &B) {
        self.account_mut(address, backend).basic.balance = U256::zero();
    }

    pub(crate) fn touch<B: Backend>(&mut self, address: H160, backend: &B) {
        self.account_mut(address, backend);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OvrStackState<'backend, 'config, B> {
    backend: &'backend B,
    substate: OvrStackSubstate<'config>,
}

impl<'backend, 'config, B: Backend> Backend for OvrStackState<'backend, 'config, B> {
    fn gas_price(&self) -> U256 {
        self.backend.gas_price()
    }
    fn origin(&self) -> H160 {
        self.backend.origin()
    }
    fn block_hash(&self, number: U256) -> H256 {
        self.backend.block_hash(number)
    }
    fn block_number(&self) -> U256 {
        self.backend.block_number()
    }
    fn block_coinbase(&self) -> H160 {
        self.backend.block_coinbase()
    }
    fn block_timestamp(&self) -> U256 {
        self.backend.block_timestamp()
    }
    fn block_difficulty(&self) -> U256 {
        self.backend.block_difficulty()
    }
    fn block_gas_limit(&self) -> U256 {
        self.backend.block_gas_limit()
    }
    fn block_base_fee_per_gas(&self) -> U256 {
        self.backend.block_base_fee_per_gas()
    }

    fn chain_id(&self) -> U256 {
        self.backend.chain_id()
    }

    fn exists(&self, address: H160) -> bool {
        self.substate.known_account(address).is_some() || self.backend.exists(address)
    }

    fn basic(&self, address: H160) -> Basic {
        self.substate
            .known_basic(address)
            .unwrap_or_else(|| self.backend.basic(address))
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.substate
            .known_code(address)
            .unwrap_or_else(|| self.backend.code(address))
    }

    fn storage(&self, address: H160, key: H256) -> H256 {
        self.substate
            .known_storage(address, key)
            .unwrap_or_else(|| self.backend.storage(address, key))
    }

    fn original_storage(&self, address: H160, key: H256) -> Option<H256> {
        if let Some(value) = self.substate.known_original_storage(address, key) {
            return Some(value);
        }
        self.backend.original_storage(address, key)
    }
}

impl<'backend, 'config, B: Backend> StackState<'config>
    for OvrStackState<'backend, 'config, B>
{
    fn metadata(&self) -> &StackSubstateMetadata<'config> {
        self.substate.metadata()
    }

    fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
        self.substate.metadata_mut()
    }

    fn enter(&mut self, gas_limit: u64, is_static: bool) {
        self.substate.enter(gas_limit, is_static)
    }

    fn exit_commit(&mut self) -> Result<(), ExitError> {
        self.substate.exit_commit()
    }

    fn exit_revert(&mut self) -> Result<(), ExitError> {
        self.substate.exit_revert()
    }

    fn exit_discard(&mut self) -> Result<(), ExitError> {
        self.substate.exit_discard()
    }

    fn is_empty(&self, address: H160) -> bool {
        if let Some(known_empty) = self.substate.known_empty(address) {
            return known_empty;
        }
        self.backend.basic(address).balance == U256::zero()
            && self.backend.basic(address).nonce == U256::zero()
            && self.backend.code(address).len() == 0
    }

    fn deleted(&self, address: H160) -> bool {
        self.substate.deleted(address)
    }

    fn is_cold(&self, address: H160) -> bool {
        self.substate.is_cold(address)
    }

    fn is_storage_cold(&self, address: H160, key: H256) -> bool {
        self.substate.is_storage_cold(address, key)
    }

    fn inc_nonce(&mut self, address: H160) {
        self.substate.inc_nonce(address, self.backend);
    }

    fn set_storage(&mut self, address: H160, key: H256, value: H256) {
        self.substate.set_storage(address, key, value)
    }

    fn reset_storage(&mut self, address: H160) {
        self.substate.reset_storage(address, self.backend);
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
        self.substate.log(address, topics, data);
    }

    fn set_deleted(&mut self, address: H160) {
        self.substate.set_deleted(address)
    }

    fn set_code(&mut self, address: H160, code: Vec<u8>) {
        self.substate.set_code(address, code, self.backend)
    }

    fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
        self.substate.transfer(transfer, self.backend)
    }

    fn reset_balance(&mut self, address: H160) {
        self.substate.reset_balance(address, self.backend)
    }

    fn touch(&mut self, address: H160) {
        self.substate.touch(address, self.backend)
    }
}

impl<'backend, 'config, B: Backend> OvrStackState<'backend, 'config, B> {
    pub(crate) fn new(
        metadata: StackSubstateMetadata<'config>,
        backend: &'backend B,
    ) -> Self {
        Self {
            backend,
            substate: OvrStackSubstate::new(metadata),
        }
    }

    #[must_use]
    pub(crate) fn deconstruct(self) -> (Vec<Apply<BTreeMap<H256, H256>>>, Vec<Log>) {
        self.substate.deconstruct(self.backend)
    }

    // pub(crate) fn withdraw(
    //     &mut self,
    //     address: H160,
    //     value: U256,
    // ) -> Result<(), ExitError> {
    //     self.substate.withdraw(address, value, self.backend)
    // }

    // pub(crate) fn deposit(&mut self, address: H160, value: U256) {
    //     self.substate.deposit(address, value, self.backend)
    // }
}

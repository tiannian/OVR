use super::{impls::stack::OvrStackState, OvrAccount};
use crate::ledger::StateBranch;
use ethereum::{TransactionAction, TransactionAny};
use evm::{
    backend::ApplyBackend,
    executor::stack::{StackExecutor, StackSubstateMetadata},
    Config as EvmCfg, ExitReason,
};
use once_cell::sync::Lazy;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::{collections::BTreeMap, result::Result as StdResult};
use vsdb::BranchName;

static EVM_CFG: Lazy<EvmCfg> = Lazy::new(|| EvmCfg::istanbul());
static GAS_PRICE_MIN: Lazy<U256> = Lazy::new(|| U256::from(10u8));

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    tx: TransactionAny,
}

impl Tx {
    pub(crate) fn apply(
        self,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> StdResult<(), Option<(H160, U256)>> {
        if let Ok((addr, mut a, gas_limit, gas_price)) = ruc::info!(self.pre_exec(sb, b))
        {
            match self.exec(addr, sb, b) {
                Ok(gas_used) => {
                    a.balance += gas_price.saturating_mul(gas_limit - gas_used);
                    sb.state.evm.accounts.insert_by_branch(addr, a, b).unwrap();
                    Ok(())
                }
                Err(gas_used) => Err(Some((addr, gas_price.saturating_mul(gas_used)))),
            }
        } else {
            Err(None)
        }
    }

    // 1. verify the transaction signature
    // 2. ensure the transaction nonce is bigger than the last nonce
    // 3. ensure the balance of OVRG is bigger than `spent_amount + gas_limit`
    // 4. deducte `gas_limit` from the balance of OVRG
    fn pre_exec(
        &self,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> Result<(H160, OvrAccount, U256, U256)> {
        let gas_price = self.check_gas_price(sb, b).c(d!())?;

        // {1.} if success, then the transaction signature is valid.
        let addr = self.recover_signer().c(d!())?;

        // {2.}
        if let Err((tx_nonce, system_nonce)) = self.check_nonce(&addr, sb, b) {
            return Err(eg!(
                "Invalid nonce: {}, should be: {}",
                tx_nonce,
                system_nonce
            ));
        }

        // {3.}{4.}
        match self.check_balance(&addr, gas_price.clone(), sb, b) {
            Ok((a, gas_limit)) => Ok((addr, a, gas_limit, gas_price)),
            Err((needed_balance, total_balance)) => Err(eg!(
                "Insufficient balance, needed: {}, total: {}",
                needed_balance,
                total_balance
            )),
        }
    }

    // Support:
    // - Legacy transactions
    // - EIP2930 transactons
    // - EIP1559 transactions
    //
    // Both LegacyTransaction and TransactionV2 data formats are supported.
    fn exec(
        self,
        addr: H160,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> StdResult<U256, U256> {
        let metadata = StackSubstateMetadata::new(u64::MAX, &EVM_CFG);
        let mut backend = sb.state.evm.get_backend_hdr(b);
        let state = OvrStackState::new(metadata, &backend);

        // TODO
        let precompiles = BTreeMap::new();

        let mut executor =
            StackExecutor::new_with_precompiles(state, &EVM_CFG, &precompiles);

        let (exit_reason, data) = match self.tx {
            TransactionAny::Legacy(tx) => match tx.action {
                TransactionAction::Call(target) => executor.transact_call(
                    addr,
                    target,
                    tx.value,
                    tx.input,
                    tx.gas_limit.try_into().unwrap_or(u64::MAX),
                    vec![],
                ),
                TransactionAction::Create => {
                    todo!()
                }
            },
            TransactionAny::EIP2930(tx) => match tx.action {
                TransactionAction::Call(target) => executor.transact_call(
                    addr,
                    target,
                    tx.value,
                    tx.input,
                    tx.gas_limit.try_into().unwrap_or(u64::MAX),
                    tx.access_list
                        .into_iter()
                        .map(|al| (al.address, al.slots))
                        .collect(),
                ),
                TransactionAction::Create => {
                    todo!()
                }
            },
            TransactionAny::EIP1559(tx) => match tx.action {
                TransactionAction::Call(target) => executor.transact_call(
                    addr,
                    target,
                    tx.value,
                    tx.input,
                    tx.gas_limit.try_into().unwrap_or(u64::MAX),
                    tx.access_list
                        .into_iter()
                        .map(|al| (al.address, al.slots))
                        .collect(),
                ),
                TransactionAction::Create => {
                    todo!()
                }
            },
        };

        let gas_used = U256::from(executor.used_gas());

        match exit_reason {
            ExitReason::Succeed(info) => {
                let (changes, logs) = executor.into_state().deconstruct();
                backend.apply(changes, logs, false);
                Ok(gas_used)
            }
            _ => Err(gas_used),
        }
    }

    fn check_gas_price(&self, sb: &StateBranch, b: BranchName) -> Result<U256> {
        let gas_price_min = sb.state.evm.gas_price.get_value_by_branch(b);
        let gas_price_min = gas_price_min.as_ref().unwrap_or(&GAS_PRICE_MIN);

        let gas_price = match &self.tx {
            TransactionAny::Legacy(tx) => &tx.gas_price,
            TransactionAny::EIP2930(tx) => &tx.gas_price,
            TransactionAny::EIP1559(tx) => &GAS_PRICE_MIN,
        };

        if gas_price_min <= gas_price {
            Ok(gas_price.clone())
        } else {
            Err(eg!("Gas price is too low"))
        }
    }

    fn check_balance(
        &self,
        addr: &H160,
        gas_price: U256,
        sb: &StateBranch,
        b: BranchName,
    ) -> StdResult<(OvrAccount, U256), (U256, U256)> {
        let (transfer_value, gas_limit) = match &self.tx {
            TransactionAny::Legacy(tx) => (tx.value, tx.gas_limit),
            TransactionAny::EIP2930(tx) => (tx.value, tx.gas_limit),
            TransactionAny::EIP1559(tx) => (tx.value, tx.gas_limit),
        };

        let needed_balance = transfer_value
            .checked_add(gas_price.saturating_mul(gas_limit))
            .ok_or(Default::default())?;

        let mut a = sb
            .state
            .evm
            .accounts
            .get_by_branch(addr, b)
            .unwrap_or_default();

        if needed_balance <= a.balance {
            a.balance -= needed_balance;
            Ok((a, gas_limit))
        } else {
            Err((needed_balance, a.balance))
        }
    }

    fn check_nonce(
        &self,
        addr: &H160,
        sb: &StateBranch,
        b: BranchName,
    ) -> StdResult<(), (U256, U256)> {
        let tx_nonce = match &self.tx {
            TransactionAny::Legacy(tx) => tx.nonce,
            TransactionAny::EIP2930(tx) => tx.nonce,
            TransactionAny::EIP1559(tx) => tx.nonce,
        };

        let system_nonce = sb
            .state
            .evm
            .accounts
            .get_by_branch(addr, b)
            .map(|a| a.nonce)
            .unwrap_or_else(|| U256::zero());

        if tx_nonce == system_nonce {
            Ok(())
        } else {
            Err((tx_nonce, system_nonce))
        }
    }

    // if success, the transaction signature is valid.
    fn recover_signer(&self) -> Option<H160> {
        let transaction = &self.tx;
        let mut sig = [0u8; 65];
        let mut msg = [0u8; 32];
        match transaction {
            TransactionAny::Legacy(t) => {
                sig[0..32].copy_from_slice(&t.signature.r()[..]);
                sig[32..64].copy_from_slice(&t.signature.s()[..]);
                sig[64] = t.signature.standard_v();
                msg.copy_from_slice(
                    &ethereum::LegacyTransactionMessage::from(t.clone()).hash()[..],
                );
            }
            TransactionAny::EIP2930(t) => {
                sig[0..32].copy_from_slice(&t.r[..]);
                sig[32..64].copy_from_slice(&t.s[..]);
                sig[64] = t.odd_y_parity as u8;
                msg.copy_from_slice(
                    &ethereum::EIP2930TransactionMessage::from(t.clone()).hash()[..],
                );
            }
            TransactionAny::EIP1559(t) => {
                sig[0..32].copy_from_slice(&t.r[..]);
                sig[32..64].copy_from_slice(&t.s[..]);
                sig[64] = t.odd_y_parity as u8;
                msg.copy_from_slice(
                    &ethereum::EIP1559TransactionMessage::from(t.clone()).hash()[..],
                );
            }
        }
        let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg).ok()?;
        Some(H160::from(H256::from_slice(
            Keccak256::digest(&pubkey).as_slice(),
        )))
    }
}

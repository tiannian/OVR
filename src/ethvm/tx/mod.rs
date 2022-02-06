pub(crate) mod meta_tokens;

use super::{impls::stack::OvrStackState, precompile::PRECOMPILE_SET, OvrAccount};
use crate::ledger::StateBranch;
use ethereum::{TransactionAction, TransactionAny};
use evm::{
    backend::{Apply, ApplyBackend},
    executor::stack::{
        PrecompileFailure, PrecompileOutput, StackExecutor, StackSubstateMetadata,
    },
    Config as EvmCfg, ExitReason,
};
use meta_tokens::Erc20Like;
use once_cell::sync::Lazy;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::{collections::BTreeMap, fmt, result::Result as StdResult};
use vsdb::BranchName;

static EVM_CFG: Lazy<EvmCfg> = Lazy::new(EvmCfg::istanbul);
pub(crate) static GAS_PRICE_MIN: Lazy<U256> = Lazy::new(|| U256::from(10u8));

type GasPrice = U256;
type NeededAmount = U256;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tx {
    tx: TransactionAny,
}

impl Tx {
    #[inline(always)]
    pub(crate) fn apply(
        self,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> StdResult<ExecRet, Option<ExecRet>> {
        if let Ok((addr, _, gas_price)) = info!(self.pre_exec(sb, b)) {
            let ret = self.exec(addr, gas_price, sb, b);
            alt!(ret.success, Ok(ret), Err(Some(ret)))
        } else {
            Err(None)
        }
    }

    // 0. ensure the given gas price is big enough
    // 1. verify the transaction signature
    // 2. ensure the transaction nonce is bigger than the last nonce
    // 3. ensure the balance of OFUEL is bigger than `spent_amount + gas_limit`
    // 4. deducte `gas_limit` from the balance of OFUEL
    fn pre_exec(
        &self,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> Result<(H160, OvrAccount, GasPrice)> {
        // {0.}
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
        match self.check_balance(&addr, gas_price, sb, b) {
            Ok((account, _)) => Ok((addr, account, gas_price)),
            Err(Some((account, needed_amount))) => Err(eg!(
                "Insufficient balance, needed: {}, total: {}",
                needed_amount,
                account.balance
            )),
            Err(_) => Err(eg!()),
        }
    }

    // Support:
    // - Legacy transactions
    // - EIP2930 transactons
    // - EIP1559 transactions
    fn exec(
        self,
        addr: H160,
        gas_price: GasPrice,
        sb: &mut StateBranch,
        b: BranchName,
    ) -> ExecRet {
        let metadata = StackSubstateMetadata::new(u64::MAX, &EVM_CFG);
        let mut backend = sb.state.evm.get_backend_hdr(b);
        let state = OvrStackState::new(metadata, &backend);

        let precompiles = PRECOMPILE_SET.clone();
        let mut executor =
            StackExecutor::new_with_precompiles(state, &EVM_CFG, &precompiles);

        enum Ret {
            Precompile(StdResult<PrecompileOutput, PrecompileFailure>),
            Normal((ExitReason, Vec<u8>)),
        }

        let mut contract_addr = None;
        let ret = match self.tx {
            TransactionAny::Legacy(tx) => {
                let gas_limit = tx.gas_limit.try_into().unwrap_or(u64::MAX);
                match tx.action {
                    TransactionAction::Call(target) => {
                        contract_addr.replace(target);
                        if Erc20Like::addr_is_meta_token(target) {
                            Ret::Precompile(Erc20Like::execute(
                                sb,
                                target,
                                addr,
                                &tx.input,
                                Some(gas_limit),
                            ))
                        } else {
                            Ret::Normal(executor.transact_call(
                                addr,
                                target,
                                tx.value,
                                tx.input,
                                gas_limit,
                                vec![],
                            ))
                        }
                    }
                    TransactionAction::Create => Ret::Normal((
                        executor.transact_create(
                            addr,
                            tx.value,
                            tx.input,
                            gas_limit,
                            vec![],
                        ),
                        vec![],
                    )),
                }
            }
            TransactionAny::EIP2930(tx) => {
                let gas_limit = tx.gas_limit.try_into().unwrap_or(u64::MAX);
                let al = tx
                    .access_list
                    .into_iter()
                    .map(|al| (al.address, al.slots))
                    .collect();
                match tx.action {
                    TransactionAction::Call(target) => {
                        contract_addr.replace(target);
                        if Erc20Like::addr_is_meta_token(target) {
                            Ret::Precompile(Erc20Like::execute(
                                sb,
                                target,
                                addr,
                                &tx.input,
                                Some(gas_limit),
                            ))
                        } else {
                            Ret::Normal(executor.transact_call(
                                addr, target, tx.value, tx.input, gas_limit, al,
                            ))
                        }
                    }
                    TransactionAction::Create => Ret::Normal((
                        executor
                            .transact_create(addr, tx.value, tx.input, gas_limit, al),
                        vec![],
                    )),
                }
            }
            TransactionAny::EIP1559(tx) => {
                let gas_limit = tx.gas_limit.try_into().unwrap_or(u64::MAX);
                let al = tx
                    .access_list
                    .into_iter()
                    .map(|al| (al.address, al.slots))
                    .collect();
                match tx.action {
                    TransactionAction::Call(target) => {
                        contract_addr.replace(target);
                        if Erc20Like::addr_is_meta_token(target) {
                            Ret::Precompile(Erc20Like::execute(
                                sb,
                                target,
                                addr,
                                &tx.input,
                                Some(gas_limit),
                            ))
                        } else {
                            Ret::Normal(executor.transact_call(
                                addr, target, tx.value, tx.input, gas_limit, al,
                            ))
                        }
                    }
                    TransactionAction::Create => Ret::Normal((
                        executor
                            .transact_create(addr, tx.value, tx.input, gas_limit, al),
                        vec![],
                    )),
                }
            }
        };

        match ret {
            Ret::Precompile(ret) => {
                let (success, exit_reason, gas_used, extra_data, logs) = match ret {
                    Ok(info) => (
                        true,
                        ExitReason::Succeed(info.exit_status),
                        info.cost.into(),
                        info.output,
                        info.logs,
                    ),
                    Err(info) => {
                        let exit_reason =
                            if let PrecompileFailure::Error { exit_status } = info {
                                exit_status.into()
                            } else {
                                unreachable!()
                            };
                        (false, exit_reason, gas_price, vec![], vec![])
                    }
                };
                sb.state.evm.get_backend_hdr(b).apply(
                    Vec::<Apply<BTreeMap<H256, H256>>>::new(),
                    logs,
                    false,
                );
                ExecRet::new(
                    success,
                    exit_reason,
                    gas_used,
                    extra_data,
                    addr,
                    contract_addr,
                )
            }
            Ret::Normal((exit_reason, extra_data)) => {
                let gas_used = U256::from(executor.used_gas());
                let success = matches!(exit_reason, ExitReason::Succeed(_));
                let (changes, logs) = executor.into_state().deconstruct();
                if success {
                    backend.apply(changes, logs, false);
                } else {
                    backend.apply(
                        Vec::<Apply<BTreeMap<H256, H256>>>::new(),
                        logs,
                        false,
                    );
                }
                ExecRet::new(
                    success,
                    exit_reason,
                    gas_used,
                    extra_data,
                    addr,
                    contract_addr,
                )
            }
        }
    }

    #[inline(always)]
    fn check_gas_price(&self, sb: &StateBranch, b: BranchName) -> Result<U256> {
        let gas_price_min = sb.state.evm.gas_price.get_value_by_branch(b);
        let gas_price_min = gas_price_min.as_ref().unwrap_or(&GAS_PRICE_MIN);

        let gas_price = match &self.tx {
            TransactionAny::Legacy(tx) => &tx.gas_price,
            TransactionAny::EIP2930(tx) => &tx.gas_price,
            TransactionAny::EIP1559(_tx) => &GAS_PRICE_MIN,
        };

        if gas_price_min <= gas_price {
            Ok(*gas_price)
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
    ) -> StdResult<(OvrAccount, NeededAmount), Option<(OvrAccount, NeededAmount)>> {
        let (transfer_value, gas_limit) = match &self.tx {
            TransactionAny::Legacy(tx) => (tx.value, tx.gas_limit),
            TransactionAny::EIP2930(tx) => (tx.value, tx.gas_limit),
            TransactionAny::EIP1559(tx) => (tx.value, tx.gas_limit),
        };

        if gas_limit.is_zero() {
            return Err(None);
        }

        let needed_amount = gas_price
            .checked_mul(gas_limit)
            .and_then(|fee_limit| transfer_value.checked_add(fee_limit))
            .ok_or(None)?;

        let account = sb
            .state
            .evm
            .OFUEL
            .accounts
            .get_by_branch(addr, b)
            .unwrap_or_default();

        if needed_amount <= account.balance {
            Ok((account, needed_amount))
        } else {
            Err(Some((account, needed_amount)))
        }
    }

    #[inline(always)]
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
            .OFUEL
            .accounts
            .get_by_branch(addr, b)
            .map(|a| a.nonce)
            .unwrap_or_else(U256::zero);

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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ExecRet {
    pub(crate) success: bool,
    pub(crate) fee_used: U256,
    pub(crate) exit_reason: ExitReason,
    pub(crate) extra_data: Vec<u8>,
    pub(crate) caller: H160,
    pub(crate) contract_addr: Option<H160>,
}

impl ExecRet {
    #[inline(always)]
    fn new(
        success: bool,
        exit_reason: ExitReason,
        fee_used: U256,
        extra_data: Vec<u8>,
        caller: H160,
        contract_addr: Option<H160>,
    ) -> Self {
        Self {
            success,
            exit_reason,
            fee_used,
            extra_data,
            caller,
            contract_addr,
        }
    }
}

impl fmt::Display for ExecRet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

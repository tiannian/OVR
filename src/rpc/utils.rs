use crate::common::{block_number_to_height, rollback_to_height};
use crate::rpc::error::new_jsonrpc_error;
use crate::{ethvm::State as EvmState, ledger::State as LedgerState};
use serde_json::Value;
use vsdb::{BranchName, VsMgmt};
use web3_rpc_core::types::BlockNumber;

pub fn rollback_by_height(
    bn: Option<BlockNumber>,
    ledger_state: Option<&LedgerState>,
    evm_state: Option<&EvmState>,
    prefix: &str,
) -> jsonrpc_core::Result<String> {
    let bn = if let Some(bn) = bn {
        bn
    } else {
        return Err(new_jsonrpc_error("block number is none", Value::Null));
    };

    let height = block_number_to_height(bn, ledger_state, evm_state);
    let new_branch_name = rollback_to_height(height, ledger_state, evm_state, prefix)
        .map_err(|e| {
            new_jsonrpc_error("rollback by height error", Value::String(e.to_string()))
        })?;
    Ok(new_branch_name)
}

pub fn remove_branch_by_name(
    branch_name: String,
    ledger_state: Option<&LedgerState>,
    evm_state: Option<&EvmState>,
) -> jsonrpc_core::Result<()> {
    if let Some(ledger_state) = ledger_state {
        ledger_state
            .branch_remove(BranchName::from(branch_name.as_str()))
            .map_err(|e| {
                new_jsonrpc_error(
                    "ledger state remove branch error",
                    Value::String(e.to_string()),
                )
            })?;
        return Ok(());
    }

    if let Some(evm_state) = evm_state {
        evm_state
            .branch_remove(BranchName::from(branch_name.as_str()))
            .map_err(|e| {
                new_jsonrpc_error(
                    "evm state remove branch error",
                    Value::String(e.to_string()),
                )
            })?;
        return Ok(());
    }

    Ok(())
}

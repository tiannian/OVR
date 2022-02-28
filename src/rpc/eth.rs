use crate::common::{
    block_number_to_height, rollback_to_height, tm_proposer_to_evm_format,
};
use crate::ledger::{State, MAIN_BRANCH_NAME};
use crate::rpc::error::new_jsonrpc_error;
use crate::rpc::utils::{remove_branch_by_name, rollback_by_height};
use byte_slice_cast::AsByteSlice;
use ethereum_types::{H160, H256, H64, U256, U64};
use jsonrpc_core::{BoxFuture, Result};
use serde_json::Value;
use std::result::Result::Err;
use web3_rpc_core::types::{Block, BlockTransactions, SyncInfo};
use web3_rpc_core::{
    types::{
        BlockNumber, Bytes, CallRequest, Filter, Index, Log, Receipt, RichBlock,
        SyncStatus, Transaction, TransactionRequest, Work,
    },
    EthApi,
};

use super::error;

pub(crate) struct EthApiImpl {
    pub upstream: String,
    pub state: State,
}

impl EthApi for EthApiImpl {
    fn protocol_version(&self) -> BoxFuture<Result<u64>> {
        Box::pin(async move { Ok(65) })
    }

    fn chain_id(&self) -> BoxFuture<Result<Option<U64>>> {
        let chain_id = self.state.chain_id.get_value();
        Box::pin(async move { Ok(Some(U64::from(chain_id))) })
    }

    fn balance(
        &self,
        address: H160,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        let new_branch_name =
            match rollback_by_height(bn, None, Some(&self.state.evm), "balance") {
                Ok(name) => name,
                Err(e) => {
                    return Box::pin(async { Err(e) });
                }
            };

        let balance = if let Some(balance) = self.state.evm.OFUEL.accounts.get(&address)
        {
            balance.balance
        } else {
            U256::zero()
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(balance) })
    }

    // Low priority, not implemented for now
    fn send_transaction(&self, _: TransactionRequest) -> BoxFuture<Result<H256>> {
        // Cal tendermint send tx.

        Box::pin(async { Ok(H256::default()) })
    }

    fn call(
        &self,
        req: CallRequest,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<Bytes>> {
        let r;
        let resp = self
            .state
            .evm
            .call_contract(MAIN_BRANCH_NAME, req, bn)
            .map_err(|e| {
                error::new_jsonrpc_error(
                    "call contract failed",
                    Value::String(e.to_string()),
                )
            });

        ruc::d!(format!("{:?}", resp));

        if let Err(e) = resp {
            r = Err(e)
        } else if let Ok(resp) = resp {
            let bytes = Bytes::new(resp.data);
            r = Ok(bytes)
        } else {
            r = Ok(Bytes::default())
        }

        Box::pin(async { r })
    }

    fn syncing(&self) -> BoxFuture<Result<SyncStatus>> {
        // is a syncing fullnode?
        let upstream = self.upstream.clone();

        Box::pin(async move {
            let url = format!("{}/{}", upstream, "status");
            let resp = reqwest::Client::new()
                .get(url)
                .send()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error("req error", Value::String(e.to_string()))
                })?
                .json::<Value>()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error(
                        "resp to value error",
                        Value::String(e.to_string()),
                    )
                })?;
            ruc::d!(resp);

            let mut r = Err(error::new_jsonrpc_error(
                "send tx to tendermint failed",
                resp.clone(),
            ));
            if let Some(result) = resp.get("result") {
                // The following unwrap operations are safe
                // If the return result field of tendermint remains unchanged
                let sync_info = result.get("sync_info").unwrap();
                let catching_up =
                    sync_info.get("catching_up").unwrap().as_bool().unwrap();
                if catching_up {
                    r = Ok(SyncStatus::Info(SyncInfo {
                        starting_block: Default::default(),
                        current_block: U256::from(
                            sync_info
                                .get("latest_block_height")
                                .unwrap()
                                .to_string()
                                .as_bytes(),
                        ),
                        highest_block: Default::default(),
                        warp_chunks_amount: None,
                        warp_chunks_processed: None,
                    }))
                } else {
                    r = Ok(SyncStatus::None)
                }
            }

            r
        })
    }

    fn author(&self) -> BoxFuture<Result<H160>> {
        // current proposer
        let current_proposer = self.state.evm.vicinity.block_coinbase;

        Box::pin(async move { Ok(current_proposer) })
    }

    fn is_mining(&self) -> BoxFuture<Result<bool>> {
        // is validator?

        Box::pin(async move { Ok(true) })
    }

    fn gas_price(&self) -> BoxFuture<Result<U256>> {
        let gas_price = self.state.evm.gas_price.get_value();

        Box::pin(async move { Ok(gas_price) })
    }

    fn block_number(&self) -> BoxFuture<Result<U256>> {
        // return current latest block number.

        let r = if let Some((height, _)) = self.state.blocks.last() {
            Ok(U256::from(height))
        } else {
            Err(new_jsonrpc_error("state blocks is none", Value::Null))
        };

        Box::pin(async move { r })
    }

    fn storage_at(
        &self,
        addr: H160,
        index: U256,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<H256>> {
        let new_branch_name =
            match rollback_by_height(bn, None, Some(&self.state.evm), "storage_at") {
                Ok(name) => name,
                Err(e) => {
                    return Box::pin(async { Err(e) });
                }
            };

        let key = (&addr, &H256::from_slice(index.as_byte_slice()));

        let val = if let Some(val) = self.state.evm.OFUEL.storages.get(&key) {
            val
        } else {
            let error_value = serde_json::to_string(&key).unwrap(); //safe
            return Box::pin(async {
                Err(new_jsonrpc_error(
                    "The key does not have a corresponding value",
                    Value::String(error_value),
                ))
            });
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(val) })
    }

    fn block_by_hash(
        &self,
        block_hash: H256,
        is_complete: bool,
    ) -> BoxFuture<Result<Option<RichBlock>>> {
        let mut op_rb = None;
        for (height, block) in self.state.blocks.iter() {
            let mut b = Block {
                hash: None,
                parent_hash: Default::default(),
                uncles_hash: Default::default(),
                author: Default::default(),
                miner: Default::default(),
                state_root: Default::default(),
                transactions_root: Default::default(),
                receipts_root: Default::default(),
                number: None,
                gas_used: Default::default(),
                gas_limit: Default::default(),
                extra_data: Default::default(),
                logs_bloom: None,
                timestamp: Default::default(),
                difficulty: Default::default(),
                total_difficulty: Default::default(),
                seal_fields: vec![],
                uncles: vec![],
                transactions: BlockTransactions::Hashes(vec![]),
                size: None,
            };

            if block.header_hash.as_slice() == block_hash.as_bytes() {
                // Determine if you want to return all block information
                if is_complete {
                    let new_branch_name = match rollback_by_height(
                        Some(BlockNumber::Num(height)),
                        None,
                        Some(&self.state.evm),
                        "block_by_hash",
                    ) {
                        Ok(name) => name,
                        Err(e) => {
                            return Box::pin(async { Err(e) });
                        }
                    };

                    let proposer = tm_proposer_to_evm_format(&block.header.proposer);

                    // TODO: To be filled
                    b = Block {
                        hash: Some(H256::from_slice(block.header_hash.as_slice())),
                        parent_hash: H256::from_slice(block.header.prev_hash.as_slice()),
                        uncles_hash: Default::default(), //Not required
                        author: proposer,
                        miner: proposer,
                        state_root: Default::default(), //missing data?
                        transactions_root: H256::from_slice(
                            block.header.tx_merkle.root_hash.as_slice(),
                        ),
                        receipts_root: Default::default(), //missing data
                        number: Some(U256::from(height)),
                        gas_used: Default::default(), //missing data
                        gas_limit: self.state.evm.block_gas_limit.get_value(),
                        extra_data: Default::default(), //Not required
                        logs_bloom: None,               //Not required
                        timestamp: U256::from(block.header.timestamp),
                        difficulty: Default::default(), //Not required
                        total_difficulty: Default::default(), //Not required
                        seal_fields: vec![],            //Not required
                        uncles: vec![],                 //Not required
                        transactions: BlockTransactions::Full(vec![]), //missing data
                        size: None,                     //missing data
                    };

                    if let Err(e) = remove_branch_by_name(
                        new_branch_name,
                        None,
                        Some(&self.state.evm),
                    ) {
                        return Box::pin(async { Err(e) });
                    }
                }

                op_rb.replace(RichBlock {
                    inner: b,
                    extra_info: Default::default(),
                });
            }
        }

        Box::pin(async { Ok(op_rb) })
    }

    fn block_by_number(
        &self,
        bn: BlockNumber,
        is_complete: bool,
    ) -> BoxFuture<Result<Option<RichBlock>>> {
        let height = block_number_to_height(bn, None, Some(&self.state.evm));

        let r =
            rollback_to_height(height, None, Some(&self.state.evm), "block_by_number");

        if let Err(e) = r.as_ref().map_err(|e| {
            new_jsonrpc_error(
                "evm state rollback to height error",
                Value::String(e.to_string()),
            )
        }) {
            return Box::pin(async { Err(e) });
        }

        let new_branch_name = r.unwrap(); //safe

        let b = if let Some(block) = self.state.blocks.get(&height) {
            let proposer = tm_proposer_to_evm_format(&block.header.proposer);

            let b = if is_complete {
                // TODO: To be filled
                Block {
                    hash: Some(H256::from_slice(block.header_hash.as_slice())),
                    parent_hash: H256::from_slice(block.header.prev_hash.as_slice()),
                    uncles_hash: Default::default(), //Not required
                    author: proposer,
                    miner: proposer,
                    state_root: Default::default(), //missing data?
                    transactions_root: H256::from_slice(
                        block.header.tx_merkle.root_hash.as_slice(),
                    ),
                    receipts_root: Default::default(), //missing data
                    number: Some(U256::from(height)),
                    gas_used: Default::default(), //missing data
                    gas_limit: self.state.evm.block_gas_limit.get_value(),
                    extra_data: Default::default(), //Not required
                    logs_bloom: None,               //Not required
                    timestamp: U256::from(block.header.timestamp),
                    difficulty: Default::default(), //Not required
                    total_difficulty: Default::default(), //Not required
                    seal_fields: vec![],            //Not required
                    uncles: vec![],                 //Not required
                    transactions: BlockTransactions::Full(vec![]), //missing data
                    size: None,                     //missing data
                }
            } else {
                Block {
                    hash: None,
                    parent_hash: Default::default(),
                    uncles_hash: Default::default(),
                    author: Default::default(),
                    miner: Default::default(),
                    state_root: Default::default(),
                    transactions_root: Default::default(),
                    receipts_root: Default::default(),
                    number: None,
                    gas_used: Default::default(),
                    gas_limit: Default::default(),
                    extra_data: Default::default(),
                    logs_bloom: None,
                    timestamp: Default::default(),
                    difficulty: Default::default(),
                    total_difficulty: Default::default(),
                    seal_fields: vec![],
                    uncles: vec![],
                    transactions: BlockTransactions::Hashes(vec![]),
                    size: None,
                }
            };

            b
        } else {
            return Box::pin(async move {
                Err(new_jsonrpc_error(
                    "The block height does not have a corresponding value",
                    Value::String(height.to_string()),
                ))
            });
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async {
            Ok(Some(RichBlock {
                inner: b,
                extra_info: Default::default(),
            }))
        })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn transaction_count(
        &self,
        _: H160,
        _: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        Box::pin(async { Ok(Default::default()) })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn block_transaction_count_by_hash(
        &self,
        _: H256,
    ) -> BoxFuture<Result<Option<U256>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn block_transaction_count_by_number(
        &self,
        _: BlockNumber,
    ) -> BoxFuture<Result<Option<U256>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn code_at(&self, addr: H160, bn: Option<BlockNumber>) -> BoxFuture<Result<Bytes>> {
        let new_branch_name =
            match rollback_by_height(bn, None, Some(&self.state.evm), "code_at") {
                Ok(name) => name,
                Err(e) => {
                    return Box::pin(async { Err(e) });
                }
            };

        let bytes = if let Some(account) = self.state.evm.OFUEL.accounts.get(&addr) {
            account.code
        } else {
            return Box::pin(async move {
                Err(new_jsonrpc_error(
                    "No corresponding account was found at this address",
                    Value::String(addr.to_string()),
                ))
            });
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async { Ok(Bytes::new(bytes)) })
    }

    fn send_raw_transaction(&self, tx: Bytes) -> BoxFuture<Result<H256>> {
        let upstream = self.upstream.clone();
        Box::pin(async move {
            let tx_param = format!("0x{}", hex::encode(tx.0));
            let url = format!("{}/{}", upstream, "broadcast_tx_sync");
            let query: Vec<(String, String)> = vec![("tx".to_string(), tx_param)];

            let resp = reqwest::Client::new()
                .get(url)
                .query(&query)
                .send()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error("req error", Value::String(e.to_string()))
                })?
                .json::<Value>()
                .await
                .map_err(|e| {
                    error::new_jsonrpc_error(
                        "resp to value error",
                        Value::String(e.to_string()),
                    )
                })?;

            ruc::d!(resp);
            let mut r = Err(error::new_jsonrpc_error(
                "send tx to tendermint failed",
                resp.clone(),
            ));
            if let Some(result) = resp.get("result") {
                if let Some(code) = result.get("code") {
                    if code.eq(&0) {
                        r = Ok(H256::default())
                    }
                }
            }

            r
        })
    }

    fn estimate_gas(
        &self,
        req: CallRequest,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        let r;
        let resp = self
            .state
            .evm
            .call_contract(MAIN_BRANCH_NAME, req, bn)
            .map_err(|e| {
                error::new_jsonrpc_error(
                    "call contract failed",
                    Value::String(e.to_string()),
                )
            });

        ruc::d!(format!("{:?}", resp));

        if let Err(e) = resp {
            r = Err(e)
        } else if let Ok(resp) = resp {
            let gas_used = U256::from(resp.gas_used);
            r = Ok(gas_used)
        } else {
            r = Err(new_jsonrpc_error("call contract resp none", Value::Null));
        }

        Box::pin(async { r })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn transaction_by_hash(&self, _: H256) -> BoxFuture<Result<Option<Transaction>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn transaction_by_block_hash_and_index(
        &self,
        _: H256,
        _: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn transaction_by_block_number_and_index(
        &self,
        _: BlockNumber,
        _: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    // TODO: impl tx Storage first and than impl this interface
    fn transaction_receipt(&self, _: H256) -> BoxFuture<Result<Option<Receipt>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn logs(&self, _: Filter) -> BoxFuture<Result<Vec<Log>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    // ----------- Not impl.
    fn work(&self) -> Result<Work> {
        Err(error::no_impl())
    }

    fn submit_work(&self, _: H64, _: H256, _: H256) -> Result<bool> {
        Err(error::no_impl())
    }

    fn submit_hashrate(&self, _: U256, _: H256) -> Result<bool> {
        Err(error::no_impl())
    }

    fn hashrate(&self) -> Result<U256> {
        Err(error::no_impl())
    }
    fn uncle_by_block_hash_and_index(
        &self,
        _: H256,
        _: Index,
    ) -> Result<Option<RichBlock>> {
        Err(error::no_impl())
    }

    fn uncle_by_block_number_and_index(
        &self,
        _: BlockNumber,
        _: Index,
    ) -> Result<Option<RichBlock>> {
        Err(error::no_impl())
    }

    fn block_uncles_count_by_hash(&self, _: H256) -> Result<U256> {
        Err(error::no_impl())
    }

    fn block_uncles_count_by_number(&self, _: BlockNumber) -> Result<U256> {
        Err(error::no_impl())
    }

    fn accounts(&self) -> Result<Vec<H160>> {
        // This api is no impl, only return a empty array.
        Ok(vec![])
    }
}

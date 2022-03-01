use crate::{
    common::{
        block_number_to_height, rollback_to_height, tm_proposer_to_evm_format, HashValue,
    },
    ledger::{State, MAIN_BRANCH_NAME},
    rpc::{
        error::new_jsonrpc_error,
        utils::{
            filter_block_logs, remove_branch_by_name, rollback_by_height, tx_to_web3_tx,
        },
    },
    tx::Tx,
};
use byte_slice_cast::AsByteSlice;
use ethereum_types::{Bloom, H160, H256, H64, U256, U64};
use jsonrpc_core::{BoxFuture, Result};
use serde_json::Value;
use std::result::Result::Err;
use web3_rpc_core::{
    types::{
        Block, BlockNumber, BlockTransactions, Bytes, CallRequest, Filter,
        FilteredParams, Index, Log, Receipt, RichBlock, SyncInfo, SyncStatus,
        Transaction, TransactionRequest, Work,
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

                    let receipt = if let Some((_, receipt)) =
                        block.header.receipts.last()
                    {
                        receipt
                    } else {
                        return Box::pin(async {
                            Err(new_jsonrpc_error("this block no receipt!", Value::Null))
                        });
                    };

                    // TODO: To be filled
                    b = Block {
                        hash: Some(H256::from_slice(block.header_hash.as_slice())),
                        parent_hash: H256::from_slice(block.header.prev_hash.as_slice()),
                        uncles_hash: Default::default(), //Not required
                        author: proposer,
                        miner: proposer,
                        state_root: Default::default(), //Not required
                        transactions_root: H256::from_slice(
                            block.header.tx_merkle.root_hash.as_slice(),
                        ),
                        receipts_root: Default::default(), //Not required
                        number: Some(U256::from(height)),
                        gas_used: receipt.block_gas_used, //missing data
                        gas_limit: self.state.evm.block_gas_limit.get_value(),
                        extra_data: Default::default(), //Not required
                        logs_bloom: Some(Bloom::from_slice(block.bloom.as_slice())), //Not required
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
        let height = block_number_to_height(Some(bn), None, Some(&self.state.evm));

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

    fn transaction_count(
        &self,
        addr: H160,
        bn: Option<BlockNumber>,
    ) -> BoxFuture<Result<U256>> {
        let height = block_number_to_height(bn, Some(&self.state), None);
        let new_branch_name = match rollback_to_height(
            height,
            Some(&self.state),
            None,
            "transaction_count",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let mut tx_count = 0;

        if let Some(block) = self.state.blocks.get(&height) {
            for tx in block.txs.iter() {
                match tx {
                    Tx::Evm(evm_tx) => {
                        // Judgement from or to
                        let (from, to) = evm_tx.get_from_to();
                        if let Some(from) = from {
                            if from.eq(&addr) {
                                tx_count += 1;
                                continue;
                            }
                        }

                        if let Some(to) = to {
                            if to.eq(&addr) {
                                tx_count += 1;
                                continue;
                            }
                        }
                    }
                    Tx::Native(_) => {
                        continue;
                    }
                };
            }
        } else {
            return Box::pin(async move {
                Err(new_jsonrpc_error(
                    "there is no corresponding block under this height",
                    Value::String(height.to_string()),
                ))
            });
        }

        if let Err(e) = remove_branch_by_name(new_branch_name, Some(&self.state), None) {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(U256::from(tx_count)) })
    }

    fn block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> BoxFuture<Result<Option<U256>>> {
        let mut tx_count = 0;

        for (_, block) in self.state.blocks.iter() {
            if block.header_hash == block_hash.as_bytes() {
                tx_count = block.txs.len();
            }
        }

        Box::pin(async move { Ok(Some(U256::from(tx_count))) })
    }

    fn block_transaction_count_by_number(
        &self,
        bn: BlockNumber,
    ) -> BoxFuture<Result<Option<U256>>> {
        let height = block_number_to_height(Some(bn), Some(&self.state), None);

        let new_branch_name = match rollback_to_height(
            height,
            Some(&self.state),
            None,
            "block_transaction_count_by_number",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let tx_count = if let Some(block) = self.state.blocks.get(&height) {
            block.txs.len()
        } else {
            return Box::pin(async move {
                Err(new_jsonrpc_error(
                    "there is no corresponding block under this height",
                    Value::String(height.to_string()),
                ))
            });
        };

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async move { Ok(Some(U256::from(tx_count))) })
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

    fn transaction_by_hash(
        &self,
        tx_hash: H256,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        let mut transaction = None;

        'outer: for (height, block) in self.state.blocks.iter() {
            for (index, tx) in block.txs.iter().enumerate() {
                if tx.hash() == tx_hash.as_bytes() {
                    match tx_to_web3_tx(
                        &tx,
                        &block,
                        height,
                        index,
                        self.state.chain_id.get_value(),
                    ) {
                        Ok(op) => {
                            transaction = op;
                            break 'outer;
                        }
                        Err(e) => {
                            return Box::pin(async { Err(e) });
                        }
                    }
                }
            }
        }

        Box::pin(async { Ok(transaction) })
    }

    fn transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        let mut transaction = None;

        for (height, block) in self.state.blocks.iter() {
            if block.header_hash == block_hash.as_bytes() {
                if let Some(tx) = block.txs.get(index.value()) {
                    match tx_to_web3_tx(
                        &tx,
                        &block,
                        height,
                        index.value(),
                        self.state.chain_id.get_value(),
                    ) {
                        Ok(op) => {
                            transaction = op;
                            break;
                        }
                        Err(e) => {
                            return Box::pin(async { Err(e) });
                        }
                    }
                }
            }
        }

        Box::pin(async { Ok(transaction) })
    }

    fn transaction_by_block_number_and_index(
        &self,
        bn: BlockNumber,
        index: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        let height = block_number_to_height(Some(bn), Some(&self.state), None);
        let new_branch_name = match rollback_to_height(
            height,
            Some(&self.state),
            None,
            "transaction_by_block_number_and_index",
        ) {
            Ok(name) => name,
            Err(e) => {
                return Box::pin(async move {
                    Err(new_jsonrpc_error(
                        "rollback to height",
                        Value::String(e.to_string()),
                    ))
                });
            }
        };

        let mut transaction = None;

        if let Some(block) = self.state.blocks.get(&height) {
            if let Some(tx) = block.txs.get(index.value()) {
                match tx_to_web3_tx(
                    &tx,
                    &block,
                    height,
                    index.value(),
                    self.state.chain_id.get_value(),
                ) {
                    Ok(op) => {
                        transaction = op;
                    }
                    Err(e) => {
                        return Box::pin(async { Err(e) });
                    }
                }
            }
        } else {
            return Box::pin(async move {
                Err(new_jsonrpc_error(
                    "there is no corresponding block under this height",
                    Value::String(height.to_string()),
                ))
            });
        }

        if let Err(e) =
            remove_branch_by_name(new_branch_name, None, Some(&self.state.evm))
        {
            return Box::pin(async { Err(e) });
        }

        Box::pin(async { Ok(transaction) })
    }

    fn transaction_receipt(&self, tx_hash: H256) -> BoxFuture<Result<Option<Receipt>>> {
        let mut op = None;

        for (height, block) in self.state.blocks.iter() {
            let hash = HashValue::from(tx_hash.as_bytes());
            let block_hash = H256::from_slice(block.header_hash.as_slice());

            if let Some(r) = block.header.receipts.get(&hash) {
                let mut logs = vec![];

                for l in r.logs {
                    logs.push(Log {
                        address: l.address,
                        topics: l.topics,
                        data: Bytes::new(l.data),
                        block_hash: Some(block_hash),
                        block_number: Some(U256::from(height)),
                        transaction_hash: Some(tx_hash),
                        transaction_index: Some(U256::from(l.tx_index)),
                        log_index: Some(U256::from(l.log_index_in_block)),
                        transaction_log_index: Some(U256::from(l.log_index_in_tx)),
                        removed: false,
                    });
                }

                op.replace(Receipt {
                    transaction_hash: Some(tx_hash),
                    transaction_index: Some(U256::from(r.tx_index)),
                    block_hash: Some(block_hash),
                    from: r.from,
                    to: r.to,
                    block_number: Some(U256::from(height)),
                    cumulative_gas_used: r.block_gas_used,
                    gas_used: Some(r.tx_gas_used),
                    contract_address: r.contract_addr,
                    logs,
                    state_root: None,
                    logs_bloom: Default::default(),
                    status_code: None,
                });
            }
        }

        Box::pin(async { Ok(op) })
    }

    fn logs(&self, filter: Filter) -> BoxFuture<Result<Vec<Log>>> {
        let mut logs = vec![];

        if let Some(hash) = filter.block_hash {
            for (height, block) in self.state.blocks.iter() {
                if block.header_hash == hash.as_bytes() {
                    logs.append(&mut filter_block_logs(&block, &filter, height));
                    break;
                }
            }
        } else {
            let (current_height, _) = self.state.blocks.last().unwrap_or_default();

            let mut to =
                block_number_to_height(filter.to_block.clone(), Some(&self.state), None);
            if to > current_height {
                to = current_height;
            }

            let mut from = block_number_to_height(
                filter.from_block.clone(),
                Some(&self.state),
                None,
            );
            if from > current_height {
                from = current_height;
            }

            let topics_input = if filter.topics.is_some() {
                let filtered_params = FilteredParams::new(Some(filter.clone()));
                Some(filtered_params.flat_topics)
            } else {
                None
            };

            let address_bloom_filter =
                FilteredParams::addresses_bloom_filter(&filter.address);
            let topic_bloom_filters = FilteredParams::topics_bloom_filter(&topics_input);

            for height in from..=to {
                if let Some(block) = self.state.blocks.get(&height) {
                    let b = Bloom::from_slice(block.bloom.as_slice());
                    if FilteredParams::address_in_bloom(b, &address_bloom_filter)
                        && FilteredParams::topics_in_bloom(b, &topic_bloom_filters)
                    {
                        logs.append(&mut filter_block_logs(&block, &filter, height));
                    }
                }
            }
        };

        Box::pin(async { Ok(logs) })
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

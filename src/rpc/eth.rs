use std::sync::Arc;

use ethereum_types::{H160, H256, H64, U256, U64};
use jsonrpc_core::{BoxFuture, Result};
use parking_lot::RwLock;
use web3_rpc_core::{
    types::{
        BlockNumber, Bytes, CallRequest, Filter, Index, Log, Receipt, RichBlock, SyncStatus,
        Transaction, TransactionRequest, Work,
    },
    EthApi,
};

use crate::ledger::StateBranch;

use super::error;

pub(crate) struct EthApiImpl {
    pub upstream: String,
    pub main: Arc<RwLock<StateBranch>>,
}

impl EthApi for EthApiImpl {
    fn protocol_version(&self) -> BoxFuture<Result<u64>> {
        Box::pin(async move { Ok(0) })
    }

    fn chain_id(&self) -> BoxFuture<Result<Option<U64>>> {
        let state = self.main.read();

        let chain_id = state.state.chain_id.get_value();

        Box::pin(async move { Ok(Some(U64::from(chain_id))) })
    }


    fn balance(&self, address: H160, _height: Option<BlockNumber>) -> BoxFuture<Result<U256>> {
        // get balance on special target.

        let state = self.main.read();

        // TODO: use get_by_version to get special height.
        let balance = if let Some(balance) = state.state.evm.OVRG.accounts.get(&address) {
            balance.balance
        } else {
            U256::zero()
        };

        Box::pin(async move { Ok(balance) })
    }

    fn send_transaction(&self, _: TransactionRequest) -> BoxFuture<Result<H256>> {
        // Cal tendermint send tx.

        Box::pin(async { Ok(H256::default()) })
    }

    fn call(&self, _: CallRequest, _: Option<BlockNumber>) -> BoxFuture<Result<Bytes>> {
        // Call evm on a new branch.

        Box::pin(async { Ok(Bytes::default()) })
    }

    fn syncing(&self) -> BoxFuture<Result<SyncStatus>> {
        // is a syncing fullnode?

        Box::pin(async move { Ok(SyncStatus::None) })
    }

    fn author(&self) -> BoxFuture<Result<H160>> {
        // current proposer

        Box::pin(async move { Ok(H160::default()) })
    }

    fn is_mining(&self) -> BoxFuture<Result<bool>> {
        // is validator?

        Box::pin(async move { Ok(true) })
    }

    fn gas_price(&self) -> BoxFuture<Result<U256>> {
        let state = self.main.read();

        let gas_price = state.state.evm.gas_price.get_value();

        Box::pin(async move { Ok(gas_price) })
    }

    fn block_number(&self) -> BoxFuture<Result<U256>> {
        // return current latest block number.

        Box::pin(async move { Ok(U256::default()) })
    }

    fn storage_at(&self, _: H160, _: U256, _: Option<BlockNumber>) -> BoxFuture<Result<H256>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn block_by_hash(&self, _: H256, _: bool) -> BoxFuture<Result<Option<RichBlock>>> {
        Box::pin(async { Ok(None) })
    }

    fn block_by_number(&self, _: BlockNumber, _: bool) -> BoxFuture<Result<Option<RichBlock>>> {
        Box::pin(async { Ok(None) })
    }

    fn transaction_count(&self, _: H160, _: Option<BlockNumber>) -> BoxFuture<Result<U256>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn block_transaction_count_by_hash(&self, _: H256) -> BoxFuture<Result<Option<U256>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn block_transaction_count_by_number(&self, _: BlockNumber) -> BoxFuture<Result<Option<U256>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn code_at(&self, _: H160, _: Option<BlockNumber>) -> BoxFuture<Result<Bytes>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn send_raw_transaction(&self, _: Bytes) -> BoxFuture<Result<H256>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn estimate_gas(&self, _: CallRequest, _: Option<BlockNumber>) -> BoxFuture<Result<U256>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn transaction_by_hash(&self, _: H256) -> BoxFuture<Result<Option<Transaction>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn transaction_by_block_hash_and_index(
        &self,
        _: H256,
        _: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        Box::pin(async { Ok(Default::default()) })
    }

    fn transaction_by_block_number_and_index(
        &self,
        _: BlockNumber,
        _: Index,
    ) -> BoxFuture<Result<Option<Transaction>>> {
        Box::pin(async { Ok(Default::default()) })
    }

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
    fn uncle_by_block_hash_and_index(&self, _: H256, _: Index) -> Result<Option<RichBlock>> {
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


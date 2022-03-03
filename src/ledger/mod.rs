//!
//! # Ledger, world state
//!

pub mod staking;

use crate::common::handle_bloom;
use crate::{
    common::{
        block_hash_to_evm_format, hash_sha3_256, tm_proposer_to_evm_format, BlockHeight,
        HashValue, HashValueRef, TmAddress, TmAddressRef,
    },
    ethvm::{self, tx::GAS_PRICE_MIN},
    tx::Tx,
};
use ethereum::Log as EthLog;
use ethereum_types::Bloom;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use primitive_types::{H160, H256, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use std::{fs, io::ErrorKind, mem, sync::Arc};
use vsdb::{
    merkle::{MerkleTree, MerkleTreeStore},
    BranchName, MapxOrd, OrphanVs, ParentBranchName, ValueEn, ValueEnDe, Vecx, Vs,
    VsMgmt, INITIAL_VERSION,
};

pub const MAIN_BRANCH_NAME: BranchName = BranchName(b"Main");
const DELIVER_TX_BRANCH_NAME: BranchName = BranchName(b"DeliverTx");
const CHECK_TX_BRANCH_NAME: BranchName = BranchName(b"CheckTx");

static LEDGER_SNAPSHOT_PATH: Lazy<String> = Lazy::new(|| {
    let dir = format!("{}/overeality/ledger", vsdb::vsdb_get_custom_dir());
    pnk!(fs::create_dir_all(&dir));
    dir + "/ledger.json"
});

#[derive(Clone, Debug)]
pub struct Ledger {
    // used for web3 APIs
    pub state: State,
    pub main: Arc<RwLock<StateBranch>>,
    pub deliver_tx: Arc<RwLock<StateBranch>>,
    pub check_tx: Arc<RwLock<StateBranch>>,
}

impl Ledger {
    pub fn new(
        chain_id: u64,
        chain_name: String,
        chain_version: String,
        gas_price: Option<u128>,
        block_gas_limit: Option<u128>,
        block_base_fee_per_gas: Option<u128>,
    ) -> Result<Self> {
        let mut state = State::default();

        state.branch_create(MAIN_BRANCH_NAME).c(d!())?;
        state.branch_set_default(MAIN_BRANCH_NAME).c(d!())?;

        // ensure we have an initial version
        state.version_create(INITIAL_VERSION).c(d!())?;

        state.chain_id.set_value(chain_id).c(d!())?;
        state.chain_name.set_value(chain_name).c(d!())?;
        state.chain_version.set_value(chain_version).c(d!())?;

        state
            .evm
            .gas_price
            .set_value(gas_price.map(|v| v.into()).unwrap_or(*GAS_PRICE_MIN))
            .c(d!())?;
        state
            .evm
            .block_gas_limit
            .set_value(block_gas_limit.unwrap_or(u128::MAX).into())
            .c(d!())?;
        state
            .evm
            .block_base_fee_per_gas
            .set_value(block_base_fee_per_gas.unwrap_or_default().into())
            .c(d!())?;

        let main = StateBranch::new(&state, MAIN_BRANCH_NAME).c(d!())?;

        state.branch_create(DELIVER_TX_BRANCH_NAME).c(d!())?;
        state.branch_create(CHECK_TX_BRANCH_NAME).c(d!())?;

        let deliver_tx = StateBranch::new(&state, DELIVER_TX_BRANCH_NAME).c(d!())?;
        let check_tx = StateBranch::new(&state, CHECK_TX_BRANCH_NAME).c(d!())?;

        Ok(Self {
            state,
            main: Arc::new(RwLock::new(main)),
            deliver_tx: Arc::new(RwLock::new(deliver_tx)),
            check_tx: Arc::new(RwLock::new(check_tx)),
        })
    }

    #[inline(always)]
    pub fn consensus_refresh(&self, proposer: TmAddress, timestamp: u64) -> Result<()> {
        self.refresh_inner(proposer, timestamp, false).c(d!())
    }

    #[inline(always)]
    fn loading_refresh(&self) -> Result<()> {
        self.refresh_inner(Default::default(), Default::default(), true)
    }

    fn refresh_inner(
        &self,
        proposer: TmAddress,
        timestamp: u64,
        is_loading: bool,
    ) -> Result<()> {
        let mut main = self.main.write();
        if !is_loading {
            main.prepare_next_block(proposer, timestamp).c(d!())?;
        }
        main.state.refresh_branches().c(d!())?;
        main.clean_cache();

        let mut deliver_tx = self.deliver_tx.write();
        let br = deliver_tx.branch.clone();
        deliver_tx.state = main.state.clone();
        deliver_tx
            .state
            .branch_set_default(br.as_slice().into())
            .c(d!())?;
        deliver_tx.clean_cache();

        let mut check_tx = self.check_tx.write();
        let br = check_tx.branch.clone();
        check_tx.state = main.state.clone();
        check_tx
            .state
            .branch_set_default(br.as_slice().into())
            .c(d!())?;
        check_tx.clean_cache();

        Ok(())
    }

    #[inline(always)]
    pub fn commit(&self) -> Result<HashValue> {
        let mut main = self.main.write();
        main.commit().c(d!()).map(|_| main.last_block_hash())
    }

    #[inline(always)]
    pub fn load_from_snapshot() -> Result<Option<Self>> {
        match StateBranch::load_from_snapshot().c(d!()) {
            Ok(Some(sb)) => {
                let ledger = Ledger {
                    state: sb.state.clone(),
                    main: Arc::new(RwLock::new(sb.clone())),
                    deliver_tx: Arc::new(RwLock::new(sb.clone())),
                    check_tx: Arc::new(RwLock::new(sb)),
                };
                ledger.loading_refresh().c(d!())?;
                Ok(Some(ledger))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e).c(d!()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateBranch {
    pub state: State,
    pub branch: Vec<u8>,
    pub tx_hashes_in_process: Vec<HashValue>,
    pub block_in_process: Block,
}

pub struct ApplyResp {
    pub receipt: Option<Receipt>,
    pub logs: Option<Vec<Log>>,
}

impl StateBranch {
    #[inline(always)]
    pub fn new(state: &State, branch: BranchName) -> Result<Self> {
        let mut s = state.clone();
        s.branch_set_default(branch).c(d!())?;

        Ok(Self {
            state: s,
            branch: branch.0.to_owned(),
            tx_hashes_in_process: vec![],
            block_in_process: Block::default(),
        })
    }

    // NOTE:
    // - Only used by the 'main' branch of `Ledger`
    // - Call this in the 'BeginBlock' field of ABCI
    fn prepare_next_block(&mut self, proposer: TmAddress, timestamp: u64) -> Result<()> {
        self.tx_hashes_in_process.clear();

        let (h, prev_hash) = self
            .last_block()
            .map(|b| (b.header.height, b.header_hash))
            .unwrap_or_default();
        self.block_in_process = Block::new(1 + h, proposer, timestamp, prev_hash);

        let b = self.branch.clone();
        let b = b.as_slice().into();

        let ver = VsVersion::new(self.block_in_process.header.height, 0);
        self.state
            .version_create_by_branch(ver.encode_value().as_ref().into(), b)
            .c(d!())?;

        self.update_evm_aux(b);

        Ok(())
    }

    // Deal with each transaction.
    // Will be used by all the 3 branches of `Ledger`.
    pub(crate) fn apply_tx(&mut self, tx: Tx) -> Result<Option<Receipt>> {
        let b = self.branch.clone();
        let b = b.as_slice().into();

        let ver = VsVersion::new(
            self.block_in_process.header.height,
            1 + self.tx_hashes_in_process.len() as u64,
        );
        self.state
            .version_create_by_branch(ver.encode_value().as_ref().into(), b)
            .c(d!())?;

        let tx_hash = tx.hash();

        let mut r = None;

        match tx {
            Tx::Evm(tx) => tx
                .apply(self, b, false)
                .map(|(ret, mut receipt)| {
                    self.charge_fee(ret.caller, ret.fee_used, b);
                    self.tx_hashes_in_process.push(tx_hash.clone());

                    let mut logs = ret.gen_logs(tx_hash);
                    receipt.add_logs(logs.as_mut_slice());

                    r = Some(receipt);
                })
                .map_err(|e| {
                    pnk!(self.state.version_pop_by_branch(b));
                    if let Some(ret) = e.as_ref() {
                        self.charge_fee(ret.caller, ret.fee_used, b);
                    }
                    eg!(e.map(|e| e.to_string()).unwrap_or_default())
                })?,
            Tx::Native(tx) => tx
                .apply(self, b)
                .map(|ret| {
                    self.charge_fee(ret.caller, ret.fee_used, b);
                    self.tx_hashes_in_process.push(tx_hash);
                })
                .map_err(|e| {
                    pnk!(self.state.version_pop_by_branch(b));
                    if let Some(ret) = e.as_ref() {
                        self.charge_fee(ret.caller, ret.fee_used, b);
                    }
                    eg!(e.map(|e| e.to_string()).unwrap_or_default())
                })?,
        };

        Ok(r)
    }

    // NOTE:
    // - Only used by the 'main' branch of the `Ledger`
    fn commit(&mut self) -> Result<()> {
        // Make it never empty,
        // thus the root hash will always exist
        self.tx_hashes_in_process.push(hash_sha3_256(&[&[]]));

        let hashes = self
            .tx_hashes_in_process
            .iter()
            .map(|h| h.as_slice())
            .collect::<Vec<_>>();
        let mt = MerkleTree::new(&hashes);
        let root = mt.get_root().unwrap().to_vec();

        self.block_in_process.header.tx_merkle.tree = mt.into();
        self.block_in_process.header.tx_merkle.root_hash = root;

        // Calculate the total amount of block gas to be used
        let mut block_gas_used: U256 = U256::zero();
        self.block_in_process
            .header
            .receipts
            .iter()
            .for_each(|(_, r)| {
                block_gas_used += r.tx_gas_used;
            });
        let mut b = Bloom::from_slice(self.block_in_process.bloom.as_slice());
        for hash in self.tx_hashes_in_process.iter() {
            if let Some(mut r) = self.block_in_process.header.receipts.get_mut(hash) {
                r.block_gas_used = block_gas_used;
                handle_bloom(&mut b, r.logs.as_slice());
            }
        }

        self.block_in_process.bloom = b.as_bytes().to_vec();
        self.block_in_process.header_hash = self.block_in_process.header.hash();

        let block = mem::take(&mut self.block_in_process);

        self.state.evm.block_hashes.insert(
            block.header.height,
            block_hash_to_evm_format(&block.header_hash),
        );

        self.state.blocks.insert(block.header.height, block);

        vsdb::vsdb_flush();
        self.write_snapshot().c(d!())
    }

    fn update_evm_aux(&mut self, b: BranchName) {
        self.state.evm.update_vicinity(
            U256::from(self.state.chain_id.get_value_by_branch(b).unwrap()),
            tm_proposer_to_evm_format(&self.block_in_process.header.proposer),
            U256::from(self.block_in_process.header.timestamp),
        );
    }

    // #[inline(always)]
    // pub fn get_evm_state(&mut self) -> &ethvm::State {
    //     &self.state.evm
    // }

    #[inline(always)]
    pub fn get_evm_state_mut(&mut self) -> &mut ethvm::State {
        &mut self.state.evm
    }

    #[inline(always)]
    fn charge_fee(&self, caller: H160, amount: U256, b: BranchName) {
        alt!(amount.is_zero(), return);
        let mut account = pnk!(self.state.evm.OFUEL.accounts.get_by_branch(&caller, b));
        account.balance = account.balance.saturating_sub(amount);
        pnk!(
            self.state
                .evm
                .OFUEL
                .accounts
                .insert_by_branch(caller, account, b)
        );
    }

    // #[inline(always)]
    // fn branch_name(&self) -> BranchName {
    //     self.branch.as_slice().into()
    // }

    #[inline(always)]
    pub fn last_block(&self) -> Option<Block> {
        self.state.blocks.last().map(|(_, b)| b)
    }

    // #[inline(always)]
    // pub fn last_block_height(&self) -> BlockHeight {
    //     self.state.blocks.last().map(|(h, _)| h).unwrap_or(0)
    // }

    #[inline(always)]
    pub fn last_block_hash(&self) -> HashValue {
        self.last_block().unwrap_or_default().header_hash
    }

    #[inline(always)]
    fn load_from_snapshot() -> Result<Option<Self>> {
        match fs::read(&*LEDGER_SNAPSHOT_PATH) {
            Ok(c) => StateBranch::decode(c.as_slice()).c(d!()).map(Some),
            Err(e) => {
                if let ErrorKind::NotFound = e.kind() {
                    Ok(None)
                } else {
                    Err(e).c(d!())
                }
            }
        }
    }

    #[inline(always)]
    fn write_snapshot(&self) -> Result<()> {
        let contents = self.encode();
        fs::write(&*LEDGER_SNAPSHOT_PATH, &contents).c(d!())
    }

    #[inline(always)]
    fn clean_cache(&mut self) {
        self.tx_hashes_in_process.clear();
    }
}

#[derive(Vs, Default, Clone, Debug, Deserialize, Serialize)]
pub struct State {
    pub chain_id: OrphanVs<u64>,
    pub chain_name: OrphanVs<String>,
    pub chain_version: OrphanVs<String>,

    pub evm: ethvm::State,
    pub staking: staking::State,

    // maintained by the 'main' branch only
    pub blocks: MapxOrd<BlockHeight, Block>,
}

impl State {
    fn refresh_branches(&self) -> Result<()> {
        self.branch_remove(CHECK_TX_BRANCH_NAME).c(d!())?;
        self.branch_merge_to_parent(DELIVER_TX_BRANCH_NAME)
            .c(d!())?;
        self.branch_create_by_base_branch(
            DELIVER_TX_BRANCH_NAME,
            ParentBranchName::from(MAIN_BRANCH_NAME.0),
        )
        .c(d!())?;
        self.branch_create_by_base_branch(
            CHECK_TX_BRANCH_NAME,
            ParentBranchName::from(MAIN_BRANCH_NAME.0),
        )
        .c(d!())
    }
}

#[derive(Vs, Clone, Debug, Default, Deserialize, Serialize)]
pub struct Block {
    pub header: BlockHeader,
    pub header_hash: HashValue,
    // transaction vec
    pub txs: Vecx<Tx>,
    // bloom
    pub bloom: Vec<u8>,
}

impl Block {
    #[inline(always)]
    fn new(
        height: BlockHeight,
        proposer: TmAddress,
        timestamp: u64,
        prev_hash: HashValue,
    ) -> Self {
        Self {
            header: BlockHeader {
                height,
                proposer,
                timestamp,
                tx_merkle: TxMerkle::default(),
                prev_hash,
                receipts: MapxOrd::default(),
            },
            header_hash: Default::default(),
            txs: Vecx::default(),
            // logs: MapxOrd::default(),
            bloom: Bloom::default().as_bytes().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Receipt {
    // transaction hash
    pub tx_hash: HashValue,
    // transaction index in block
    pub tx_index: u64,
    // transaction originator
    pub from: Option<H160>,
    // transaction recipients
    pub to: Option<H160>,
    // the total amount of gas used for all transactions in this block
    pub block_gas_used: U256,
    // gas used for transaction
    pub tx_gas_used: U256,
    // here is contract address if recipients is none
    pub contract_addr: Option<H160>,
    // TODO: to be filled
    pub state_root: Option<HashValue>,
    // execute success or failure
    pub status_code: bool,
    // logs
    pub logs: Vec<Log>,
}

impl Receipt {
    pub fn add_logs(&mut self, logs: &mut [Log]) {
        for (index, log) in logs.iter_mut().enumerate() {
            log.tx_index = self.tx_index;
            log.log_index_in_tx = index as u64;
        }

        self.logs = logs.to_vec();
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Log {
    // source address of this log
    pub address: H160,
    // 0 to 4 32 bytes of data for the index log parameter. In solidity, the first topic is event signatures
    pub topics: Vec<H256>,
    // One or more 32-byte un-indexed parameters containing this log
    pub data: Vec<u8>,
    // transaction hash
    pub tx_hash: HashValue,
    // transaction index in block
    pub tx_index: u64,
    // log index in block
    pub log_index_in_block: u64,
    // log index in transaction
    pub log_index_in_tx: u64,
    // returns true if the log has been deleted, false if it is a valid log
    pub removed: bool,
}

impl Log {
    pub fn new_from_eth_log_and_tx_hash(log: &EthLog, tx_hash: &HashValue) -> Self {
        Self {
            address: log.address,
            topics: log.topics.clone(),
            data: log.data.clone(),
            tx_hash: tx_hash.clone(),
            tx_index: 0,
            log_index_in_block: 0,
            log_index_in_tx: 0,
            removed: false,
        }
    }
}

#[derive(Vs, Clone, Debug, Default, Deserialize, Serialize)]
pub struct BlockHeader {
    // height of the current block
    pub height: BlockHeight,
    // proposer of the current block
    pub proposer: TmAddress,
    // timestamp of the current block
    pub timestamp: u64,
    // transaction merkle tree of the current block
    pub tx_merkle: TxMerkle,
    // hash of the previous block header
    pub prev_hash: HashValue,
    // execution results for each transaction
    pub receipts: MapxOrd<HashValue, Receipt>,
}

impl BlockHeader {
    #[inline(always)]
    fn hash(&self) -> HashValue {
        #[derive(Serialize)]
        struct Contents<'a> {
            height: BlockHeight,
            proposer: TmAddressRef<'a>,
            timestamp: u64,
            merkle_root: HashValueRef<'a>,
            prev_hash: HashValueRef<'a>,
            receipts: &'a MapxOrd<HashValue, Receipt>,
        }

        let contents = Contents {
            height: self.height,
            proposer: &self.proposer,
            timestamp: self.timestamp,
            merkle_root: &self.tx_merkle.root_hash,
            prev_hash: &self.prev_hash,
            receipts: &self.receipts,
        }
        .encode_value();

        hash_sha3_256(&[&contents])
    }
}

#[derive(Vs, Clone, Debug, Default, Deserialize, Serialize)]
pub struct TxMerkle {
    pub root_hash: HashValue,
    pub tree: MerkleTreeStore,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VsVersion {
    block_height: u64,
    // NOTE:
    // - starting from 1
    // - 0 is reserved for the block itself
    tx_position: u64,
}

impl VsVersion {
    pub fn new(block_height: BlockHeight, tx_position: u64) -> Self {
        Self {
            block_height,
            tx_position,
        }
    }
}

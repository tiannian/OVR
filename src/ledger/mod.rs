//!
//! # Ledger, world state
//!

pub(crate) mod staking;

use crate::{
    common::{
        block_hash_to_evm_format, hash_sha3_256, tm_proposer_to_evm_format, BlockHeight,
        HashValue, HashValueRef, TmAddress, TmAddressRef,
    },
    ethvm::{self, tx::GAS_PRICE_MIN},
    tx::Tx,
};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use primitive_types::{H160, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use std::{fs, io::ErrorKind, mem, sync::Arc};
use vsdb::{
    merkle::{MerkleTree, MerkleTreeStore},
    BranchName, MapxOrd, OrphanVs, ParentBranchName, ValueEn, Vs, VsMgmt,
};

const MAIN_BRANCH_NAME: BranchName = BranchName(b"Main");
const DELIVER_TX_BRANCH_NAME: BranchName = BranchName(b"DeliverTx");
const CHECK_TX_BRANCH_NAME: BranchName = BranchName(b"CheckTx");

static LEDGER_SNAPSHOT_PATH: Lazy<String> = Lazy::new(|| {
    let dir = format!("{}/or/ledger", vsdb::vsdb_get_custom_dir());
    pnk!(fs::create_dir_all(&dir));
    dir + "/ledger.json"
});

#[derive(Clone, Debug)]
pub(crate) struct Ledger {
    // used for web3 APIs
    pub(crate) state: State,
    pub(crate) main: Arc<RwLock<StateBranch>>,
    pub(crate) deliver_tx: Arc<RwLock<StateBranch>>,
    pub(crate) check_tx: Arc<RwLock<StateBranch>>,
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
        state.branch_create(MAIN_BRANCH_NAME).unwrap();
        state.branch_set_default(MAIN_BRANCH_NAME).unwrap();

        state.chain_id.set_value(chain_id).unwrap();
        state.chain_name.set_value(chain_name).unwrap();
        state.chain_version.set_value(chain_version).unwrap();

        state
            .evm
            .gas_price
            .set_value(gas_price.map(|v| v.into()).unwrap_or(*GAS_PRICE_MIN))
            .unwrap();
        state
            .evm
            .block_gas_limit
            .set_value(block_gas_limit.unwrap_or(u128::MAX).into())
            .unwrap();
        state
            .evm
            .block_base_fee_per_gas
            .set_value(block_base_fee_per_gas.unwrap_or_default().into())
            .unwrap();

        let main = StateBranch::new(&state, MAIN_BRANCH_NAME).c(d!())?;
        let deliver_tx = StateBranch::new(&state, DELIVER_TX_BRANCH_NAME).c(d!())?;
        let check_tx = StateBranch::new(&state, CHECK_TX_BRANCH_NAME).c(d!())?;

        state.branch_create(deliver_tx.branch_name()).unwrap();
        state.branch_create(check_tx.branch_name()).unwrap();

        Ok(Self {
            state,
            main: Arc::new(RwLock::new(main)),
            deliver_tx: Arc::new(RwLock::new(deliver_tx)),
            check_tx: Arc::new(RwLock::new(check_tx)),
        })
    }

    #[inline(always)]
    pub(crate) fn consensus_refresh(
        &self,
        proposer: TmAddress,
        timestamp: u64,
    ) -> Result<()> {
        self.refresh_inner(proposer, timestamp, false)
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
    pub(crate) fn commit(&self) -> Result<HashValue> {
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
pub(crate) struct StateBranch {
    pub(crate) state: State,
    pub(crate) branch: Vec<u8>,
    pub(crate) tx_hashes_in_process: Vec<HashValue>,
    pub(crate) block_in_process: Block,
}

impl StateBranch {
    #[inline(always)]
    fn new(state: &State, branch: BranchName) -> Result<Self> {
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
            .state
            .blocks
            .last()
            .map(|(h, b)| (h, b.header_hash))
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
    pub(crate) fn apply_tx(&mut self, tx: Tx) -> Result<()> {
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

        match tx {
            Tx::Evm(tx) => tx
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
                }),
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
                }),
        }
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
        self.block_in_process.header_hash = self.block_in_process.header.hash();

        let mut empty_block = Block::default();
        mem::swap(&mut empty_block, &mut self.block_in_process);
        let block = empty_block;

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
    // pub(crate) fn get_evm_state(&mut self) -> &ethvm::State {
    //     &self.state.evm
    // }

    #[inline(always)]
    pub(crate) fn get_evm_state_mut(&mut self) -> &mut ethvm::State {
        &mut self.state.evm
    }

    #[inline(always)]
    fn charge_fee(&self, caller: H160, amount: U256, b: BranchName) {
        alt!(amount.is_zero(), return);
        let mut account = self
            .state
            .evm
            .OFUEL
            .accounts
            .get_by_branch(&caller, b)
            .unwrap();
        account.balance = account.balance.saturating_sub(amount);
        self.state
            .evm
            .OFUEL
            .accounts
            .insert_by_branch(caller, account, b)
            .unwrap();
    }

    #[inline(always)]
    fn branch_name(&self) -> BranchName {
        self.branch.as_slice().into()
    }

    #[inline(always)]
    pub(crate) fn last_block(&self) -> Option<Block> {
        self.state.blocks.last().map(|(_, b)| b)
    }

    // #[inline(always)]
    // pub(crate) fn last_block_height(&self) -> BlockHeight {
    //     self.state.blocks.last().map(|(h, _)| h).unwrap_or(0)
    // }

    #[inline(always)]
    pub(crate) fn last_block_hash(&self) -> HashValue {
        self.last_block().unwrap_or_default().header_hash
    }

    #[inline(always)]
    fn load_from_snapshot() -> Result<Option<Self>> {
        match fs::read_to_string(&*LEDGER_SNAPSHOT_PATH) {
            Ok(c) => serde_json::from_str::<StateBranch>(&c).c(d!()).map(Some),
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
        let contents = serde_json::to_string_pretty(self).c(d!())?;
        fs::write(&*LEDGER_SNAPSHOT_PATH, &contents).c(d!())
    }

    #[inline(always)]
    fn clean_cache(&mut self) {
        self.tx_hashes_in_process.clear();
        self.block_in_process = Block::default();
    }
}

#[derive(Vs, Default, Clone, Debug, Deserialize, Serialize)]
pub(crate) struct State {
    pub(crate) chain_id: OrphanVs<u64>,
    pub(crate) chain_name: OrphanVs<String>,
    pub(crate) chain_version: OrphanVs<String>,

    pub(crate) evm: ethvm::State,
    pub(crate) staking: staking::State,

    // maintained by the 'main' branch only
    pub(crate) blocks: MapxOrd<BlockHeight, Block>,
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
pub(crate) struct Block {
    pub(crate) header: BlockHeader,
    pub(crate) header_hash: HashValue,
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
            },
            header_hash: Default::default(),
        }
    }
}

#[derive(Vs, Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct BlockHeader {
    // height of the current block
    pub(crate) height: BlockHeight,
    // proposer of the current block
    pub(crate) proposer: TmAddress,
    // timestamp of the current block
    pub(crate) timestamp: u64,
    // transaction merkle tree of the current block
    pub(crate) tx_merkle: TxMerkle,
    // hash of the previous block header
    pub(crate) prev_hash: HashValue,
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
        }

        let contents = Contents {
            height: self.height,
            proposer: &self.proposer,
            timestamp: self.timestamp,
            merkle_root: &self.tx_merkle.root_hash,
            prev_hash: &self.prev_hash,
        }
        .encode_value();

        hash_sha3_256(&[&contents])
    }
}

#[derive(Vs, Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct TxMerkle {
    pub(crate) root_hash: HashValue,
    pub(crate) tree: MerkleTreeStore,
}

#[derive(Deserialize, Serialize)]
struct VsVersion {
    block_height: u64,
    // NOTE:
    // - starting from 1
    // - 0 is reserved for the block itself
    tx_position: u64,
}

impl VsVersion {
    fn new(block_height: BlockHeight, tx_position: u64) -> Self {
        Self {
            block_height,
            tx_position,
        }
    }
}

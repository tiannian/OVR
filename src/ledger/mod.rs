//!
//! # Ledger, world state
//!

pub(crate) mod staking;

use crate::{
    common::{
        block_hash_to_evm_format, hash_sha3_256, tm_proposer_to_evm_format, BlockHeight,
        HashValue, HashValueRef, TmAddress, TmAddressRef,
    },
    ethvm,
    tx::Tx,
};
use primitive_types::{H160, U256};
use ruc::*;
use serde::{Deserialize, Serialize};
use std::mem;
use vsdb::{
    merkle::{MerkleTree, MerkleTreeStore},
    BranchName, MapxOrd, OrphanVs, ParentBranchName, ValueEn, Vs, VsMgmt,
};

const MAIN_BRANCH_NAME: BranchName = BranchName(b"Main");
const DELIVER_TX_BRANCH_NAME: BranchName = BranchName(b"DeliverTx");
const CHECK_TX_BRANCH_NAME: BranchName = BranchName(b"CheckTx");

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ledger {
    state: State,
    pub main: StateBranch,
    pub deliver_tx: StateBranch,
    pub check_tx: StateBranch,
}

impl Ledger {
    pub fn new(
        chain_id: u64,
        chain_name: String,
        chain_version: String,
        gas_price: u128,
        block_gas_limit: u128,
        block_base_fee_per_gas: u128,
    ) -> Self {
        let mut state = State::default();
        state.branch_create(MAIN_BRANCH_NAME).unwrap();
        state.branch_set_default(MAIN_BRANCH_NAME).unwrap();

        state.chain_id.set_value(chain_id).unwrap();
        state.chain_name.set_value(chain_name).unwrap();
        state.chain_version.set_value(chain_version).unwrap();

        state.evm.chain_id = chain_id.into();
        state.evm.gas_price.set_value(gas_price.into()).unwrap();
        state
            .evm
            .block_gas_limit
            .set_value(block_gas_limit.into())
            .unwrap();
        state
            .evm
            .block_base_fee_per_gas
            .set_value(block_base_fee_per_gas.into())
            .unwrap();

        let main = StateBranch::new(&state, MAIN_BRANCH_NAME);
        let deliver_tx = StateBranch::new(&state, DELIVER_TX_BRANCH_NAME);
        let check_tx = StateBranch::new(&state, CHECK_TX_BRANCH_NAME);

        state.branch_create(deliver_tx.branch_name()).unwrap();
        state.branch_create(check_tx.branch_name()).unwrap();

        Self {
            state,
            main,
            deliver_tx,
            check_tx,
        }
    }

    pub fn refresh(&self) -> Result<()> {
        self.state
            .branch_remove(self.check_tx.branch_name())
            .c(d!())?;
        self.state
            .branch_merge_to_parent(self.deliver_tx.branch_name())
            .c(d!())?;
        self.state
            .branch_create_by_base_branch(
                self.deliver_tx.branch_name(),
                ParentBranchName::from(&self.main.branch),
            )
            .c(d!())?;
        self.state.branch_create_by_base_branch(
            self.check_tx.branch_name(),
            ParentBranchName::from(&self.main.branch),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateBranch {
    pub(crate) state: State,
    pub(crate) branch: Vec<u8>,
    pub(crate) tx_hashes: Vec<HashValue>,
    pub(crate) block_in_process: Block,
}

impl StateBranch {
    #[inline(always)]
    fn new(state: &State, branch: BranchName) -> Self {
        Self {
            state: state.clone(),
            branch: branch.0.to_owned(),
            tx_hashes: vec![],
            block_in_process: Block::default(),
        }
    }

    /// NOTE:
    /// - Only used by the 'main' branch of `Ledger`
    /// - Call this in the 'Info' and 'Commit' field of ABCI
    /// - Should **NOT** be called in the 'BeginBlock' field of ABCI
    pub fn prepare_next_block(
        &mut self,
        proposer: TmAddress,
        timestamp: u64,
    ) -> Result<()> {
        self.tx_hashes.clear();

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

        self.update_evm_state(b);

        Ok(())
    }

    fn update_evm_state(&mut self, b: BranchName) {
        self.state.evm.chain_id =
            U256::from(self.state.chain_id.get_value_by_branch(b).unwrap());
        self.state.evm.block_coinbase =
            tm_proposer_to_evm_format(&self.block_in_process.header.proposer);
        self.state.evm.block_timestamp =
            U256::from(self.block_in_process.header.timestamp);
        self.state.evm.update_vicinity();
    }

    /// Deal with each transaction.
    /// Will be used by all the 3 branches of `Ledger`.
    pub fn apply_tx(&mut self, tx: Tx) -> Result<()> {
        let b = self.branch.clone();
        let b = b.as_slice().into();

        let ver = VsVersion::new(
            self.block_in_process.header.height,
            1 + self.tx_hashes.len() as u64,
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
                    self.tx_hashes.push(tx_hash);
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
                    self.tx_hashes.push(tx_hash);
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

    /// NOTE:
    /// - Only used by the 'main' branch of the `Ledger`
    pub fn end_block(&mut self) -> Result<()> {
        // Make it never empty,
        // thus the root hash will always exist
        self.tx_hashes.push(hash_sha3_256(&[&[]]));

        let hashes = self
            .tx_hashes
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

        Ok(())
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
            .OVRG
            .accounts
            .get_by_branch(&caller, b)
            .unwrap();
        account.balance = account.balance.saturating_sub(amount);
        self.state
            .evm
            .OVRG
            .accounts
            .insert_by_branch(caller, account, b)
            .unwrap();
    }

    #[inline(always)]
    fn branch_name(&self) -> BranchName {
        self.branch.as_slice().into()
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

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
use primitive_types::U256;
use ruc::*;
use serde::{Deserialize, Serialize};
use std::mem;
use vsdb::{
    merkle::{MerkleTree, MerkleTreeStore},
    MapxOrd, OrphanVs, ValueEn, Vs, VsMgmt,
};

pub type Ledger = TmStates;

#[derive(Deserialize, Serialize)]
pub struct TmStates {
    pub main: StateBranch,
    pub deliver_tx: StateBranch,
    pub check_tx: StateBranch,
}

#[derive(Deserialize, Serialize)]
pub struct StateBranch {
    pub(crate) state: State,
    pub(crate) branch: Vec<u8>,
    pub(crate) tx_hashes: Vec<HashValue>,
}

impl StateBranch {
    /// NOTE:
    /// - Only used by the 'main' branch of `TmStates`
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
        self.state.block_in_process = Block::new(1 + h, proposer, timestamp, prev_hash);

        let b = self.branch.clone();
        let b = b.as_slice().into();

        let ver = VsVersion::new(self.state.block_in_process.header.height, 0);
        self.state
            .version_create_by_branch(ver.encode_value().as_ref().into(), b)
            .c(d!())?;

        self.state.evm.chain_id =
            U256::from(self.state.chain_id.get_value_by_branch(b).unwrap());
        self.state.evm.block_coinbase =
            tm_proposer_to_evm_format(&self.state.block_in_process.header.proposer);
        self.state.evm.block_timestamp =
            U256::from(self.state.block_in_process.header.timestamp);
        self.state.evm.update_vicinity();

        Ok(())
    }

    /// Deal with each transaction.
    /// Will be used by all the 3 branches of `TmStates`.
    pub fn apply_tx(&mut self, tx: Tx) -> Result<()> {
        let b = self.branch.clone();
        let b = b.as_slice().into();

        let tx_hash = tx.hash();
        match tx {
            // evm has its own atomic cache, need NOT tx-level snapshot
            Tx::Evm(tx) => tx.apply(self, b).map_err(|e| eg!(@e)).map(|_| {
                self.tx_hashes.push(tx_hash);
            }),
            Tx::Native(tx) => {
                let ver = VsVersion::new(
                    self.state.block_in_process.header.height,
                    1 + self.tx_hashes.len() as u64,
                );

                self.state
                    .version_create_by_branch(ver.encode_value().as_ref().into(), b)
                    .c(d!())?;

                tx.apply(self, b)
                    .c(d!())
                    .map(|_| {
                        self.tx_hashes.push(tx_hash);
                    })
                    .map_err(|e| {
                        pnk!(self.state.version_pop_by_branch(b));
                        e
                    })
            }
        }
    }

    /// NOTE:
    /// - Only used by the 'main' branch of the `TmStates`
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

        self.state.block_in_process.header.tx_merkle.tree = mt.into();
        self.state.block_in_process.header.tx_merkle.root_hash = root;
        self.state.block_in_process.header_hash =
            self.state.block_in_process.header.hash();

        let mut empty_block = Block::default();
        mem::swap(&mut empty_block, &mut self.state.block_in_process);
        let block = empty_block;

        self.state.evm.block_hashes.insert(
            block.header.height,
            block_hash_to_evm_format(&block.header_hash),
        );

        self.state.blocks.insert(block.header.height, block);

        Ok(())
    }
}

#[derive(Vs, Deserialize, Serialize)]
pub(crate) struct State {
    pub(crate) chain_id: OrphanVs<u64>,
    pub(crate) chain_name: OrphanVs<String>,
    pub(crate) chain_version: OrphanVs<String>,

    pub(crate) evm: ethvm::State,
    pub(crate) staking: staking::State,

    pub(crate) blocks: MapxOrd<BlockHeight, Block>,
    pub(crate) block_in_process: Block,
}

#[derive(Vs, Default, Deserialize, Serialize)]
pub(crate) struct Block {
    pub(crate) header: BlockHeader,
    pub(crate) header_hash: HashValue,
}

impl Block {
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

#[derive(Vs, Default, Deserialize, Serialize)]
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

#[derive(Vs, Default, Deserialize, Serialize)]
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

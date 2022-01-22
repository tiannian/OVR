//!
//! # Ledger, world state
//!

pub(crate) mod staking;

use crate::{ethvm, tx::Tx};
use primitive_types::H160;
use ruc::*;
use std::mem::size_of;
use vsdb::{merkle::MerkleTreeStore, BranchName, MapxOrd, OrphanVs};

pub(crate) type BlockHeight = u64;
// BigEndian
pub(crate) type BlockHeightBytes = [u8; size_of::<u64>()];

pub(crate) type HashValue = Vec<u8>;

pub(crate) type TmAddress = Vec<u8>;
pub(crate) type TmAddressRef<'a> = &'a [u8];

pub struct Ledger {
    pub tm_states: TmStates,
}

pub struct TmStates {
    pub main: StateBranch,
    pub deliver_tx: StateBranch,
    pub check_tx: StateBranch,
}

pub struct StateBranch {
    pub(crate) state: State,
    pub(crate) branch: Vec<u8>,
}

pub(crate) struct State {
    pub(crate) chain_id: OrphanVs<u64>,
    pub(crate) chain_name: OrphanVs<String>,
    pub(crate) chain_version: OrphanVs<String>,

    pub(crate) blocks: MapxOrd<BlockHeight, Block>,

    pub(crate) evm: ethvm::State,
    pub(crate) staking: staking::State,
}

pub(crate) struct Block {
    pub(crate) header: BlockHeader,
    pub(crate) header_hash: HashValue,
}

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

pub(crate) struct TxMerkle {
    pub(crate) root_hash: HashValue,
    pub(crate) tree: MerkleTreeStore,
}

impl StateBranch {
    pub fn apply_tx(&mut self, tx: Tx) -> Result<()> {
        match tx {
            Tx::Evm(tx) => tx.apply(self).c(d!()),
            Tx::Native(tx) => tx.apply(self).c(d!()),
        }
    }
}

// `evm block coinbase` == `H160(tendermint proposer address)`
fn tm_proposer_to_evm_coinbase(addr: TmAddressRef) -> H160 {
    todo!()
}

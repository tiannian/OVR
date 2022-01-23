use primitive_types::{H160, H256};
use ruc::*;
use sha3::{Digest, Sha3_256};
use std::mem::size_of;

pub(crate) type BlockHeight = u64;

pub(crate) type HashValue = Vec<u8>;
pub(crate) type HashValueRef<'a> = &'a [u8];

pub(crate) type TmAddress = Vec<u8>;
pub(crate) type TmAddressRef<'a> = &'a [u8];

/// global hash function
pub fn hash_sha3_256(contents: &[&[u8]]) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    for c in contents {
        hasher.update(c);
    }
    hasher.finalize().to_vec()
}

/// block proposer address of tendermint ==> evm coinbase address
pub fn tm_proposer_to_evm_format(addr: TmAddressRef) -> H160 {
    const LEN: usize = H160::len_bytes();

    let mut buf = [0_u8; LEN];
    buf.copy_from_slice(&addr[..min!(LEN, addr.len())]);

    H160::from_slice(&buf)
}

/// block proposer address of tendermint ==> evm coinbase address
pub fn block_hash_to_evm_format(hash: &HashValue) -> H256 {
    const LEN: usize = H256::len_bytes();

    let mut buf = [0; LEN];
    buf.copy_from_slice(&hash[..min!(LEN, hash.len())]);

    H256::from_slice(&buf)
}

use ethereum_types::H256;
use jsonrpc_core::{BoxFuture, Result};
use sha3::{Digest, Keccak256};
use web3_rpc_core::{types::Bytes, Web3Api};

pub struct Web3ApiImpl {}

async fn client_version() -> Result<String> {
    Ok(String::new())
}

impl Web3Api for Web3ApiImpl {
    fn client_version(&self) -> BoxFuture<Result<String>> {
        Box::pin(async move { client_version().await })
    }

    fn sha3(&self, input: Bytes) -> Result<H256> {
        Ok(H256::from_slice(
            Keccak256::digest(&input.into_vec()).as_slice(),
        ))
    }
}

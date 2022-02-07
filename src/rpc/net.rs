use jsonrpc_core::{BoxFuture, Result};
use web3_rpc_core::{types::PeerCount, NetApi};

pub struct NetApiImpl {}

impl NetApi for NetApiImpl {
    fn version(&self) -> BoxFuture<Result<String>> {
        Box::pin(async { Ok(0.to_string()) })
    }

    fn peer_count(&self) -> BoxFuture<Result<PeerCount>> {
        // try to get infomation from tendermint.
        Box::pin(async move { Ok(PeerCount::U32(0)) })
    }

    fn is_listening(&self) -> BoxFuture<Result<bool>> {
        // try to get infomation from tendermint.
        Box::pin(async move { Ok(true) })
    }
}

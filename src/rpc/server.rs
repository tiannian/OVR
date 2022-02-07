use std::{net::SocketAddr, thread, sync::Arc};

use crate::ledger::StateBranch;

use super::{eth::EthApiImpl, net::NetApiImpl, web3::Web3ApiImpl};
use parking_lot::RwLock;
use web3_rpc_core::{EthApi, NetApi, Web3Api};

pub(crate) struct Web3ServerBuilder {
    pub upstream: String,
    pub http: SocketAddr,
    pub ws: SocketAddr,
    pub main: Arc<RwLock<StateBranch>>,
}

impl Web3ServerBuilder {
    fn build_http(&self) -> jsonrpc_http_server::Server {
        let upstream = self.upstream.clone();

        let mut io = jsonrpc_core::IoHandler::new();

        let eth = EthApiImpl {
            upstream: upstream.clone(),
            main: self.main.clone(),
        };

        let net = NetApiImpl {};

        let web3 = Web3ApiImpl {};

        io.extend_with(eth.to_delegate());
        io.extend_with(net.to_delegate());
        io.extend_with(web3.to_delegate());

        jsonrpc_http_server::ServerBuilder::new(io.clone())
            .start_http(&self.http)
            .expect("failed to create http server")
    }

    fn build_ws(&self) -> jsonrpc_ws_server::Server {
        let upstream = self.upstream.clone();

        let mut io = jsonrpc_core::IoHandler::new();

        let eth = EthApiImpl {
            upstream: upstream.clone(),
            main: self.main.clone(),
        };

        let net = NetApiImpl {};

        let web3 = Web3ApiImpl {};

        io.extend_with(eth.to_delegate());
        io.extend_with(net.to_delegate());
        io.extend_with(web3.to_delegate());

        jsonrpc_ws_server::ServerBuilder::new(io.clone())
            .start(&self.ws)
            .expect("failed to create http server")
    }
    pub fn build(self) -> Web3Server {
        let http = self.build_http();

        let ws = self.build_ws();

        Web3Server {
            http,
            ws,
        }
    }
}

pub struct Web3Server {
    http: jsonrpc_http_server::Server,
    ws: jsonrpc_ws_server::Server,
}

impl Web3Server {
    pub fn start(self) {
        let _ = thread::spawn(move || {
            self.http.wait();
        });

        let _ = thread::spawn(move || {
            self.ws.wait().expect("ws start error");
        });
    }
}

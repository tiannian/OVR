//!
//! # Web3 compatible APIs
//!

#![allow(warnings)]

use evm::{ExitError, ExitReason};
use f_rpc_core::{
    types::pubsub::Metadata, EthApiServer, EthFilterApiServer, EthPubSubApiServer,
    NetApiServer, Web3ApiServer,
};
use f_rpc_server::{rpc_handler, start_http, start_ws, RpcHandler, RpcMiddleware};
use f_utils::ecdsa::SecpPair;
use jsonrpc_core::types::error::{Error, ErrorCode};
use log::error;
use ruc::*;
use rustc_hex::ToHex;
use serde_json::Value;
use std::net::SocketAddr;

pub use waiting::{HttpServer, WsServer};

const MAX_PAST_LOGS: u32 = 10000;
const MAX_STORED_FILTERS: usize = 500;

// TODO
pub fn start_web3_service(
    http_serv_list: &[SocketAddr],
    ws_serv_list: &[SocketAddr],
) -> Result<(Vec<HttpServer>, Vec<WsServer>)> {
    // PrivateKey: 9f7bebaa5c55464b10150bc2e0fd552e915e2bdbca95cc45ed1c909aca96e7f5
    // Address: 0xf6aca39539374993b37d29ccf0d93fa214ea0af1
    let dev_signer = "zebra paddle unveil toilet weekend space gorilla lesson relief useless arrive picture";
    let signers = vec![SecpPair::from_phrase(dev_signer, None).unwrap().0];

    let io = || -> RpcHandler<Metadata> { rpc_handler((), RpcMiddleware::new()) };

    let http_hdrs = http_serv_list
        .into_iter()
        .map(|addr| {
            println!("*** Web3-http serve at {} ***", addr);
            start_http(addr, None, Some(&vec!["*".to_string()]), io(), None)
                .c(d!("Unable to start web3 http service"))
                .map(|s| HttpServer(Some(s)))
        })
        .collect::<Result<Vec<_>>>()?;

    let ws_hdrs = ws_serv_list
        .into_iter()
        .map(|addr| {
            println!("*** Web3-websocket serve at {} ***", addr);
            start_ws(addr, None, Some(&vec!["*".to_string()]), io(), None)
                .c(d!("Unable to start web3 ws service"))
                .map(|s| WsServer(Some(s)))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok((http_hdrs, ws_hdrs))
}

// Wrapper for HTTP and WS servers that makes sure they are properly shut down.
mod waiting {
    pub struct HttpServer(pub Option<f_rpc_server::HttpServer>);
    impl Drop for HttpServer {
        fn drop(&mut self) {
            if let Some(server) = self.0.take() {
                server.close_handle().close();
                server.wait();
            }
        }
    }

    pub struct IpcServer(pub Option<f_rpc_server::IpcServer>);
    impl Drop for IpcServer {
        fn drop(&mut self) {
            if let Some(server) = self.0.take() {
                server.close_handle().close();
                let _ = server.wait();
            }
        }
    }

    pub struct WsServer(pub Option<f_rpc_server::WsServer>);
    impl Drop for WsServer {
        fn drop(&mut self) {
            if let Some(server) = self.0.take() {
                server.close_handle().close();
                let _ = server.wait();
            }
        }
    }
}

pub fn internal_err<T: ToString>(message: T) -> Error {
    error!(target: "eth_rpc", "internal error: {:?}", message.to_string());
    Error {
        code: ErrorCode::InternalError,
        message: message.to_string(),
        data: None,
    }
}

pub fn error_on_execution_failure(
    reason: &ExitReason,
    data: &[u8],
) -> std::result::Result<(), Error> {
    match reason {
        ExitReason::Succeed(_) => Ok(()),
        ExitReason::Error(e) => {
            if *e == ExitError::OutOfGas {
                // `ServerError(0)` will be useful in estimate gas
                return Err(Error {
                    code: ErrorCode::ServerError(0),
                    message: "out of gas".to_string(),
                    data: None,
                });
            }
            Err(Error {
                code: ErrorCode::InternalError,
                message: format!("evm error: {:?}", e),
                data: Some(Value::String("0x".to_string())),
            })
        }
        ExitReason::Revert(_) => {
            let mut message =
                "VM Exception while processing transaction: revert".to_string();
            // A minimum size of error function selector (4) + offset (32) + string length (32)
            // should contain a utf-8 encoded revert reason.
            if data.len() > 68 {
                let message_len = data[36..68].iter().sum::<u8>();
                let body: &[u8] = &data[68..68 + message_len as usize];
                if let Ok(reason) = std::str::from_utf8(body) {
                    message = format!("{} {}", message, reason);
                }
            }
            Err(Error {
                code: ErrorCode::InternalError,
                message,
                data: Some(Value::String(data.to_hex())),
            })
        }
        ExitReason::Fatal(e) => Err(Error {
            code: ErrorCode::InternalError,
            message: format!("evm fatal: {:?}", e),
            data: Some(Value::String("0x".to_string())),
        }),
    }
}

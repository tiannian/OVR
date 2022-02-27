use jsonrpc_core::{Error, ErrorCode};
use serde_json::Value;

pub fn no_impl() -> Error {
    Error {
        code: ErrorCode::ServerError(40001),
        message: String::from("No impl."),
        data: None,
    }
}

pub fn new_jsonrpc_error(msg: &str, data: Value) -> Error {
    Error {
        code: ErrorCode::ServerError(40002),
        message: msg.to_string(),
        data: Some(data),
    }
}

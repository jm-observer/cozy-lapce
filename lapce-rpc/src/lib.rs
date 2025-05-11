#![allow(clippy::manual_clamp)]

pub mod buffer;
pub mod core;
pub mod counter;
pub mod dap_types;
pub mod file;
pub mod file_line;
mod parse;
pub mod plugin;
pub mod proxy;
pub mod rust_module_resolve;
pub mod source_control;
pub mod stdio;
pub mod style;
pub mod terminal;

use std::fmt::Display;

pub use parse::{Call, RequestId, RpcObject};
use serde::{Deserialize, Serialize};
pub use stdio::stdio_transport;

#[derive(Debug)]
pub enum RpcMessage<Req, Notif, Resp> {
    Request(RequestId, Req),
    Response(RequestId, Resp),
    Notification(Notif),
    Error(RequestId, RpcError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code:    i64,
    pub message: String,
}

impl Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RpcError(code: {}, message: {})",
            self.code, self.message
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RpcResult<T: std::fmt::Debug + Clone + Send + Sync + 'static> {
    Err(String),
    Ok(T),
}

impl<T: std::fmt::Debug + Clone + Send + Sync + 'static> From<String>
    for RpcResult<T>
{
    fn from(value: String) -> Self {
        RpcResult::Err(value)
    }
}

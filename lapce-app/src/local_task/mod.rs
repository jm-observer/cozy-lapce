mod requester;
mod handler;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use parking_lot::Mutex;
use lapce_rpc::{RequestId};
use lapce_rpc::plugin::{VoltInfo, VoltMetadata};
use crate::local_task::handler::LocalTaskHandler;
use anyhow::Result;
use lapce_xi_rope::spans::Spans;
use lapce_core::directory::Directory;
use lapce_rpc::style::SemanticStyles;
pub use requester::LocalTaskRequester;

pub trait LocalCallback:
Send + FnOnce((u64, Result<LocalResponse>)) {
}

impl<F: Send + FnOnce((u64, Result<LocalResponse>))> LocalCallback for F {}

enum LocalResponseHandler {
    Callback(Box<dyn LocalCallback>),
    // Chan(Sender<(u64, Result<LocalResponse>)>)
}

impl LocalResponseHandler {
    fn invoke(self, id: u64, result: Result<LocalResponse>) {
        match self {
            LocalResponseHandler::Callback(f) => f((id, result)),
            // LocalResponseHandler::Chan(tx) => {
            //     if let Err(err) = tx.send((id, result)) {
            //         log::error!("{:?}", err);
            //     }
            // },
        }
    }
}


pub enum LocalRpc {
    Request {
        id: RequestId, request: LocalRequest
    },
    Notification {
        notification: LocalNotification,
    },
    // Shutdown
}

pub enum LocalRequest {
    FindAllVolts {
        extra_plugin_paths:       Arc<Vec<PathBuf>>
    },
    SpansBuilder {
        len: usize,
        styles:    SemanticStyles,
        result_id: Option<String>
    },
    InstallVolt {
        info: VoltInfo
    },
    QueryVoltInfo {
        meta: VoltMetadata
    }
}
pub enum LocalResponse {
    FindAllVolts {
        volts:       Vec<VoltMetadata>
    },
    SpansBuilder {
        styles: Spans<String>,
        result_id: Option<String>
    },
    InstallVolt {
        volt: VoltMetadata, icon: Option<Vec<u8>>
    },
    QueryVoltInfo {
        info: VoltInfo
    },
}

pub enum LocalNotification {
}



pub fn new_local_handler(directory: Directory) -> Result<LocalTaskRequester> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let pending =  Arc::new(Mutex::new(HashMap::new()));
    let requester = LocalTaskRequester::new(tx, pending.clone());
    let _handler = LocalTaskHandler {
        rx, pending, directory
    };
    start_proxy(_handler)?;
    Ok(requester)
}


fn start_proxy(_handler: LocalTaskHandler) -> Result<thread::JoinHandle<()>> {
    Ok(thread::Builder::new().name("Local Task Handler".to_string()).spawn(move || {
        main_start_proxy(_handler);
    })?)
}

#[tokio::main]
async fn main_start_proxy(_handler: LocalTaskHandler) {
    _handler.handle().await;
}


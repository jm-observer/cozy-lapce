mod handler;
mod requester;

use std::{collections::HashMap, path::PathBuf, sync::Arc, thread};

use anyhow::Result;
use crossbeam_channel::Receiver;
use lapce_core::directory::Directory;
use lapce_rpc::{
    RequestId,
    plugin::{VoltInfo, VoltMetadata},
    style::SemanticStyles,
};
use lapce_xi_rope::spans::Spans;
use parking_lot::Mutex;
pub use requester::LocalTaskRequester;

use crate::{
    config::LapceConfig,
    db::{LapceDb, SaveEvent},
    local_task::handler::LocalTaskHandler,
    markdown::MarkdownContent,
    plugin::{VoltIcon, VoltsInfo},
};

pub trait LocalCallback: Send + FnOnce((u64, Result<LocalResponse>)) {}

impl<F: Send + FnOnce((u64, Result<LocalResponse>))> LocalCallback for F {}

enum LocalResponseHandler {
    Callback(Box<dyn LocalCallback>), // Chan(Sender<(u64, Result<LocalResponse>)>)
}

impl LocalResponseHandler {
    fn invoke(self, id: u64, result: Result<LocalResponse>) {
        match self {
            LocalResponseHandler::Callback(f) => f((id, result)), /* LocalResponseHandler::Chan(tx) => {
                                                                   *     if let
                                                                   * Err(err) =
                                                                   * tx.send((id,
                                                                   * result)) {
                                                                   *         log::error!("{:?}", err);
                                                                   *     }
                                                                   * }, */
        }
    }
}

pub enum LocalRpc {
    Request {
        id:      RequestId,
        request: LocalRequest,
    },
    Notification {
        notification: LocalNotification,
    }, // Shutdown
}

#[derive(Debug)]
pub enum LocalRequest {
    FindAllVolts {
        extra_plugin_paths: Arc<Vec<PathBuf>>,
    },
    SpansBuilder {
        len:       usize,
        styles:    SemanticStyles,
        result_id: Option<String>,
    },
    InstallVolt {
        info: VoltInfo,
    },
    QueryVoltInfo {
        meta: VoltMetadata,
    },
    QueryVolts {
        query:  String,
        offset: usize,
    },
    LoadIcon {
        info: VoltInfo,
    },
    UninstallVolt {
        dir: PathBuf,
    },
    DownloadVoltReadme {
        info: VoltInfo,
    },
    FindGrammar,
}
pub enum LocalResponse {
    FindAllVolts {
        volts: Vec<VoltMetadata>,
    },
    SpansBuilder {
        styles:    Spans<String>,
        result_id: Option<String>,
    },
    InstallVolt {
        volt: VoltMetadata,
        icon: Option<Vec<u8>>,
    },
    QueryVoltInfo {
        info: VoltInfo,
    },
    QueryVolts {
        volts: VoltsInfo,
    },
    LoadIcon {
        icon: VoltIcon,
    },
    UninstallVolt,
    DownloadVoltReadme {
        readme: Vec<MarkdownContent>,
    },
    FindGrammar {
        updated: bool,
    },
}

pub enum LocalNotification {
    DbSaveEvent(SaveEvent),
}

pub fn new_local_handler(
    directory: Directory,
    config: LapceConfig,
    db: Arc<LapceDb>,
) -> Result<LocalTaskRequester> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let requester = LocalTaskRequester::new(tx, pending.clone());
    let _handler = LocalTaskHandler {
        config,
        pending,
        directory,
        db,
    };
    start_proxy(_handler, rx)?;
    Ok(requester)
}

fn start_proxy(
    _handler: LocalTaskHandler,
    rx: Receiver<LocalRpc>,
) -> Result<thread::JoinHandle<()>> {
    Ok(thread::Builder::new()
        .name("Local Task Handler".to_string())
        .spawn(move || {
            main_start_proxy(_handler, rx);
        })?)
}

#[tokio::main]
async fn main_start_proxy(mut _handler: LocalTaskHandler, rx: Receiver<LocalRpc>) {
    _handler.handle(rx).await;
}

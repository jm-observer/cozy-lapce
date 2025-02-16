use crate::local_task::{
    LocalRequest, LocalResponse, LocalResponseHandler, LocalRpc,
};
use anyhow::Result;
use crossbeam_channel::Receiver;
use lapce_core::directory::Directory;
use lapce_proxy::plugin::wasi::find_all_volts;
use lapce_proxy::plugin::{async_volt_icon, download_volt};
use lapce_rpc::RequestId;
use lapce_rpc::plugin::VoltInfo;
use lapce_xi_rope::Interval;
use lapce_xi_rope::spans::SpansBuilder;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc};
use lapce_rpc::style::SemanticStyles;

#[derive(Clone)]
pub struct LocalTaskHandler {
    pub directory: Directory,
    pub(crate) rx: Receiver<LocalRpc>,
    pub(crate) pending: Arc<Mutex<HashMap<u64, LocalResponseHandler>>>,
}

impl LocalTaskHandler {
    pub async fn handle(&self) {
        use crate::local_task::LocalRpc::*;
        for msg in &self.rx {
            match msg {
                Request { id, request } => match request {
                    LocalRequest::FindAllVolts { extra_plugin_paths } => {
                        let plugin_dir = self.directory.plugins_directory.clone();
                        let pending = self.pending.clone();
                        tokio::spawn(async move {
                            let rs = handle_find_all_volts(extra_plugin_paths, plugin_dir).await;
                            handle_response(id, rs, pending);
                        });
                    },
                    LocalRequest::SpansBuilder {
                        len,
                        styles,
                        result_id,
                    } => {
                        let pending = self.pending.clone();
                        tokio::spawn(async move {
                            let rs = handle_spans_builder(len, styles, result_id).await;
                            handle_response(id, rs, pending);
                        });
                    },
                    LocalRequest::InstallVolt { info } => {
                        let plugin_dir = self.directory.plugins_directory.clone();
                        let pending = self.pending.clone();
                        tokio::spawn(async move {
                            let rs = handle_install_volt(info, plugin_dir).await;
                            handle_response(id, rs, pending);
                        });
                    },
                },
                Notification { notification: _notification } => {},
                // Shutdown => {
                //     return;
                // },
            }
        }
    }

    // pub fn handle_response(&self, id: RequestId, result: Result<LocalResponse>) {
    //     let handler = { self.pending.lock().remove(&id) };
    //     if let Some(handler) = handler {
    //         handler.invoke(id, result);
    //     }
    // }
}


async fn handle_spans_builder(
    len: usize,
    styles:    SemanticStyles,
    result_id: Option<String>
) -> Result<LocalResponse> {
    let mut styles_span = SpansBuilder::new(len);
    for style in styles.styles {
        if let Some(fg) = style.style.fg_color {
            styles_span.add_span(
                Interval::new(style.start, style.end),
                fg,
            );
        }
    }
    let styles = styles_span.build();
    Ok(LocalResponse::SpansBuilder { styles, result_id })
}
async fn handle_find_all_volts(
    extra_plugin_paths:       Arc<Vec<PathBuf>>,
    plugin_dir: PathBuf,
) -> Result<LocalResponse> {
    let volts = find_all_volts(
        &extra_plugin_paths,
        &plugin_dir,
    )
        .await;
    Ok(LocalResponse::FindAllVolts { volts })
}

async fn handle_install_volt(
    info: VoltInfo,
    plugin_dir: PathBuf,
) -> Result<LocalResponse> {
    let volt = download_volt(&info, &plugin_dir).await?;
    let icon = async_volt_icon(&volt).await;
    Ok(LocalResponse::InstallVolt { volt, icon })
}

fn handle_response(
    id: RequestId,
    result: Result<LocalResponse>,
    pending: Arc<Mutex<HashMap<u64, LocalResponseHandler>>>,
) {
    let handler = { pending.lock().remove(&id) };
    if let Some(handler) = handler {
        handler.invoke(id, result);
    }
}

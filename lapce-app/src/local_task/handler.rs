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
use sha2::{Digest, Sha256};
use lapce_rpc::style::SemanticStyles;
use crate::plugin::{VoltIcon, VoltsInfo};

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
                    LocalRequest::QueryVoltInfo { meta } => {
                        let pending = self.pending.clone();
                        tokio::spawn(async move {
                            let url = format!(
                                "https://plugins.lapce.dev/api/v1/plugins/{}/{}/latest",
                                meta.author, meta.name
                            );
                            let rs = handle_query_volt_info(url).await;
                            handle_response(id, rs, pending);
                        });
                    }
                    LocalRequest::QueryVolts { query, offset } => {
                        let pending = self.pending.clone();
                        tokio::spawn(async move {
                            let rs = handle_query_volts(&query, offset).await;
                            handle_response(id, rs, pending);
                        });
                    }
                    LocalRequest::LoadIcon { info } => {
                        let pending = self.pending.clone();
                        let cache_directory = self.directory.cache_directory.clone();
                        tokio::spawn(async move {
                            let rs = handle_load_icon(&info, cache_directory).await;
                            handle_response(id, rs, pending);
                        });
                    }
                },
                Notification { notification: _notification } => {},
                // Shutdown => {
                //     return;
                // },
            }
        }
    }
}

async fn handle_query_volts(
    query: &str, offset: usize
) -> Result<LocalResponse> {
    let url = format!(
        "https://plugins.lapce.dev/api/v1/plugins?q={query}&offset={offset}"
    );
    let volts: VoltsInfo = lapce_proxy::async_get_url(url, None).await?.json().await?;
    Ok(LocalResponse::QueryVolts { volts})
}

async fn handle_load_icon(
    volt: &VoltInfo,
    cache_directory: Option<PathBuf>,
) -> Result<LocalResponse> {
    let url = format!(
        "https://plugins.lapce.dev/api/v1/plugins/{}/{}/{}/icon?id={}",
        volt.author, volt.name, volt.version, volt.updated_at_ts
    );

    let cache_file_path = cache_directory.map(|cache_dir| {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let filename = format!("{:x}", hasher.finalize());
        cache_dir.join(filename)
    });

    if let Some(cache_file) = &cache_file_path {
        if cache_file.exists() {
            let icon = VoltIcon::from_bytes(&tokio::fs::read(cache_file).await?)?;
            return Ok(LocalResponse::LoadIcon {icon});
        }
    }
    let resp = lapce_proxy::async_get_url(&url, None).await?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("can't download icon"));
    }
    let buf = resp.bytes().await?.to_vec();
    if let Some(path) = cache_file_path.as_ref() {
        tokio::fs::write(path, &buf).await?
    }
    let icon = VoltIcon::from_bytes(&buf)?;
    Ok(LocalResponse::LoadIcon {icon})
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

async fn handle_query_volt_info(
    url: String,
) -> Result<LocalResponse> {
    let info: VoltInfo = lapce_proxy::async_get_url(url, None).await?.json().await?;
    Ok(LocalResponse::QueryVoltInfo { info })
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

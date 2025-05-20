use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, anyhow};
use crossbeam_channel::Receiver;
use doc::lines::{buffer::diff::rope_diff, selection::Selection};
use floem::{prelude::Color, text::FamilyOwned};
use lapce_core::directory::Directory;
use lapce_proxy::plugin::{async_volt_icon, download_volt, wasi::find_all_volts};
use lapce_rpc::{RequestId, plugin::VoltInfo, style::SemanticStyles};
use lapce_xi_rope::{
    Interval, RopeInfo, find::CaseMatching, spans::SpansBuilder, tree::Node,
};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

use crate::{
    config::{LapceConfig, color::LapceColor},
    db::{LapceDb, SaveEvent},
    find::{Find, FindSearchString},
    local_task::{
        LocalNotification, LocalRequest, LocalResponse, LocalResponseHandler,
        LocalRpc,
    },
    markdown::parse_markdown,
    plugin::{VoltIcon, VoltsInfo},
};

#[derive(Clone)]
pub struct LocalTaskHandler {
    pub directory:      Directory,
    pub config:         LapceConfig,
    pub(crate) pending: Arc<Mutex<HashMap<u64, LocalResponseHandler>>>,
    pub db:             Arc<LapceDb>,
}

impl LocalTaskHandler {
    pub async fn handle(&mut self, rx: Receiver<LocalRpc>) {
        use crate::local_task::LocalRpc::*;
        for msg in &rx {
            match msg {
                Request { id, request } => {
                    self.handle_request(id, request).await;
                },
                Notification {
                    notification: _notification,
                } => match _notification {
                    LocalNotification::DbSaveEvent(event) => {
                        let db = self.db.clone();
                        tokio::spawn(async move {
                            handle_notification_db_save_event(db, event).await;
                        });
                    },
                }, /* Shutdown => {
                    *     return;
                    * }, */
            }
        }
    }

    pub async fn handle_request(&mut self, id: RequestId, request: LocalRequest) {
        // debug!("handler handle_request {request:?}");
        match request {
            LocalRequest::FindAllVolts { extra_plugin_paths } => {
                let plugin_dir = self.directory.plugins_directory.clone();
                let pending = self.pending.clone();
                tokio::spawn(async move {
                    let rs =
                        handle_find_all_volts(extra_plugin_paths, plugin_dir).await;
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
            },
            LocalRequest::QueryVolts { query, offset } => {
                let pending = self.pending.clone();
                tokio::spawn(async move {
                    let rs = handle_query_volts(&query, offset).await;
                    handle_response(id, rs, pending);
                });
            },
            LocalRequest::LoadIcon { info } => {
                let pending = self.pending.clone();
                let cache_directory = self.directory.cache_directory.clone();
                tokio::spawn(async move {
                    let rs = handle_load_icon(&info, cache_directory).await;
                    handle_response(id, rs, pending);
                });
            },
            LocalRequest::UninstallVolt { dir } => {
                let pending = self.pending.clone();
                tokio::spawn(async move {
                    let rs = handle_uninstall_volt(&dir).await;
                    handle_response(id, rs, pending);
                });
            },
            LocalRequest::DownloadVoltReadme { info } => {
                let pending = self.pending.clone();
                let (
                    font_family,
                    editor_fg,
                    style_colors,
                    font_size,
                    markdown_blockquote,
                    editor_link,
                ) = (
                    FamilyOwned::parse_list(&self.config.editor.font_family)
                        .collect::<Vec<FamilyOwned>>(),
                    self.config.color(LapceColor::EDITOR_FOREGROUND),
                    self.config.style_colors(),
                    self.config.ui.font_size() as f32,
                    self.config.color(LapceColor::MARKDOWN_BLOCKQUOTE),
                    self.config.color(LapceColor::EDITOR_LINK),
                );

                let dir = self.directory.clone();
                tokio::spawn(async move {
                    let rs = handle_download_readme(
                        &info,
                        &dir,
                        &font_family,
                        editor_fg,
                        &style_colors,
                        font_size,
                        markdown_blockquote,
                        editor_link,
                    )
                    .await;
                    handle_response(id, rs, pending);
                });
            },
            LocalRequest::FindGrammar => {
                let pending = self.pending.clone();
                let grammars_directory = self.directory.grammars_directory.clone();
                let queries_directory = self.directory.queries_directory.clone();
                tokio::spawn(async move {
                    let rs =
                        handle_find_grammar(&grammars_directory, &queries_directory)
                            .await;
                    handle_response(id, rs, pending);
                });
            },
            LocalRequest::FindText {
                text,
                case_matching,
                whole_words,
                search,
            } => {
                let pending = self.pending.clone();

                tokio::spawn(async move {
                    let rs =
                        handle_find_text(text, case_matching, whole_words, search)
                            .await;
                    handle_response(id, rs, pending);
                });
            },

            LocalRequest::RopeDiff {
                left_rope,
                right_rope,
                rev,
                atomic_rev,
                context_lines,
            } => {
                let pending = self.pending.clone();

                tokio::spawn(async move {
                    let changes = rope_diff(
                        left_rope,
                        right_rope,
                        rev,
                        atomic_rev,
                        context_lines,
                    );
                    handle_response(
                        id,
                        Ok(LocalResponse::RopeDiff { changes, rev }),
                        pending,
                    );
                });
            },
            LocalRequest::SyntaxParse {
                rev,
                text,
                edits,
                mut syntax,
            } => {
                let pending = self.pending.clone();
                let grammars_directory = self.directory.grammars_directory.clone();
                let queries_directory = self.directory.queries_directory.clone();

                tokio::spawn(async move {
                    syntax.parse(
                        rev,
                        text,
                        edits.as_deref(),
                        &grammars_directory,
                        &queries_directory,
                    );
                    handle_response(
                        id,
                        Ok(LocalResponse::SyntaxParse { syntax }),
                        pending,
                    );
                });
            },
        }
    }
}

async fn handle_find_text(
    text: Node<RopeInfo>,
    case_matching: CaseMatching,
    whole_words: bool,
    search: FindSearchString,
) -> Result<LocalResponse> {
    let mut occurrences = Selection::new();
    Find::find(
        &text,
        &search,
        0,
        text.len(),
        case_matching,
        whole_words,
        true,
        &mut occurrences,
    );
    Ok(LocalResponse::FindText {
        selection: occurrences,
    })
}

async fn handle_find_grammar(
    grammars_directory: &Path,
    queries_directory: &Path,
) -> Result<LocalResponse> {
    use crate::app::grammars::*;
    let release = find_grammar_release()
        .await
        .map_err(|x| anyhow!("find_grammar_release fail: {x}"))?;
    let mut updated = false;
    updated |= fetch_grammars(&release, grammars_directory)
        .await
        .map_err(|x| anyhow!("fetch_grammars fail: {x}"))?;
    updated |= fetch_queries(&release, queries_directory)
        .await
        .map_err(|x| anyhow!("fetch_queries fail: {x}"))?;
    Ok(LocalResponse::FindGrammar { updated })
}
#[allow(clippy::too_many_arguments)]
async fn handle_download_readme(
    volt: &VoltInfo,
    directory: &Directory,
    font_family: &[FamilyOwned],
    editor_fg: Color,
    style_colors: &HashMap<String, Color>,
    font_size: f32,
    markdown_blockquote: Color,
    editor_link: Color,
) -> Result<LocalResponse> {
    let url = format!(
        "https://plugins.lapce.dev/api/v1/plugins/{}/{}/{}/readme",
        volt.author, volt.name, volt.version
    );
    let resp = lapce_proxy::async_get_url(&url, None).await?;
    if resp.status() != 200 {
        let text = parse_markdown(
            "Plugin doesn't have a README",
            2.0,
            directory,
            font_family,
            editor_fg,
            style_colors,
            font_size,
            markdown_blockquote,
            editor_link,
        );
        return Ok(LocalResponse::DownloadVoltReadme { readme: text });
    }
    let text = resp.text().await?;
    let text = parse_markdown(
        &text,
        2.0,
        directory,
        font_family,
        editor_fg,
        style_colors,
        font_size,
        markdown_blockquote,
        editor_link,
    );
    Ok(LocalResponse::DownloadVoltReadme { readme: text })
}

async fn handle_uninstall_volt(dir: &Path) -> Result<LocalResponse> {
    tokio::fs::remove_dir_all(dir).await?;
    Ok(LocalResponse::UninstallVolt)
}
async fn handle_query_volts(query: &str, offset: usize) -> Result<LocalResponse> {
    let url = format!(
        "https://plugins.lapce.dev/api/v1/plugins?q={query}&offset={offset}"
    );
    let volts: VoltsInfo =
        lapce_proxy::async_get_url(url, None).await?.json().await?;
    Ok(LocalResponse::QueryVolts { volts })
}

async fn handle_load_icon(
    volt: &VoltInfo,
    cache_directory: Option<PathBuf>,
) -> Result<LocalResponse> {
    let url = format!(
        "https://plugins.lapce.dev/api/v1/plugins/{}/{}/{}/icon?id={}",
        volt.author, volt.name, volt.version, volt.updated_at_ts
    );
    log::info!("{url}");
    let cache_file_path = cache_directory.map(|cache_dir| {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let filename = format!("{:x}", hasher.finalize());
        cache_dir.join(filename)
    });

    if let Some(cache_file) = &cache_file_path
        && cache_file.exists()
    {
        let icon = VoltIcon::from_bytes(&tokio::fs::read(cache_file).await?)?;
        return Ok(LocalResponse::LoadIcon { icon });
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
    Ok(LocalResponse::LoadIcon { icon })
}

async fn handle_spans_builder(
    len: usize,
    styles: SemanticStyles,
    result_id: Option<String>,
) -> Result<LocalResponse> {
    let mut styles_span = SpansBuilder::new(len);
    for style in styles.styles {
        if let Some(fg) = style.style.fg_color {
            styles_span.add_span(Interval::new(style.start, style.end), fg);
        }
    }
    let styles = styles_span.build();
    Ok(LocalResponse::SpansBuilder { styles, result_id })
}
async fn handle_find_all_volts(
    extra_plugin_paths: Arc<Vec<PathBuf>>,
    plugin_dir: PathBuf,
) -> Result<LocalResponse> {
    let volts = find_all_volts(&extra_plugin_paths, &plugin_dir).await;
    Ok(LocalResponse::FindAllVolts { volts })
}

async fn handle_query_volt_info(url: String) -> Result<LocalResponse> {
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

/// todo 内部是否可以异步
async fn handle_notification_db_save_event(db: Arc<LapceDb>, event: SaveEvent) {
    match event {
        SaveEvent::App(info) => {
            if let Err(err) = db.insert_app_info(info) {
                log::error!("{:?}", err);
            }
        },
        SaveEvent::Workspace(workspace, info) => {
            if let Err(err) = db.insert_workspace(&workspace, &info) {
                log::error!("{:?}", err);
            }
        },
        SaveEvent::RecentWorkspace(workspace) => {
            if let Err(err) = db.insert_recent_workspace(workspace) {
                log::error!("{:?}", err);
            }
        },
        SaveEvent::Doc(info) => {
            if let Err(err) = db.insert_doc(&info) {
                log::error!("{:?}", err);
            }
        },
        SaveEvent::DisabledVolts(volts) => {
            if let Err(err) = db.insert_disabled_volts(volts) {
                log::error!("{:?}", err);
            }
        },
        SaveEvent::WorkspaceDisabledVolts(workspace, volts) => {
            if let Err(err) = db.insert_workspace_disabled_volts(workspace, volts) {
                log::error!("{:?}", err);
            }
        },
        SaveEvent::PanelOrder(order) => {
            if let Err(err) = db.insert_panel_orders(&order) {
                log::error!("{:?}", err);
            }
        },
    }
}

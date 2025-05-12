#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    process,
    sync::{Arc, RwLock},
    thread,
};

use anyhow::{Result, anyhow};
use jsonrpc_lite::{Id, Params};
use lapce_rpc::{
    RpcError,
    plugin::{PluginId, VoltID, VoltInfo, VoltMetadata},
    style::LineStyle,
};
use lapce_xi_rope::{Rope, RopeDelta};
use log::debug;
use lsp_types::{
    DocumentFilter, InitializeParams, InitializedParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, Url,
    VersionedTextDocumentIdentifier, WorkDoneProgressParams, WorkspaceFolder,
    notification::Initialized, request::Initialize,
};
use parking_lot::Mutex;
use psp_types::{Notification, Request};
use serde_json::Value;
use tokio::io::AsyncReadExt;
use wasi_experimental_http_wasmtime::{HttpCtx, HttpState};
use wasmtime_wasi::WasiCtxBuilder;

use super::{
    PluginCatalogRpcHandler, client_capabilities,
    psp::{
        HandlerType, PluginHandlerNotification, PluginHostHandler,
        PluginServerHandler, PluginServerRpc, ResponseSender, RpcCallback,
        handle_plugin_server_message,
    },
    volt_icon,
};
use crate::plugin::psp::PluginServerRpcHandler;

#[derive(Default)]
pub struct WasiPipe {
    buffer: VecDeque<u8>,
}

impl WasiPipe {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Read for WasiPipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let amt = std::cmp::min(buf.len(), self.buffer.len());
        for (i, byte) in self.buffer.drain(..amt).enumerate() {
            buf[i] = byte;
        }
        Ok(amt)
    }
}

impl Write for WasiPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for WasiPipe {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::other("can not seek in a pipe"))
    }
}

pub struct Plugin {
    #[allow(dead_code)]
    id:             PluginId,
    host:           PluginHostHandler,
    configurations: Option<HashMap<String, serde_json::Value>>,
}

impl PluginServerHandler for Plugin {
    fn method_registered(&mut self, method: &str) -> bool {
        self.host.method_registered(method)
    }

    fn document_supported(
        &mut self,
        language_id: Option<&str>,
        path: Option<&Path>,
    ) -> bool {
        self.host.document_supported(language_id, path)
    }

    fn handle_handler_notification(
        &mut self,
        notification: PluginHandlerNotification,
    ) -> Result<()> {
        use PluginHandlerNotification::*;
        match notification {
            Initialize(id) => {
                self.initialize(id)?;
            },
            InitializeResult(result) => {
                self.host.server_capabilities = result.capabilities;
            },
            Shutdown => {
                self.shutdown();
            },
            SpawnedPluginLoaded { plugin_id } => {
                self.host.handle_spawned_plugin_loaded(plugin_id);
            },
        }
        Ok(())
    }

    fn handle_host_notification(
        &mut self,
        method: String,
        params: Params,
        from: String,
    ) -> Result<()> {
        self.host.handle_notification(method, params, from)
    }

    fn handle_to_host_request(
        &mut self,
        id: Id,
        method: String,
        params: Params,
        resp: ResponseSender,
    ) {
        self.host.handle_request(id, method, params, resp);
    }

    fn handle_did_save_text_document(
        &self,
        language_id: String,
        path: PathBuf,
        text_document: TextDocumentIdentifier,
        text: Rope,
    ) -> Result<()> {
        self.host.handle_did_save_text_document(
            language_id,
            path,
            text_document,
            text,
        )
    }

    fn handle_did_change_text_document(
        &mut self,
        language_id: String,
        document: VersionedTextDocumentIdentifier,
        delta: RopeDelta,
        text: Rope,
        new_text: Rope,
        change: Arc<
            Mutex<(
                Option<TextDocumentContentChangeEvent>,
                Option<TextDocumentContentChangeEvent>,
            )>,
        >,
    ) -> Result<()> {
        self.host.handle_did_change_text_document(
            language_id,
            document,
            delta,
            text,
            new_text,
            change,
        )
    }

    fn format_semantic_tokens(
        &self,
        id: u64,
        tokens: lsp_types::SemanticTokens,
        text: Rope,
        f: Box<dyn RpcCallback<(Vec<LineStyle>, Option<String>), RpcError>>,
    ) {
        self.host.format_semantic_tokens(id, tokens, text, f);
    }
}

impl Plugin {
    fn initialize(&mut self, id: u64) -> Result<()> {
        let workspace = self.host.workspace.clone();
        let configurations = self.configurations.as_ref().map(unflatten_map);
        let root_uri = workspace.map(|p| Url::from_directory_path(p).unwrap());
        let server_rpc = self.host.server_rpc.clone();
        self.host.server_rpc.server_request_async(
            Initialize::METHOD,
            #[allow(deprecated)]
            InitializeParams {
                process_id:                Some(process::id()),
                root_path:                 None,
                root_uri:                  root_uri.clone(),
                capabilities:              client_capabilities(),
                trace:                     None,
                client_info:               None,
                locale:                    None,
                initialization_options:    configurations,
                workspace_folders:         root_uri.map(|uri| {
                    vec![WorkspaceFolder {
                        name: uri.as_str().to_string(),
                        uri,
                    }]
                }),
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
            None,
            None,
            false,
            id,
            move |_id, value| {
                if let Err(err) = handle_initialize_response(server_rpc, value) {
                    log::error!("{:?}", err);
                }
            },
        )
    }

    fn shutdown(&self) {}
}

pub fn handle_initialize_response(
    server_rpc: PluginServerRpcHandler,
    response: Result<Value, RpcError>,
) -> Result<()> {
    let response = response.map_err(|err| anyhow!("response error: {err:?}"))?;
    let result = serde_json::from_value(response)?;
    server_rpc.handle_rpc(PluginServerRpc::Handler(
        PluginHandlerNotification::InitializeResult(result),
    ))?;
    server_rpc.server_notification(
        Initialized::METHOD,
        InitializedParams {},
        None,
        None,
        false,
    )?;
    Ok(())
}

#[tokio::main]
pub async fn load_all_volts(
    plugin_rpc: PluginCatalogRpcHandler,
    extra_plugin_paths: &[PathBuf],
    disabled_volts: Vec<VoltID>,
    id: u64,
    plugin_dir: PathBuf,
) {
    let all_volts = find_all_volts(extra_plugin_paths, &plugin_dir).await;
    let mut volts = Vec::with_capacity(all_volts.len());
    for meta in all_volts {
        let Some(_) = meta.wasm.as_ref() else {
            continue;
        };
        let icon = volt_icon(&meta);
        plugin_rpc.core_rpc.volt_installed(meta.clone(), icon);
        if disabled_volts.contains(&meta.id()) {
            continue;
        }
        volts.push(meta);
    }
    if let Err(err) = plugin_rpc.unactivated_volts(volts, id) {
        log::error!("{:?}", err);
    }
}

/// Find all installed volts.
/// `plugin_dev_path` allows launching Lapce with a plugin on your local system
/// for testing purposes.
/// As well, this function skips any volt in the typical plugin directory that
/// match the name of the dev plugin so as to support developing a plugin you
/// actively use.
/// todo change to async
pub fn sync_find_all_volts(
    extra_plugin_paths: &[PathBuf],
    plugin_dir: &Path,
) -> Vec<VoltMetadata> {
    // let Some(plugin_dir) = Directory::plugins_directory() else {
    //     return Vec::new();
    // };

    let mut plugins: Vec<VoltMetadata> = plugin_dir
        .read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let entry = result.ok()?;
            let metadata = entry.metadata().ok()?;

            // Ignore any loose files or '.' prefixed hidden directories
            if metadata.is_file() || entry.file_name().to_str()?.starts_with('.') {
                return None;
            }

            Some(entry.path())
        })
        .filter_map(|path| match sync_load_volt(&path) {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                log::error!("Failed to load plugin: {:?}", e);
                None
            },
        })
        .collect();

    for plugin_path in extra_plugin_paths {
        let mut metadata = match sync_load_volt(plugin_path) {
            Ok(metadata) => metadata,
            Err(e) => {
                log::error!("Failed to load extra plugin: {:?}", e);
                continue;
            },
        };

        let pos = plugins.iter().position(|meta| {
            meta.name == metadata.name && meta.author == metadata.author
        });

        if let Some(pos) = pos {
            std::mem::swap(&mut plugins[pos], &mut metadata);
        } else {
            plugins.push(metadata);
        }
    }

    plugins
}

/// Find all installed volts.  
/// `plugin_dev_path` allows launching Lapce with a plugin on your local system
/// for testing purposes.  
/// As well, this function skips any volt in the typical plugin directory that
/// match the name of the dev plugin so as to support developing a plugin you
/// actively use.
/// todo change to async
pub async fn find_all_volts(
    extra_plugin_paths: &[PathBuf],
    plugin_dir: &Path,
) -> Vec<VoltMetadata> {
    // let Some(plugin_dir) = Directory::plugins_directory().await else {
    //     return Vec::new();
    // };
    let mut plugins: Vec<VoltMetadata> = vec![];
    match tokio::fs::read_dir(&plugin_dir).await {
        Ok(mut dir) => {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let Some(path) = entry.metadata().await.ok().and_then(|meta| {
                    if meta.is_file() || entry.file_name().to_str()?.starts_with('.')
                    {
                        return None;
                    }
                    Some(entry.path())
                }) else {
                    continue;
                };
                match load_volt(&path).await {
                    Ok(metadata) => plugins.push(metadata),
                    Err(e) => {
                        log::error!("Failed to load plugin: {:?}", e);
                    },
                }
            }
        },
        Err(err) => {
            log::warn!("{err:?}");
        },
    }

    for plugin_path in extra_plugin_paths {
        let mut metadata = match load_volt(plugin_path).await {
            Ok(metadata) => metadata,
            Err(e) => {
                log::error!("Failed to load extra plugin: {:?}", e);
                continue;
            },
        };

        let pos = plugins.iter().position(|meta| {
            meta.name == metadata.name && meta.author == metadata.author
        });

        if let Some(pos) = pos {
            std::mem::swap(&mut plugins[pos], &mut metadata);
        } else {
            plugins.push(metadata);
        }
    }

    plugins
}

pub fn sync_load_volt(path: &Path) -> Result<VoltMetadata> {
    let path = path.canonicalize()?;
    let mut file = fs::File::open(path.join("volt.toml"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let mut meta: VoltMetadata = toml::from_str(&contents)?;

    meta.dir = Some(path.clone());
    meta.wasm = meta.wasm.as_ref().and_then(|wasm| {
        Some(path.join(wasm).canonicalize().ok()?.to_str()?.to_string())
    });
    // FIXME: This does `meta.color_themes = Some([])` in case, for example,
    // it cannot find matching files, but in that case it should do
    // `meta.color_themes = None`
    meta.color_themes = meta.color_themes.as_ref().map(|themes| {
        themes
            .iter()
            .filter_map(|theme| {
                Some(path.join(theme).canonicalize().ok()?.to_str()?.to_string())
            })
            .collect()
    });
    // FIXME: This does `meta.icon_themes = Some([])` in case, for example,
    // it cannot find matching files, but in that case it should do
    // `meta.icon_themes = None`
    meta.icon_themes = meta.icon_themes.as_ref().map(|themes| {
        themes
            .iter()
            .filter_map(|theme| {
                Some(path.join(theme).canonicalize().ok()?.to_str()?.to_string())
            })
            .collect()
    });

    Ok(meta)
}
pub async fn load_volt(path: &Path) -> Result<VoltMetadata> {
    let path = path.canonicalize()?;
    let mut file = tokio::fs::File::open(path.join("volt.toml")).await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    let mut meta: VoltMetadata = toml::from_str(&contents)?;

    meta.dir = Some(path.clone());
    meta.wasm = meta.wasm.as_ref().and_then(|wasm| {
        Some(path.join(wasm).canonicalize().ok()?.to_str()?.to_string())
    });
    // FIXME: This does `meta.color_themes = Some([])` in case, for example,
    // it cannot find matching files, but in that case it should do
    // `meta.color_themes = None`
    meta.color_themes = meta.color_themes.as_ref().map(|themes| {
        themes
            .iter()
            .filter_map(|theme| {
                Some(path.join(theme).canonicalize().ok()?.to_str()?.to_string())
            })
            .collect()
    });
    // FIXME: This does `meta.icon_themes = Some([])` in case, for example,
    // it cannot find matching files, but in that case it should do
    // `meta.icon_themes = None`
    meta.icon_themes = meta.icon_themes.as_ref().map(|themes| {
        themes
            .iter()
            .filter_map(|theme| {
                Some(path.join(theme).canonicalize().ok()?.to_str()?.to_string())
            })
            .collect()
    });

    Ok(meta)
}

pub async fn enable_volt(
    plugin_rpc: PluginCatalogRpcHandler,
    volt: VoltInfo,
    id: u64,
    plugins_directory: PathBuf,
) -> Result<()> {
    let path = plugins_directory.join(volt.id().to_string());
    let meta = load_volt(&path).await?;
    plugin_rpc.unactivated_volts(vec![meta], id)?;
    Ok(())
}

pub fn start_volt(
    workspace: Option<PathBuf>,
    configurations: Option<HashMap<String, serde_json::Value>>,
    plugin_rpc: PluginCatalogRpcHandler,
    meta: VoltMetadata,
    id: u64,
) -> Result<()> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_file(
        &engine,
        meta.wasm
            .as_ref()
            .ok_or_else(|| anyhow!("no wasm in plugin"))?,
    )?;
    let mut linker = wasmtime::Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
    HttpState::new()?.add_to_linker(&mut linker, |_| HttpCtx {
        allowed_hosts:           Some(vec!["insecure:allow-all".to_string()]),
        max_concurrent_requests: Some(100),
    })?;

    let volt_path = meta
        .dir
        .as_ref()
        .ok_or_else(|| anyhow!("plugin meta doesn't have dir"))?;

    #[cfg(target_os = "linux")]
    let volt_libc = {
        match std::process::Command::new("ldd").arg("--version").output() {
            Ok(cmd) => {
                if String::from_utf8_lossy(&cmd.stdout)
                    .to_lowercase()
                    .split_terminator('\n')
                    .next()
                    .unwrap_or("")
                    .contains("musl")
                {
                    "musl"
                } else {
                    "glibc"
                }
            },
            _ => "glibc",
        }
    };

    #[cfg(not(target_os = "linux"))]
    let volt_libc = "";

    let stdin = Arc::new(RwLock::new(WasiPipe::new()));
    let stdout = Arc::new(RwLock::new(WasiPipe::new()));
    let stderr = Arc::new(RwLock::new(WasiPipe::new()));
    let wasi = WasiCtxBuilder::new()
        .inherit_env()?
        .env("VOLT_OS", std::env::consts::OS)?
        .env("VOLT_ARCH", std::env::consts::ARCH)?
        .env("VOLT_LIBC", volt_libc)?
        .env(
            "VOLT_URI",
            Url::from_directory_path(volt_path)
                .map_err(|_| anyhow!("can't convert folder path to uri"))?
                .as_ref(),
        )?
        .stdin(Box::new(wasi_common::pipe::ReadPipe::from_shared(
            stdin.clone(),
        )))
        .stdout(Box::new(wasi_common::pipe::WritePipe::from_shared(
            stdout.clone(),
        )))
        .stderr(Box::new(wasi_common::pipe::WritePipe::from_shared(
            stderr.clone(),
        )))
        .preopened_dir(
            wasmtime_wasi::Dir::open_ambient_dir(
                volt_path,
                wasmtime_wasi::ambient_authority(),
            )?,
            "/",
        )?
        .build();
    let mut store = wasmtime::Store::new(&engine, wasi);

    let (io_tx, io_rx) = crossbeam_channel::unbounded();
    let rpc = PluginServerRpcHandler::new(
        meta.id(),
        None,
        None,
        io_tx,
        id,
        HandlerType::Plugin,
    )?;

    let local_rpc = rpc.clone();
    let local_stdin = stdin.clone();
    let volt_name = format!("volt {}", meta.name);
    linker.func_wrap("lapce", "host_handle_rpc", move || {
        if let Ok(msg) = wasi_read_string(&stdout) {
            debug!("read from wasi: {msg}");
            if let Some(resp) =
                handle_plugin_server_message(&local_rpc, &msg, &volt_name)
            {
                if let Ok(msg) = serde_json::to_string(&resp) {
                    if let Err(err) = writeln!(local_stdin.write().unwrap(), "{msg}")
                    {
                        log::error!("{:?}", err);
                    }
                }
            }
        }
    })?;
    let plugin_meta = meta.clone();
    linker.func_wrap("lapce", "host_handle_stderr", move || {
        if let Ok(msg) = wasi_read_string(&stderr) {
            log::error!(
                "lapce_proxy::plugin::wasi::{}::{} {msg}",
                plugin_meta.author,
                plugin_meta.name
            );
        }
    })?;
    linker.module(&mut store, "", &module)?;
    let local_rpc = rpc.clone();
    thread::spawn(move || {
        let mut exist_id = None;
        {
            let instance = linker.instantiate(&mut store, &module).unwrap();
            let handle_rpc = instance
                .get_func(&mut store, "handle_rpc")
                .ok_or_else(|| anyhow!("can't convet to function"))
                .unwrap()
                .typed::<(), ()>(&mut store)
                .unwrap();
            for msg in io_rx {
                if msg
                    .get_method()
                    .map(|x| x == lsp_types::request::Shutdown::METHOD)
                    .unwrap_or_default()
                {
                    exist_id = msg.get_id();
                    break;
                }
                debug!("write to wasi: {msg:?}");
                if let Ok(msg) = serde_json::to_string(&msg) {
                    if let Err(err) = writeln!(stdin.write().unwrap(), "{msg}") {
                        log::error!("{:?}", err);
                    }
                }
                if let Err(err) = handle_rpc.call(&mut store, ()) {
                    log::error!("{:?}", err);
                }
            }
        }
        if let Some(id) = exist_id {
            local_rpc.handle_server_response(id, Ok(Value::Null));
        }
    });

    let id = PluginId::next();
    let mut plugin = Plugin {
        id,
        host: PluginHostHandler::new(
            workspace,
            meta.dir.clone(),
            meta.id(),
            meta.display_name.clone(),
            meta.activation
                .iter()
                .flat_map(|m| m.language.iter().flatten())
                .cloned()
                .map(|s| DocumentFilter {
                    language: Some(s),
                    pattern:  None,
                    scheme:   None,
                })
                .chain(
                    meta.activation
                        .iter()
                        .flat_map(|m| m.workspace_contains.iter().flatten())
                        .cloned()
                        .map(|s| DocumentFilter {
                            language: None,
                            pattern:  Some(s),
                            scheme:   None,
                        }),
                )
                .collect(),
            plugin_rpc.core_rpc.clone(),
            rpc.clone(),
            plugin_rpc.clone(),
        ),
        configurations,
    };
    let local_rpc = rpc.clone();
    thread::spawn(move || {
        let handler_name = format!("plugin {}", plugin.host.volt_display_name);
        local_rpc.mainloop(&mut plugin, &handler_name);
    });

    if plugin_rpc.plugin_server_loaded(rpc.clone()).is_err() {
        rpc.shutdown()?;
    }
    Ok(())
}

fn wasi_read_string(stdout: &Arc<RwLock<WasiPipe>>) -> Result<String> {
    let mut buf = String::new();
    stdout.write().unwrap().read_to_string(&mut buf)?;
    Ok(buf)
}

fn unflatten_map(map: &HashMap<String, serde_json::Value>) -> serde_json::Value {
    let mut new = serde_json::json!({});
    for (key, value) in map.iter() {
        let mut current = new.as_object_mut().unwrap();
        let parts: Vec<&str> = key.split('.').collect();
        let total_parts = parts.len();
        for (i, part) in parts.into_iter().enumerate() {
            if i + 1 < total_parts {
                if !current.get(part).map(|v| v.is_object()).unwrap_or(false) {
                    current.insert(part.to_string(), serde_json::json!({}));
                }
                current = current.get_mut(part).unwrap().as_object_mut().unwrap();
            } else {
                current.insert(part.to_string(), value.clone());
            }
        }
    }
    new
}

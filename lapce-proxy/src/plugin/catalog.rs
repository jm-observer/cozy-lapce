use std::{
    borrow::Cow,
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use anyhow::Result;
use jsonrpc_lite::Id;
use lapce_core::directory::Directory;
use lapce_rpc::{
    RpcError,
    dap_types::{
        self, DapId, DapServer, RunDebugConfig, SetBreakpointsResponse,
        SourceBreakpoint,
    },
    plugin::{PluginId, VoltID, VoltInfo, VoltMetadata},
    proxy::ProxyResponse,
    style::LineStyle,
};
use lapce_xi_rope::{Rope, RopeDelta};
use log::{debug, error};
use lsp_types::{
    DidOpenTextDocumentParams, MessageType, SemanticTokens, ShowMessageParams,
    TextDocumentIdentifier, TextDocumentItem, VersionedTextDocumentIdentifier,
    notification::DidOpenTextDocument, request::Request,
};
use parking_lot::Mutex;
use psp_types::Notification;
use serde_json::Value;

use super::{
    DapNotificationOfUser, PluginCatalogNotification, PluginCatalogRpcHandler,
    dap::{DapClient, DapRpcHandler, DebuggerData},
    install_volt,
    psp::{ClonableCallback, PluginServerRpc, PluginServerRpcHandler, RpcCallback},
    wasi::start_volt,
};
use crate::plugin::{
    psp::PluginHandlerNotification,
    wasi::{enable_volt, load_all_volts},
};

pub struct PluginCatalog {
    workspace:             Option<PathBuf>,
    plugin_rpc:            PluginCatalogRpcHandler,
    plugins:               HashMap<PluginId, PluginServerRpcHandler>,
    daps:                  HashMap<DapId, DapRpcHandler>,
    debuggers:             HashMap<String, DebuggerData>,
    plugin_configurations: HashMap<String, HashMap<String, serde_json::Value>>,
    unactivated_volts:     HashMap<VoltID, VoltMetadata>,
    open_files:            HashMap<PathBuf, String>,
    directory:             Directory,
}

impl PluginCatalog {
    pub fn new(
        _id: u64,
        workspace: Option<PathBuf>,
        _disabled_volts: Vec<VoltID>,
        _extra_plugin_paths: Vec<PathBuf>,
        plugin_configurations: HashMap<String, HashMap<String, serde_json::Value>>,
        plugin_rpc: PluginCatalogRpcHandler,
        directory: Directory,
    ) -> Self {
        let _plugin_dir = directory.plugins_directory.clone();
        let plugin = Self {
            workspace,
            plugin_rpc: plugin_rpc.clone(),
            plugin_configurations,
            plugins: HashMap::new(),
            daps: HashMap::new(),
            debuggers: HashMap::new(),
            unactivated_volts: HashMap::new(),
            open_files: HashMap::new(),
            directory,
        };

        // todo remove
        thread::spawn(move || {
            load_all_volts(
                plugin_rpc,
                &_extra_plugin_paths,
                _disabled_volts,
                _id,
                _plugin_dir,
            );
        });

        plugin
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_server_request(
        &mut self,
        plugin_id: Option<PluginId>,
        request_sent: Option<Arc<AtomicUsize>>,
        method: Cow<'static, str>,
        params: Value,
        language_id: Option<String>,
        path: Option<PathBuf>,
        check: bool,
        id: u64,
        f: Box<dyn ClonableCallback<Value, RpcError>>,
    ) -> Result<()> {
        if let Some(plugin_id) = plugin_id {
            if let Some(plugin) = self.plugins.get(&plugin_id) {
                plugin.server_request_async(
                    method,
                    params,
                    language_id,
                    path,
                    check,
                    id,
                    move |id, result| {
                        f(id, plugin_id, result);
                    },
                )?;
            } else {
                f(
                    Id::Num(id as i64),
                    plugin_id,
                    Err(RpcError {
                        code:    0,
                        message: "plugin doesn't exist".to_string(),
                    }),
                );
            }
            return Ok(());
        }
        let mut count = 0;
        for (plugin_id, plugin) in self.plugins.iter() {
            if plugin.handler_type.is_plugin() {
                continue;
            }
            count += 1;
            let f = dyn_clone::clone_box(&*f);
            let plugin_id = *plugin_id;
            plugin.server_request_async(
                method.clone(),
                params.clone(),
                language_id.clone(),
                path.clone(),
                check,
                id,
                move |id, result| {
                    // error!("handle_server_request response {id:?} {plugin_id:?}");
                    f(id, plugin_id, result);
                },
            )?;
        }

        if let Some(request_sent) = request_sent {
            // if there are no plugins installed the callback of the client is not
            // called so check if plugins list is empty
            if self.plugins.is_empty() {
                // Add a request
                request_sent.fetch_add(1, Ordering::Relaxed);

                // make a direct callback with an "error"
                f(
                    Id::Num(id as i64),
                    lapce_rpc::plugin::PluginId(0),
                    Err(RpcError {
                        code:    0,
                        message: "no available plugin could make a callback, \
                                  because the plugins list is empty"
                            .to_string(),
                    }),
                );
            } else {
                request_sent.fetch_add(count, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_server_notification(
        &mut self,
        plugin_id: Option<PluginId>,
        method: impl Into<Cow<'static, str>>,
        params: Value,
        language_id: Option<String>,
        path: Option<PathBuf>,
        check: bool,
    ) -> Result<()> {
        if let Some(plugin_id) = plugin_id {
            if let Some(plugin) = self.plugins.get(&plugin_id) {
                plugin.server_notification(
                    method,
                    params,
                    language_id,
                    path,
                    check,
                )?;
            }

            return Ok(());
        }

        // Otherwise send it to all plugins
        let method = method.into();
        for (_, plugin) in self.plugins.iter() {
            plugin.server_notification(
                method.clone(),
                params.clone(),
                language_id.clone(),
                path.clone(),
                check,
            )?;
        }
        Ok(())
    }

    pub fn shutdown_volt(
        &mut self,
        volt: VoltInfo,
        f: Box<dyn ClonableCallback<Value, RpcError>>,
        request_id: u64,
    ) -> Result<()> {
        let id = volt.id();
        for (plugin_id, plugin) in self.plugins.iter() {
            if plugin.volt_id == id {
                let f = dyn_clone::clone_box(&*f);
                let plugin_id = *plugin_id;
                plugin.server_request_async(
                    lsp_types::request::Shutdown::METHOD,
                    Value::Null,
                    None,
                    None,
                    false,
                    request_id,
                    move |id, result| {
                        f(id, plugin_id, result);
                    },
                )?;
                if let Err(err) = plugin.shutdown() {
                    error!("shutdown fail {plugin_id:?} {err}");
                }
            }
        }
        Ok(())
    }

    fn start_unactivated_volts(
        &mut self,
        to_be_activated: Vec<VoltID>,
        request_id: u64,
    ) {
        for id in to_be_activated.iter() {
            let workspace = self.workspace.clone();
            if let Some(meta) = self.unactivated_volts.remove(id) {
                let configurations =
                    self.plugin_configurations.get(&meta.name).cloned();
                log::debug!("{:?} {:?}", id, configurations);
                let plugin_rpc = self.plugin_rpc.clone();
                thread::spawn(move || {
                    if let Err(err) = start_volt(
                        workspace,
                        configurations,
                        plugin_rpc,
                        meta,
                        request_id,
                    ) {
                        log::error!("{:?}", err);
                    }
                });
            }
        }
    }

    fn check_unactivated_volts(&mut self, id: u64) {
        let to_be_activated: Vec<VoltID> = self
            .unactivated_volts
            .iter()
            .filter_map(|(id, meta)| {
                let contains = meta
                    .activation
                    .as_ref()
                    .and_then(|a| a.language.as_ref())
                    .map(|l| {
                        self.open_files
                            .iter()
                            .any(|(_, language_id)| l.contains(language_id))
                    })
                    .unwrap_or(false);
                if contains {
                    return Some(id.clone());
                }

                if let Some(workspace) = self.workspace.as_ref() {
                    if let Some(globs) = meta
                        .activation
                        .as_ref()
                        .and_then(|a| a.workspace_contains.as_ref())
                    {
                        let mut builder = globset::GlobSetBuilder::new();
                        for glob in globs {
                            match globset::Glob::new(glob) {
                                Ok(glob) => {
                                    builder.add(glob);
                                },
                                Err(err) => {
                                    log::error!("{:?}", err);
                                },
                            }
                        }
                        match builder.build() {
                            Ok(matcher) => {
                                if !matcher.is_empty() {
                                    for entry in walkdir::WalkDir::new(workspace)
                                        .into_iter()
                                        .flatten()
                                    {
                                        if matcher.is_match(entry.path()) {
                                            return Some(id.clone());
                                        }
                                    }
                                }
                            },
                            Err(err) => {
                                log::error!("{:?}", err);
                            },
                        }
                    }
                }

                None
            })
            .collect();
        self.start_unactivated_volts(to_be_activated, id);
    }

    pub fn handle_did_open_text_document(
        &mut self,
        document: TextDocumentItem,
        id: u64,
    ) -> Result<()> {
        match document.uri.to_file_path() {
            Ok(path) => {
                self.open_files.insert(path, document.language_id.clone());
            },
            Err(err) => {
                log::error!("{:?}", err);
            },
        }

        let to_be_activated: Vec<VoltID> = self
            .unactivated_volts
            .iter()
            .filter_map(|(id, meta)| {
                let contains = meta
                    .activation
                    .as_ref()
                    .and_then(|a| a.language.as_ref())
                    .map(|l| l.contains(&document.language_id))?;
                if contains { Some(id.clone()) } else { None }
            })
            .collect();
        self.start_unactivated_volts(to_be_activated, id);

        let path = document.uri.to_file_path().ok();
        for (_, plugin) in self.plugins.iter() {
            plugin.server_notification(
                DidOpenTextDocument::METHOD,
                DidOpenTextDocumentParams {
                    text_document: document.clone(),
                },
                Some(document.language_id.clone()),
                path.clone(),
                true,
            )?;
        }

        Ok(())
    }

    pub fn handle_did_save_text_document(
        &mut self,
        language_id: String,
        path: PathBuf,
        text_document: TextDocumentIdentifier,
        text: Rope,
    ) -> Result<()> {
        for (_, plugin) in self.plugins.iter() {
            plugin.handle_rpc(PluginServerRpc::DidSaveTextDocument {
                language_id:   language_id.clone(),
                path:          path.clone(),
                text_document: text_document.clone(),
                text:          text.clone(),
            })?;
        }
        Ok(())
    }

    pub fn handle_did_change_text_document(
        &mut self,
        language_id: String,
        document: VersionedTextDocumentIdentifier,
        delta: RopeDelta,
        text: Rope,
        new_text: Rope,
    ) -> Result<()> {
        let change = Arc::new(Mutex::new((None, None)));
        for (_, plugin) in self.plugins.iter() {
            plugin.handle_rpc(PluginServerRpc::DidChangeTextDocument {
                language_id: language_id.clone(),
                document:    document.clone(),
                delta:       delta.clone(),
                text:        text.clone(),
                new_text:    new_text.clone(),
                change:      change.clone(),
            })?;
        }
        Ok(())
    }

    pub fn format_semantic_tokens(
        &self,
        id: u64,
        plugin_id: PluginId,
        tokens: SemanticTokens,
        text: Rope,
        f: Box<dyn RpcCallback<(Vec<LineStyle>, Option<String>), RpcError>>,
    ) -> Result<()> {
        if let Some(plugin) = self.plugins.get(&plugin_id) {
            plugin.handle_rpc(PluginServerRpc::FormatSemanticTokens {
                id,
                tokens,
                text,
                f,
            })?;
        } else {
            f.call(
                Id::Num(id as i64),
                Err(RpcError {
                    code:    0,
                    message: "plugin doesn't exist".to_string(),
                }),
            );
        }
        Ok(())
    }

    pub fn dap_variable(
        &self,
        dap_id: DapId,
        reference: usize,
        f: Box<dyn RpcCallback<Vec<dap_types::Variable>, RpcError>>,
    ) {
        if let Some(dap) = self.daps.get(&dap_id) {
            dap.variables_async(
                reference,
                |id, result: Result<dap_types::VariablesResponse, RpcError>| {
                    f.call(id, result.map(|resp| resp.variables))
                },
            );
        } else {
            f.call(
                Id::Num(0),
                Err(RpcError {
                    code:    0,
                    message: "plugin doesn't exist".to_string(),
                }),
            );
        }
    }

    pub fn dap_get_scopes(
        &self,
        dap_id: DapId,
        frame_id: usize,
        f: Box<
            dyn RpcCallback<
                    Vec<(dap_types::Scope, Vec<dap_types::Variable>)>,
                    RpcError,
                >,
        >,
    ) {
        if let Some(dap) = self.daps.get(&dap_id) {
            let local_dap = dap.clone();
            dap.scopes_async(
                frame_id,
                move |id, result: Result<dap_types::ScopesResponse, RpcError>| {
                    match result {
                        Ok(resp) => {
                            let scopes = resp.scopes.clone();
                            if let Some(scope) = resp.scopes.first() {
                                let scope = scope.to_owned();
                                thread::spawn(move || {
                                    local_dap.variables_async(
                                        scope.variables_reference,
                                        move |id,
                                              result: Result<
                                            dap_types::VariablesResponse,
                                            RpcError,
                                        >| {
                                            let resp: Vec<(
                                                dap_types::Scope,
                                                Vec<dap_types::Variable>,
                                            )> = scopes
                                                .iter()
                                                .enumerate()
                                                .map(|(index, s)| {
                                                    (
                                                        s.clone(),
                                                        if index == 0 {
                                                            result
                                                                .as_ref()
                                                                .map(|resp| {
                                                                    resp.variables
                                                                        .clone()
                                                                })
                                                                .unwrap_or_default()
                                                        } else {
                                                            Vec::new()
                                                        },
                                                    )
                                                })
                                                .collect();
                                            f.call(id, Ok(resp));
                                        },
                                    );
                                });
                            } else {
                                f.call(id, Ok(Vec::new()));
                            }
                        },
                        Err(e) => {
                            f.call(id, Err(e));
                        },
                    }
                },
            );
        } else {
            f.call(
                Id::Num(0),
                Err(RpcError {
                    code:    0,
                    message: "plugin doesn't exist".to_string(),
                }),
            );
        }
    }

    pub async fn handle_notification(
        &mut self,
        notification: PluginCatalogNotification,
    ) -> Result<()> {
        use PluginCatalogNotification::*;
        match notification {
            UnactivatedVolts(volts, id) => {
                log::debug!("UnactivatedVolts {:?}", volts);
                for volt in volts {
                    let id = volt.id();
                    self.unactivated_volts.insert(id, volt);
                }
                self.check_unactivated_volts(id);
            },
            UpdatePluginConfigs(configs) => {
                log::debug!("UpdatePluginConfigs {:?}", configs);
                self.plugin_configurations = configs;
            },
            PluginServerLoaded(plugin) => {
                // TODO: check if the server has did open registered
                match self.plugin_rpc.proxy_rpc.get_open_files_content() {
                    Ok(ProxyResponse::GetOpenFilesContentResponse { items }) => {
                        for item in items {
                            let language_id = Some(item.language_id.clone());
                            let path = item.uri.to_file_path().ok();
                            plugin.server_notification(
                                DidOpenTextDocument::METHOD,
                                DidOpenTextDocumentParams {
                                    text_document: item,
                                },
                                language_id,
                                path,
                                true,
                            )?;
                        }
                    },
                    Ok(_) => {},
                    Err(err) => {
                        log::error!("{:?}", err);
                    },
                }

                let plugin_id = plugin.plugin_id;
                let spawned_by = plugin.spawned_by;

                self.plugins.insert(plugin.plugin_id, plugin);

                if let Some(spawned_by) = spawned_by {
                    if let Some(plugin) = self.plugins.get(&spawned_by) {
                        plugin.handle_rpc(PluginServerRpc::Handler(
                            PluginHandlerNotification::SpawnedPluginLoaded {
                                plugin_id,
                            },
                        ))?;
                    }
                }
            },
            InstallVolt(volt, id) => {
                let workspace = self.workspace.clone();
                let configurations =
                    self.plugin_configurations.get(&volt.name).cloned();
                let catalog_rpc = self.plugin_rpc.clone();
                let plugin_dir = self.directory.plugins_directory.clone();
                catalog_rpc.stop_volt(id, volt.clone());
                tokio::spawn(async move {
                    if let Err(err) = install_volt(
                        catalog_rpc,
                        workspace,
                        configurations,
                        volt,
                        id,
                        plugin_dir,
                    )
                    .await
                    {
                        log::error!("{:?}", err);
                    }
                });
            },
            ReloadVolt(volt, id) => {
                log::debug!("ReloadVolt {:?}", volt);
                let volt_id = volt.id();
                let ids: Vec<PluginId> = self.plugins.keys().cloned().collect();
                for id in ids {
                    if self.plugins.get(&id).unwrap().volt_id == volt_id {
                        let plugin = self.plugins.remove(&id).unwrap();
                        if let Err(err) = plugin.shutdown() {
                            error!("shutdown fail {volt_id:?} {err}");
                        }
                    }
                }
                if let Err(err) = self.plugin_rpc.unactivated_volts(vec![volt], id) {
                    log::error!("{:?}", err);
                }
            },
            StopVolt(volt) => {
                log::debug!("StopVolt {:?}", volt);
                let volt_id = volt.id();
                let ids: Vec<PluginId> = self.plugins.keys().cloned().collect();
                for id in ids {
                    if self.plugins.get(&id).unwrap().volt_id == volt_id {
                        let plugin = self.plugins.remove(&id).unwrap();
                        if let Err(err) = plugin.shutdown() {
                            error!("shutdown fail {volt_id:?} {err}");
                        }
                    }
                }
            },
            EnableVolt(volt, id) => {
                log::debug!("EnableVolt {:?}", volt);
                let volt_id = volt.id();
                for (_, volt) in self.plugins.iter() {
                    if volt.volt_id == volt_id {
                        return Ok(());
                    }
                }
                let plugin_rpc = self.plugin_rpc.clone();
                let plugin_dir = self.directory.plugins_directory.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        enable_volt(plugin_rpc, volt, id, plugin_dir).await
                    {
                        log::error!("{:?}", err);
                    }
                });
            },
            DapNotificationOfUser(notification) => {
                self.handle_dap_notification_of_user(notification).await;
            },
            RegisterDebuggerType {
                debugger_type,
                program,
                args,
            } => {
                self.debuggers.insert(
                    debugger_type.clone(),
                    DebuggerData {
                        debugger_type,
                        program,
                        args,
                    },
                );
            },
            Shutdown => {
                for (id, plugin) in self.plugins.iter() {
                    if let Err(err) = plugin.shutdown() {
                        error!("shutdown fail {id:?} {err}");
                    }
                }
            },
            DapLoaded(dap_rpc) => {
                self.daps.insert(dap_rpc.dap_id, dap_rpc);
            },
        }

        Ok(())
    }

    async fn handle_dap_notification_of_user(
        &mut self,
        notification: DapNotificationOfUser,
    ) {
        use DapNotificationOfUser::*;
        log::debug!("handle_dap_notification_of_user {notification:?}");
        match notification {
            DapDisconnected(dap_id) => {
                self.daps.remove(&dap_id);
            },
            DapStart {
                config,
                breakpoints,
            } => {
                self.dap_start(config, breakpoints);
            },
            DapProcessId {
                dap_id,
                process_id,
                term_id,
            } => {
                if let Some(dap) = self.daps.get(&dap_id) {
                    if let Err(err) =
                        dap.termain_process_tx.send((term_id, process_id))
                    {
                        log::error!("{:?}", err);
                    }
                }
            },
            DapContinue { dap_id, thread_id } => {
                if let Some(dap) = self.daps.get(&dap_id).cloned() {
                    let plugin_rpc = self.plugin_rpc.clone();
                    dap.continue_thread(thread_id, plugin_rpc);
                }
            },
            DapPause { dap_id, thread_id } => {
                if let Some(dap) = self.daps.get(&dap_id).cloned() {
                    dap.pause_thread(thread_id)
                }
            },
            DapStepOver { dap_id, thread_id } => {
                if let Some(dap) = self.daps.get(&dap_id).cloned() {
                    dap.next(thread_id);
                }
            },
            DapStepInto { dap_id, thread_id } => {
                if let Some(dap) = self.daps.get(&dap_id).cloned() {
                    dap.step_in(thread_id);
                }
            },
            DapStepOut { dap_id, thread_id } => {
                if let Some(dap) = self.daps.get(&dap_id).cloned() {
                    dap.step_out(thread_id);
                }
            },
            DapStopByUser { dap_id } => {
                if let Some(dap) = self.daps.remove(&dap_id) {
                    debug!("DapStop {dap_id:?}");
                    dap.stop();
                }
            },
            DapDisconnect { dap_id } => {
                if let Some(dap) = self.daps.get(&dap_id).cloned() {
                    dap.disconnect();
                }
            },
            DapRestart {
                config,
                breakpoints,
            } => {
                if let Some(dap) = self.daps.remove(&config.dap_id) {
                    dap.stop();
                }
                self.dap_start(config, breakpoints);
            },
            DapSetBreakpoints {
                dap_id,
                path,
                breakpoints,
            } => {
                if let Some(dap) = self.daps.get(&dap_id) {
                    let core_rpc = self.plugin_rpc.core_rpc.clone();
                    dap.set_breakpoints_async(
                        path.clone(),
                        breakpoints,
                        move |_id, result: Result<SetBreakpointsResponse, RpcError>| {
                            match result {
                                Ok(resp) => {
                                    core_rpc.dap_breakpoints_resp(
                                        dap_id,
                                        path,
                                        resp.breakpoints.unwrap_or_default(),
                                    );
                                }
                                Err(err) => {
                                    log::error!("{:?}", err);
                                }
                            }
                        },
                    );
                }
            },
        }
    }

    fn dap_start(
        &mut self,
        config: RunDebugConfig,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
    ) {
        let workspace = self.workspace.clone();
        let plugin_rpc = self.plugin_rpc.clone();
        if let Some(debugger) = config
            .ty
            .as_ref()
            .and_then(|ty| self.debuggers.get(ty).cloned())
        {
            thread::spawn(move || {
                match DapClient::start(
                    DapServer {
                        program: debugger.program,
                        args:    debugger.args.unwrap_or_default(),
                        cwd:     workspace,
                    },
                    config.clone(),
                    breakpoints,
                    plugin_rpc.clone(),
                ) {
                    Ok(dap_rpc) => {
                        if let Err(err) = plugin_rpc.dap_loaded(dap_rpc.clone()) {
                            log::error!("plugin_rpc.dap_loaded {:?}", err);
                        }

                        if let Err(err) = dap_rpc.launch(&config) {
                            log::error!("dap_rpc.launch {:?}", err);
                        }
                    },
                    Err(err) => {
                        log::error!("DapClient::start {:?}", err);
                    },
                }
            });
        } else {
            log::error!("{config:?}");
            self.plugin_rpc.core_rpc.show_message(
                "debug fail".to_owned(),
                ShowMessageParams {
                    typ:     MessageType::ERROR,
                    message: "Debugger not found. Please install the appropriate \
                              plugin."
                        .to_owned(),
                },
            )
        }
    }
}

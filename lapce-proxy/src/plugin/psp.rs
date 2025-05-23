use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender};
use doc::lines::buffer::rope_text::{RopeText, RopeTextRef};
use dyn_clone::DynClone;
use jsonrpc_lite::{Id, JsonRpc, Params};
use lapce_core::{encoding::offset_utf16_to_utf8, rope_text_pos::RopeTextPosition};
use lapce_rpc::{
    RpcError,
    core::{CoreRpcHandler, ServerStatusParams},
    plugin::{PluginId, VoltID},
    style::{LineStyle, Style},
};
use lapce_xi_rope::{Rope, RopeDelta};
use log::{debug, error};
use lsp_types::{
    CancelParams, CodeActionProviderCapability, DidChangeTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSelector, FoldingRangeProviderCapability,
    HoverProviderCapability, ImplementationProviderCapability, InitializeResult,
    LogMessageParams, MessageType, OneOf, ProgressParams, PublishDiagnosticsParams,
    Range, Registration, RegistrationParams, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensServerCapabilities, ServerCapabilities, ShowMessageParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentSaveRegistrationOptions, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncSaveOptions,
    VersionedTextDocumentIdentifier,
    notification::{
        Cancel, DidChangeTextDocument, DidOpenTextDocument, DidSaveTextDocument,
        Initialized, LogMessage, Notification, Progress, PublishDiagnostics,
        ShowMessage,
    },
    request::{
        CallHierarchyIncomingCalls, CallHierarchyPrepare, CodeActionRequest,
        CodeActionResolveRequest, CodeLensRequest, CodeLensResolve, Completion,
        DocumentHighlightRequest, DocumentSymbolRequest, FoldingRangeRequest,
        Formatting, GotoDefinition, GotoImplementation, GotoTypeDefinition,
        HoverRequest, Initialize, InlayHintRequest, InlineCompletionRequest,
        OnTypeFormatting, PrepareRenameRequest, References, RegisterCapability,
        Rename, ResolveCompletionItem, SelectionRangeRequest,
        SemanticTokensFullDeltaRequest, SemanticTokensFullRequest,
        SignatureHelpRequest, WorkDoneProgressCreate, WorkspaceSymbolRequest,
    },
};
use parking_lot::Mutex;
use psp_types::{
    ExecuteProcess, ExecuteProcessParams, ExecuteProcessResult,
    RegisterDebuggerType, RegisterDebuggerTypeParams, Request, SendLspNotification,
    SendLspNotificationParams, SendLspRequest, SendLspRequestParams,
    SendLspRequestResult, StartLspServer, StartLspServerParams,
    StartLspServerResult,
};
use serde::Serialize;
use serde_json::Value;

use super::{
    PluginCatalogRpcHandler,
    lsp::{DocumentFilter, LspClient},
};

pub enum ResponseHandler<Resp, Error> {
    Chan(Sender<(Id, Result<Resp, Error>)>),
    Callback(Box<dyn RpcCallback<Resp, Error>>),
}

impl<Resp, Error> ResponseHandler<Resp, Error> {
    pub fn invoke(self, id: Id, result: Result<Resp, Error>) {
        match self {
            ResponseHandler::Chan(tx) => {
                if let Err(err) = tx.send((id, result)) {
                    log::error!("{:?}", err);
                }
            },
            ResponseHandler::Callback(f) => f.call(id, result),
        }
    }
}

pub trait ClonableCallback<Resp, Error>:
    FnOnce(Id, PluginId, Result<Resp, Error>) + Send + DynClone {
}

impl<Resp, Error, F: Send + FnOnce(Id, PluginId, Result<Resp, Error>) + DynClone>
    ClonableCallback<Resp, Error> for F
{
}

pub trait RpcCallback<Resp, Error>: Send {
    fn call(self: Box<Self>, id: Id, result: Result<Resp, Error>);
}

impl<Resp, Error, F: Send + FnOnce(Id, Result<Resp, Error>)> RpcCallback<Resp, Error>
    for F
{
    fn call(self: Box<F>, id: Id, result: Result<Resp, Error>) {
        (*self)(id, result)
    }
}
#[allow(clippy::large_enum_variant)]
pub enum PluginHandlerNotification {
    Initialize(u64),
    InitializeResult(InitializeResult),
    Shutdown,
    SpawnedPluginLoaded { plugin_id: PluginId },
}
#[allow(clippy::large_enum_variant)]
pub enum PluginServerRpc {
    Shutdown,
    Handler(PluginHandlerNotification),
    HostSendToPluginOrLspRequest {
        id:          Id,
        method:      Cow<'static, str>,
        params:      Params,
        language_id: Option<String>,
        path:        Option<PathBuf>,
        rh:          ResponseHandler<Value, RpcError>,
    },
    ServerNotification {
        method:      Cow<'static, str>,
        params:      Params,
        language_id: Option<String>,
        path:        Option<PathBuf>,
    },
    ToHostRequest {
        id:     Id,
        method: String,
        params: Params,
        resp:   ResponseSender,
    },
    ToHostNotification {
        method: String,
        params: Params,
        from:   String,
    },
    DidSaveTextDocument {
        language_id:   String,
        path:          PathBuf,
        text_document: TextDocumentIdentifier,
        text:          Rope,
    },
    DidChangeTextDocument {
        language_id: String,
        document:    VersionedTextDocumentIdentifier,
        delta:       RopeDelta,
        text:        Rope,
        new_text:    Rope,
        change: Arc<
            Mutex<(
                Option<TextDocumentContentChangeEvent>,
                Option<TextDocumentContentChangeEvent>,
            )>,
        >,
    },
    FormatSemanticTokens {
        id:     u64,
        tokens: SemanticTokens,
        text:   Rope,
        f:      Box<dyn RpcCallback<(Vec<LineStyle>, Option<String>), RpcError>>,
    },
}
#[derive(Clone)]
pub enum HandlerType {
    Plugin,
    Lsp,
}

impl HandlerType {
    pub fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin)
    }
}

#[derive(Clone)]
pub struct PluginServerRpcHandler {
    pub spawned_by:   Option<PluginId>,
    pub plugin_id:    PluginId,
    pub volt_id:      VoltID,
    pub handler_type: HandlerType,
    rpc_tx:           Sender<PluginServerRpc>,
    rpc_rx:           Receiver<PluginServerRpc>,
    io_tx:            Sender<JsonRpc>,
    server_pending:   Arc<Mutex<HashMap<Id, ResponseHandler<Value, RpcError>>>>,
}

#[derive(Clone)]
pub struct ResponseSender {
    tx: Sender<Result<Value, RpcError>>,
}
impl ResponseSender {
    pub fn new(tx: Sender<Result<Value, RpcError>>) -> Self {
        Self { tx }
    }

    pub fn send(&self, result: impl Serialize) {
        let result = serde_json::to_value(result).map_err(|e| RpcError {
            code:    0,
            message: e.to_string(),
        });
        if let Err(err) = self.tx.send(result) {
            log::error!("{:?}", err);
        }
    }

    pub fn send_null(&self) {
        if let Err(err) = self.tx.send(Ok(Value::Null)) {
            log::error!("{:?}", err);
        }
    }

    pub fn send_err(&self, code: i64, message: impl Into<String>) {
        if let Err(err) = self.tx.send(Err(RpcError {
            code,
            message: message.into(),
        })) {
            log::error!("{:?}", err);
        }
    }
}

pub trait PluginServerHandler {
    fn document_supported(
        &mut self,
        language_id: Option<&str>,
        path: Option<&Path>,
    ) -> bool;
    fn method_registered(&mut self, method: &str) -> bool;
    fn handle_host_notification(
        &mut self,
        method: String,
        params: Params,
        from: String,
    ) -> Result<()>;
    fn handle_to_host_request(
        &mut self,
        id: Id,
        method: String,
        params: Params,
        chan: ResponseSender,
    );
    fn handle_handler_notification(
        &mut self,
        notification: PluginHandlerNotification,
    ) -> Result<()>;
    fn handle_did_save_text_document(
        &self,
        language_id: String,
        path: PathBuf,
        text_document: TextDocumentIdentifier,
        text: Rope,
    ) -> Result<()>;
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
    ) -> Result<()>;
    fn format_semantic_tokens(
        &self,
        id: u64,
        tokens: SemanticTokens,
        text: Rope,
        f: Box<dyn RpcCallback<(Vec<LineStyle>, Option<String>), RpcError>>,
    );
}

impl PluginServerRpcHandler {
    pub fn new(
        volt_id: VoltID,
        spawned_by: Option<PluginId>,
        plugin_id: Option<PluginId>,
        io_tx: Sender<JsonRpc>,
        id: u64,
        handler_type: HandlerType,
    ) -> Result<Self> {
        let (rpc_tx, rpc_rx) = crossbeam_channel::unbounded();

        let rpc = Self {
            spawned_by,
            volt_id,
            plugin_id: plugin_id.unwrap_or_else(PluginId::next),
            rpc_tx,
            rpc_rx,
            io_tx,
            server_pending: Arc::new(Mutex::new(HashMap::new())),
            handler_type,
        };

        rpc.initialize(id)?;
        Ok(rpc)
    }

    fn initialize(&self, id: u64) -> Result<()> {
        self.handle_rpc(PluginServerRpc::Handler(
            PluginHandlerNotification::Initialize(id),
        ))
    }

    fn send_server_request(
        &self,
        id: Id,
        method: &str,
        params: Params,
        rh: ResponseHandler<Value, RpcError>,
    ) -> Result<()> {
        {
            let mut pending = self.server_pending.lock();
            if pending.insert(id.clone(), rh).is_some() {
                error!("send_server_request {id:?} repeat");
            }
        }
        let msg = JsonRpc::request_with_params(id, method, params);
        self.send_server_rpc(msg)
    }

    fn send_server_notification(&self, method: &str, params: Params) -> Result<()> {
        let msg = JsonRpc::notification_with_params(method, params);
        self.send_server_rpc(msg)
    }

    fn send_server_rpc(&self, msg: JsonRpc) -> Result<()> {
        self.io_tx.send(msg)?;
        Ok(())
    }

    pub fn handle_rpc(&self, rpc: PluginServerRpc) -> Result<()> {
        self.rpc_tx
            .send(rpc)
            .map_err(|_| anyhow!("send rpc fail"))?;
        Ok(())
    }

    /// Send a notification.  
    /// The callback is called when the function is actually sent.
    pub fn server_notification<P: Serialize>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: P,
        language_id: Option<String>,
        path: Option<PathBuf>,
        check: bool,
    ) -> Result<()> {
        let params = Params::from(serde_json::to_value(params).unwrap());
        let method = method.into();

        if check {
            self.rpc_tx
                .send(PluginServerRpc::ServerNotification {
                    method,
                    params,
                    language_id,
                    path,
                })
                .map_err(|_| anyhow!("send notification fail"))?;
        } else {
            self.send_server_notification(&method, params)?;
        }
        Ok(())
    }

    /// Make a request to plugin/language server and get the response.
    ///
    /// When check is true, the request will be in the handler mainloop to
    /// do checks like if the server has the capability of the request.
    ///
    /// When check is false, the request will be sent out straight away.
    pub fn server_request<P: Serialize>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: P,
        language_id: Option<String>,
        path: Option<PathBuf>,
        check: bool,
        id: u64,
    ) -> Result<Value, RpcError> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.server_request_common(
            method.into(),
            params,
            language_id,
            path,
            check,
            id,
            ResponseHandler::Chan(tx),
        )
        .map_err(|err| {
            error!("server_request_common {err}");
            RpcError {
                code:    0,
                message: "io error".to_string(),
            }
        })?;
        rx.recv()
            .map_err(|_| RpcError {
                code:    0,
                message: "io error".to_string(),
            })?
            .1
    }

    #[allow(clippy::too_many_arguments)]
    pub fn server_request_async<P: Serialize>(
        &self,
        method: impl Into<Cow<'static, str>>,
        params: P,
        language_id: Option<String>,
        path: Option<PathBuf>,
        check: bool,
        id: u64,
        f: impl RpcCallback<Value, RpcError> + 'static,
    ) -> Result<()> {
        self.server_request_common(
            method.into(),
            params,
            language_id,
            path,
            check,
            id,
            ResponseHandler::Callback(Box::new(f)),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn server_request_common<P: Serialize>(
        &self,
        method: Cow<'static, str>,
        params: P,
        language_id: Option<String>,
        path: Option<PathBuf>,
        check: bool,
        id: u64,
        rh: ResponseHandler<Value, RpcError>,
    ) -> Result<()> {
        let params = Params::from(serde_json::to_value(params).unwrap());
        if check {
            self.rpc_tx
                .send(PluginServerRpc::HostSendToPluginOrLspRequest {
                    id: Id::Num(id as i64),
                    method,
                    params,
                    language_id,
                    path,
                    rh,
                })
                .map_err(|_| anyhow!("send fail"))?;
            Ok(())
        } else {
            self.send_server_request(Id::Num(id as i64), &method, params, rh)
        }
    }

    pub fn handle_server_response(&self, id: Id, result: Result<Value, RpcError>) {
        match self.server_pending.lock().remove(&id) {
            Some(handler) => handler.invoke(id, result),
            None => error!("handle_server_response {id:?} not found"),
        }
    }

    pub fn shutdown(&self) -> Result<()> {
        // to kill lsp
        self.handle_rpc(PluginServerRpc::Handler(
            PluginHandlerNotification::Shutdown,
        ))?;
        // to end PluginServerRpcHandler::mainloop
        self.handle_rpc(PluginServerRpc::Shutdown)
    }

    pub fn mainloop<H>(&self, handler: &mut H, _handler_name: &str)
    where
        H: PluginServerHandler, {
        for msg in &self.rpc_rx {
            match msg {
                PluginServerRpc::HostSendToPluginOrLspRequest {
                    id,
                    method,
                    params,
                    language_id,
                    path,
                    rh,
                } => {
                    if handler
                        .document_supported(language_id.as_deref(), path.as_deref())
                        && handler.method_registered(&method)
                    {
                        if let Err(err) =
                            self.send_server_request(id, &method, params, rh)
                        {
                            error!("send_server_request fail {err}")
                        }
                    } else {
                        let message =
                            format!("{_handler_name} not capable: {method}");
                        debug!("{message}");
                        rh.invoke(id, Err(RpcError { code: 0, message }));
                    }
                },
                PluginServerRpc::ServerNotification {
                    method,
                    params,
                    language_id,
                    path,
                } => {
                    if handler
                        .document_supported(language_id.as_deref(), path.as_deref())
                        && handler.method_registered(&method)
                    {
                        if let Err(err) =
                            self.send_server_notification(&method, params)
                        {
                            error!("send_server_notification fail {err}")
                        }
                    }
                },
                PluginServerRpc::ToHostRequest {
                    id,
                    method,
                    params,
                    resp,
                } => {
                    handler.handle_to_host_request(id, method, params, resp);
                },
                PluginServerRpc::ToHostNotification {
                    method,
                    params,
                    from,
                } => {
                    if let Err(err) =
                        handler.handle_host_notification(method, params, from)
                    {
                        error!("handle_host_notification {err}")
                    }
                },
                PluginServerRpc::DidSaveTextDocument {
                    language_id,
                    path,
                    text_document,
                    text,
                } => {
                    if let Err(err) = handler.handle_did_save_text_document(
                        language_id,
                        path,
                        text_document,
                        text,
                    ) {
                        error!("handle_did_save_text_document {err}")
                    }
                },
                PluginServerRpc::DidChangeTextDocument {
                    language_id,
                    document,
                    delta,
                    text,
                    new_text,
                    change,
                } => {
                    if let Err(err) = handler.handle_did_change_text_document(
                        language_id,
                        document,
                        delta,
                        text,
                        new_text,
                        change,
                    ) {
                        error!("handle_did_change_text_document {err}")
                    }
                },
                PluginServerRpc::FormatSemanticTokens {
                    id,
                    tokens,
                    text,
                    f,
                } => {
                    handler.format_semantic_tokens(id, tokens, text, f);
                },
                PluginServerRpc::Handler(notification) => {
                    if let Err(err) =
                        handler.handle_handler_notification(notification)
                    {
                        error!("handle_handler_notification {err:?}");
                    }
                },
                PluginServerRpc::Shutdown => {
                    return;
                },
            }
        }
    }
}

pub fn handle_plugin_server_message(
    server_rpc: &PluginServerRpcHandler,
    message: &str,
    from: &str,
) -> Option<JsonRpc> {
    match JsonRpc::parse(message) {
        Ok(value @ JsonRpc::Request(_)) => {
            let (tx, rx) = crossbeam_channel::bounded(1);
            let id = value.get_id().unwrap();
            let rpc = PluginServerRpc::ToHostRequest {
                id:     id.clone(),
                method: value.get_method().unwrap().to_string(),
                params: value.get_params().unwrap(),
                resp:   ResponseSender::new(tx),
            };
            if let Err(err) = server_rpc.handle_rpc(rpc) {
                error!("handle_host_notification {err}");
                return None;
            }
            let result = rx.recv().unwrap();
            let resp = match result {
                Ok(v) => JsonRpc::success(id, &v),
                Err(e) => JsonRpc::error(
                    id,
                    jsonrpc_lite::Error {
                        code:    e.code,
                        message: e.message,
                        data:    None,
                    },
                ),
            };
            Some(resp)
        },
        Ok(value @ JsonRpc::Notification(_)) => {
            let rpc = PluginServerRpc::ToHostNotification {
                method: value.get_method().unwrap().to_string(),
                params: value.get_params().unwrap(),
                from:   from.to_string(),
            };
            if let Err(err) = server_rpc.handle_rpc(rpc) {
                error!("handle_rpc Notification {err}");
                return None;
            }
            None
        },
        Ok(value @ JsonRpc::Success(_)) => {
            let result = value.get_result().unwrap().clone();
            server_rpc.handle_server_response(value.get_id().unwrap(), Ok(result));
            None
        },
        Ok(value @ JsonRpc::Error(_)) => {
            let error = value.get_error().unwrap();
            server_rpc.handle_server_response(
                value.get_id().unwrap(),
                Err(RpcError {
                    code:    error.code,
                    message: error.message.clone(),
                }),
            );
            None
        },
        Err(err) => {
            eprintln!("parse error {err} message {message}");
            None
        },
    }
}

struct SaveRegistration {
    include_text: bool,
    filters:      Vec<DocumentFilter>,
}

#[derive(Default)]
struct ServerRegistrations {
    save: Option<SaveRegistration>,
}

pub struct PluginHostHandler {
    volt_id:                 VoltID,
    pub volt_display_name:   String,
    pwd:                     Option<PathBuf>,
    pub(crate) workspace:    Option<PathBuf>,
    document_selector:       Vec<DocumentFilter>,
    core_rpc:                CoreRpcHandler,
    catalog_rpc:             PluginCatalogRpcHandler,
    pub server_rpc:          PluginServerRpcHandler,
    pub server_capabilities: ServerCapabilities,
    server_registrations:    ServerRegistrations,

    /// Language servers that this plugin has spawned.  
    /// Note that these plugin ids could be 'dead' if the LSP died/exited.  
    spawned_lsp: HashMap<PluginId, SpawnedLspInfo>,
}

impl PluginHostHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workspace: Option<PathBuf>,
        pwd: Option<PathBuf>,
        volt_id: VoltID,
        volt_display_name: String,
        document_selector: DocumentSelector,
        core_rpc: CoreRpcHandler,
        server_rpc: PluginServerRpcHandler,
        catalog_rpc: PluginCatalogRpcHandler,
    ) -> Self {
        let document_selector = document_selector
            .iter()
            .map(DocumentFilter::from_lsp_filter_loose)
            .collect();
        Self {
            pwd,
            workspace,
            volt_id,
            volt_display_name,
            document_selector,
            core_rpc,
            catalog_rpc,
            server_rpc,
            server_capabilities: ServerCapabilities::default(),
            server_registrations: ServerRegistrations::default(),
            spawned_lsp: HashMap::new(),
        }
    }

    pub fn document_supported(
        &self,
        language_id: Option<&str>,
        path: Option<&Path>,
    ) -> bool {
        match language_id {
            Some(language_id) => {
                for filter in self.document_selector.iter() {
                    if (filter.language_id.is_none()
                        || filter.language_id.as_deref() == Some(language_id))
                        && (path.is_none()
                            || filter.pattern.is_none()
                            || filter
                                .pattern
                                .as_ref()
                                .unwrap()
                                .is_match(path.as_ref().unwrap()))
                    {
                        return true;
                    }
                }
                false
            },
            None => true,
        }
    }

    pub fn method_registered(&mut self, method: &str) -> bool {
        match method {
            Initialize::METHOD => true,
            Initialized::METHOD => true,
            Completion::METHOD => {
                self.server_capabilities.completion_provider.is_some()
            },
            OnTypeFormatting::METHOD => self
                .server_capabilities
                .document_on_type_formatting_provider
                .is_some(),
            ResolveCompletionItem::METHOD => self
                .server_capabilities
                .completion_provider
                .as_ref()
                .and_then(|c| c.resolve_provider)
                .unwrap_or(false),
            DidOpenTextDocument::METHOD => {
                match &self.server_capabilities.text_document_sync {
                    Some(TextDocumentSyncCapability::Kind(kind)) => {
                        kind != &TextDocumentSyncKind::NONE
                    },
                    Some(TextDocumentSyncCapability::Options(options)) => options
                        .open_close
                        .or_else(|| {
                            options
                                .change
                                .map(|kind| kind != TextDocumentSyncKind::NONE)
                        })
                        .unwrap_or(false),
                    None => false,
                }
            },
            DidChangeTextDocument::METHOD => {
                match &self.server_capabilities.text_document_sync {
                    Some(TextDocumentSyncCapability::Kind(kind)) => {
                        kind != &TextDocumentSyncKind::NONE
                    },
                    Some(TextDocumentSyncCapability::Options(options)) => options
                        .change
                        .map(|kind| kind != TextDocumentSyncKind::NONE)
                        .unwrap_or(false),
                    None => false,
                }
            },
            SignatureHelpRequest::METHOD => {
                self.server_capabilities.signature_help_provider.is_some()
            },
            HoverRequest::METHOD => self
                .server_capabilities
                .hover_provider
                .as_ref()
                .map(|c| match c {
                    HoverProviderCapability::Simple(is_capable) => *is_capable,
                    HoverProviderCapability::Options(_) => true,
                })
                .unwrap_or(false),
            GotoDefinition::METHOD => self
                .server_capabilities
                .definition_provider
                .as_ref()
                .map(|d| match d {
                    OneOf::Left(is_capable) => *is_capable,
                    OneOf::Right(_) => true,
                })
                .unwrap_or(false),
            GotoTypeDefinition::METHOD => {
                self.server_capabilities.type_definition_provider.is_some()
            },
            References::METHOD => self
                .server_capabilities
                .references_provider
                .as_ref()
                .map(|r| match r {
                    OneOf::Left(is_capable) => *is_capable,
                    OneOf::Right(_) => true,
                })
                .unwrap_or(false),
            GotoImplementation::METHOD => self
                .server_capabilities
                .implementation_provider
                .as_ref()
                .map(|r| match r {
                    ImplementationProviderCapability::Simple(is_capable) => {
                        *is_capable
                    },
                    ImplementationProviderCapability::Options(_) => {
                        // todo
                        false
                    },
                })
                .unwrap_or(false),
            FoldingRangeRequest::METHOD => self
                .server_capabilities
                .folding_range_provider
                .as_ref()
                .map(|r| match r {
                    FoldingRangeProviderCapability::Simple(support) => *support,
                    FoldingRangeProviderCapability::FoldingProvider(_) => {
                        // todo
                        true
                    },
                    FoldingRangeProviderCapability::Options(_) => {
                        // todo
                        true
                    },
                })
                .unwrap_or(false),
            CodeActionRequest::METHOD => self
                .server_capabilities
                .code_action_provider
                .as_ref()
                .map(|a| match a {
                    CodeActionProviderCapability::Simple(is_capable) => *is_capable,
                    CodeActionProviderCapability::Options(_) => true,
                })
                .unwrap_or(false),
            Formatting::METHOD => self
                .server_capabilities
                .document_formatting_provider
                .as_ref()
                .map(|f| match f {
                    OneOf::Left(is_capable) => *is_capable,
                    OneOf::Right(_) => true,
                })
                .unwrap_or(false),
            SemanticTokensFullRequest::METHOD => {
                self.server_capabilities.semantic_tokens_provider.is_some()
            },
            SemanticTokensFullDeltaRequest::METHOD => {
                if let Some(provider) =
                    &self.server_capabilities.semantic_tokens_provider
                {
                    if let Some(SemanticTokensFullOptions::Delta { delta: Some(delta) }) = match provider {
                        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
                            &options.full
                        }
                        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
                            &options.semantic_tokens_options.full
                        }
                    }{
                        *delta
                    } else {
                        false
                    }
                } else {
                    false
                }
            },
            InlayHintRequest::METHOD => {
                self.server_capabilities.inlay_hint_provider.is_some()
            },
            InlineCompletionRequest::METHOD => self
                .server_capabilities
                .inline_completion_provider
                .is_some(),
            DocumentSymbolRequest::METHOD => {
                self.server_capabilities.document_symbol_provider.is_some()
            },
            WorkspaceSymbolRequest::METHOD => {
                self.server_capabilities.workspace_symbol_provider.is_some()
            },
            PrepareRenameRequest::METHOD => {
                self.server_capabilities.rename_provider.is_some()
            },
            Rename::METHOD => self.server_capabilities.rename_provider.is_some(),
            SelectionRangeRequest::METHOD => {
                self.server_capabilities.selection_range_provider.is_some()
            },
            CodeActionResolveRequest::METHOD => {
                self.server_capabilities.code_action_provider.is_some()
            },
            CodeLensRequest::METHOD => {
                self.server_capabilities.code_lens_provider.is_some()
            },
            CodeLensResolve::METHOD => self
                .server_capabilities
                .code_lens_provider
                .as_ref()
                .and_then(|x| x.resolve_provider)
                .unwrap_or(false),
            CallHierarchyPrepare::METHOD => {
                self.server_capabilities.call_hierarchy_provider.is_some()
            },
            CallHierarchyIncomingCalls::METHOD => {
                self.server_capabilities.call_hierarchy_provider.is_some()
            },
            DocumentHighlightRequest::METHOD => self
                .server_capabilities
                .document_highlight_provider
                .is_some(),
            "experimental/onEnter" => {
                if let Some(Value::Object(values)) =
                    &self.server_capabilities.experimental
                    && let Some(Value::Bool(on_enter)) = values.get("onEnter")
                {
                    *on_enter
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    fn check_save_capability(&self, language_id: &str, path: &Path) -> (bool, bool) {
        if self.document_supported(Some(language_id), Some(path)) {
            let (should_send, include_text) = self
                .server_capabilities
                .text_document_sync
                .as_ref()
                .and_then(|sync| match sync {
                    TextDocumentSyncCapability::Kind(_) => None,
                    TextDocumentSyncCapability::Options(options) => Some(options),
                })
                .and_then(|o| o.save.as_ref())
                .map(|o| match o {
                    TextDocumentSyncSaveOptions::Supported(is_supported) => {
                        (*is_supported, true)
                    },
                    TextDocumentSyncSaveOptions::SaveOptions(options) => {
                        (true, options.include_text.unwrap_or(false))
                    },
                })
                .unwrap_or((false, false));
            return (should_send, include_text);
        }

        if let Some(options) = self.server_registrations.save.as_ref() {
            for filter in options.filters.iter() {
                if (filter.language_id.is_none()
                    || filter.language_id.as_deref() == Some(language_id))
                    && (filter.pattern.is_none()
                        || filter.pattern.as_ref().unwrap().is_match(path))
                {
                    return (true, options.include_text);
                }
            }
        }

        (false, false)
    }

    fn register_capabilities(&mut self, registrations: Vec<Registration>) {
        for registration in registrations {
            if let Err(err) = self.register_capability(registration) {
                log::error!("{:?}", err);
            }
        }
    }

    fn register_capability(&mut self, registration: Registration) -> Result<()> {
        match registration.method.as_str() {
            DidSaveTextDocument::METHOD => {
                let options = registration
                    .register_options
                    .ok_or_else(|| anyhow!("don't have options"))?;
                let options: TextDocumentSaveRegistrationOptions =
                    serde_json::from_value(options)?;
                self.server_registrations.save = Some(SaveRegistration {
                    include_text: options.include_text.unwrap_or(false),
                    filters:      options
                        .text_document_registration_options
                        .document_selector
                        .map(|s| {
                            s.iter()
                                .map(DocumentFilter::from_lsp_filter_loose)
                                .collect()
                        })
                        .unwrap_or_default(),
                });
            },
            _ => {
                eprintln!(
                    "don't handle register capability for {}",
                    registration.method
                );
            },
        }
        Ok(())
    }

    pub fn handle_request(
        &mut self,
        _id: Id,
        method: String,
        params: Params,
        resp: ResponseSender,
    ) {
        if let Err(err) = self.process_request(method, params, resp.clone()) {
            resp.send_err(0, err.to_string());
        }
    }

    pub fn process_request(
        &mut self,
        method: String,
        params: Params,
        resp: ResponseSender,
    ) -> Result<()> {
        match method.as_str() {
            WorkDoneProgressCreate::METHOD => {
                resp.send_null();
            },
            RegisterCapability::METHOD => {
                let params: RegistrationParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                self.register_capabilities(params.registrations);
                resp.send_null();
            },
            ExecuteProcess::METHOD => {
                let params: ExecuteProcessParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                let output = std::process::Command::new(params.program)
                    .args(params.args)
                    .output()?;

                resp.send(ExecuteProcessResult {
                    success: output.status.success(),
                    stdout:  Some(output.stdout),
                    stderr:  Some(output.stderr),
                });
            },
            RegisterDebuggerType::METHOD => {
                let params: RegisterDebuggerTypeParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                self.catalog_rpc.register_debugger_type(
                    params.debugger_type,
                    params.program,
                    params.args,
                );
                resp.send_null();
            },
            StartLspServer::METHOD => {
                let params: StartLspServerParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                let workspace = self.workspace.clone();
                let pwd = self.pwd.clone();
                let catalog_rpc = self.catalog_rpc.clone();
                let volt_id = self.volt_id.clone();
                let volt_display_name = self.volt_display_name.clone();

                let spawned_by = self.server_rpc.plugin_id;
                let plugin_id = PluginId::next();
                self.spawned_lsp
                    .insert(plugin_id, SpawnedLspInfo { resp: Some(resp) });
                thread::spawn(move || {
                    if let Err(err) = LspClient::start(
                        catalog_rpc,
                        params.document_selector,
                        workspace,
                        volt_id,
                        volt_display_name,
                        Some(spawned_by),
                        Some(plugin_id),
                        pwd,
                        params.server_uri,
                        params.server_args,
                        params.options,
                        0,
                    ) {
                        log::error!("{:?}", err);
                    }
                });
            },
            SendLspNotification::METHOD => {
                let params: SendLspNotificationParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                let lsp_id = params.id;
                let method = params.method;
                let params = params.params;

                // The lsp ids we give the plugins are just the plugin id of the lsp
                let plugin_id = PluginId(lsp_id);

                if !self.spawned_lsp.contains_key(&plugin_id) {
                    return Err(anyhow!("lsp not found, it may have exited"));
                }

                // Send the notification to the plugin
                self.catalog_rpc.send_notification(
                    Some(plugin_id),
                    method.to_string(),
                    params,
                    None,
                    None,
                    false,
                );
            },
            SendLspRequest::METHOD => {
                let params: SendLspRequestParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                let lsp_id = params.id;
                let method = params.method;
                let params = params.params;

                // The lsp ids we give the plugins are just the plugin id of the lsp
                let plugin_id = PluginId(lsp_id);

                if !self.spawned_lsp.contains_key(&plugin_id) {
                    return Err(anyhow!("lsp not found, it may have exited"));
                }

                // Send the request to the plugin
                self.catalog_rpc.send_request(
                    Some(plugin_id),
                    None,
                    method.to_string(),
                    params,
                    None,
                    None,
                    false,
                    lsp_id,
                    move |_, _, res| {
                        // We just directly send it back to the plugin that requested
                        // this
                        match res {
                            Ok(res) => {
                                resp.send(SendLspRequestResult { result: res });
                            },
                            Err(err) => {
                                resp.send_err(err.code, err.message);
                            },
                        }
                    },
                )
            },
            _ => return Err(anyhow!("request not supported")),
        }

        Ok(())
    }

    pub fn handle_notification(
        &mut self,
        method: String,
        params: Params,
        from: String,
    ) -> Result<()> {
        match method.as_str() {
            // TODO: remove this after the next release and once we convert all the
            // existing plugins to use the request.
            StartLspServer::METHOD => {
                self.core_rpc.log(
                    lapce_rpc::core::LogLevel::Warn,
                    format!(
                        "[{}] Usage of startLspServer as a notification is \
                         deprecated.",
                        self.volt_display_name
                    ),
                    Some(format!(
                        "lapce_proxy::plugin::psp::{}::{}::StartLspServer",
                        self.volt_id.author, self.volt_id.name
                    )),
                );

                let params: StartLspServerParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                let workspace = self.workspace.clone();
                let pwd = self.pwd.clone();
                let catalog_rpc = self.catalog_rpc.clone();
                let volt_id = self.volt_id.clone();
                let volt_display_name = self.volt_display_name.clone();
                thread::spawn(move || {
                    if let Err(err) = LspClient::start(
                        catalog_rpc,
                        params.document_selector,
                        workspace,
                        volt_id,
                        volt_display_name,
                        None,
                        None,
                        pwd,
                        params.server_uri,
                        params.server_args,
                        params.options,
                        0,
                    ) {
                        log::error!("{:?}", err);
                    }
                });
            },
            PublishDiagnostics::METHOD => {
                let diagnostics: PublishDiagnosticsParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                self.catalog_rpc.core_rpc.publish_diagnostics(diagnostics);
            },
            Progress::METHOD => {
                let progress: ProgressParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                self.catalog_rpc.core_rpc.work_done_progress(progress);
            },
            ShowMessage::METHOD => {
                let message: ShowMessageParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                let title = format!("Plugin: {}", self.volt_display_name);
                self.catalog_rpc.core_rpc.show_message(title, message);
            },
            LogMessage::METHOD => {
                let message: LogMessageParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                self.catalog_rpc.core_rpc.log_message(
                    message,
                    format!(
                        "lapce_proxy::plugin::psp::{}::{}::LogMessage",
                        self.volt_id.author, self.volt_id.name
                    ),
                );
            },
            Cancel::METHOD => {
                let params: CancelParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                self.catalog_rpc.core_rpc.cancel(params);
            },
            "experimental/serverStatus" => {
                let param: ServerStatusParams =
                    serde_json::from_value(serde_json::to_value(params)?)?;
                if !param.is_ok() {
                    if let Some(msg) = &param.message {
                        self.core_rpc.show_message(
                            from,
                            ShowMessageParams {
                                typ:     MessageType::ERROR,
                                message: msg.clone(),
                            },
                        );
                    }
                }
                self.catalog_rpc.core_rpc.server_status(param);
            },
            _ => {
                self.core_rpc.log(
                    lapce_rpc::core::LogLevel::Warn,
                    format!("host notification {method} not handled"),
                    Some(format!(
                        "lapce_proxy::plugin::psp::{}::{}::{method}",
                        self.volt_id.author, self.volt_id.name
                    )),
                );
            },
        }
        Ok(())
    }

    pub fn handle_did_save_text_document(
        &self,
        language_id: String,
        path: PathBuf,
        text_document: TextDocumentIdentifier,
        text: Rope,
    ) -> Result<()> {
        let (should_send, include_text) =
            self.check_save_capability(language_id.as_str(), &path);
        if !should_send {
            return Ok(());
        }
        let params = DidSaveTextDocumentParams {
            text_document,
            text: if include_text {
                Some(text.to_string())
            } else {
                None
            },
        };
        self.server_rpc.server_notification(
            DidSaveTextDocument::METHOD,
            params,
            Some(language_id),
            Some(path),
            false,
        )
    }

    pub fn handle_did_change_text_document(
        &mut self,
        lanaguage_id: String,
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
        let kind = match &self.server_capabilities.text_document_sync {
            Some(TextDocumentSyncCapability::Kind(kind)) => *kind,
            Some(TextDocumentSyncCapability::Options(options)) => {
                options.change.unwrap_or(TextDocumentSyncKind::NONE)
            },
            None => TextDocumentSyncKind::NONE,
        };

        let mut existing = change.lock();
        let change = match kind {
            TextDocumentSyncKind::FULL => {
                if let Some(c) = existing.0.as_ref() {
                    c.clone()
                } else {
                    let change = TextDocumentContentChangeEvent {
                        range:        None,
                        range_length: None,
                        text:         new_text.to_string(),
                    };
                    existing.0 = Some(change.clone());
                    change
                }
            },
            TextDocumentSyncKind::INCREMENTAL => {
                if let Some(c) = existing.1.as_ref() {
                    c.clone()
                } else {
                    let change = get_document_content_change(&text, &delta)
                        .unwrap_or_else(|| TextDocumentContentChangeEvent {
                            range:        None,
                            range_length: None,
                            text:         new_text.to_string(),
                        });
                    existing.1 = Some(change.clone());
                    change
                }
            },
            TextDocumentSyncKind::NONE => return Ok(()),
            _ => return Ok(()),
        };

        let path = document.uri.to_file_path().ok();

        let params = DidChangeTextDocumentParams {
            text_document:   document,
            content_changes: vec![change],
        };

        self.server_rpc.server_notification(
            DidChangeTextDocument::METHOD,
            params,
            Some(lanaguage_id),
            path,
            false,
        )
    }

    pub fn format_semantic_tokens(
        &self,
        id: u64,
        tokens: SemanticTokens,
        text: Rope,
        f: Box<dyn RpcCallback<(Vec<LineStyle>, Option<String>), RpcError>>,
    ) {
        let result = format_semantic_styles(
            &text,
            self.server_capabilities.semantic_tokens_provider.as_ref(),
            &tokens,
        )
        .ok_or_else(|| RpcError {
            code:    0,
            message: "can't get styles".to_string(),
        });
        f.call(Id::Num(id as i64), result);
    }

    pub fn handle_spawned_plugin_loaded(&mut self, plugin_id: PluginId) {
        if let Some(info) = self.spawned_lsp.get_mut(&plugin_id) {
            let Some(resp) = info.resp.take() else {
                self.core_rpc.log(
                    lapce_rpc::core::LogLevel::Warn,
                    "Spawned lsp initialized twice?".to_string(),
                    Some(format!(
                        "{}::{}::handle_spawned_plugin_loaded",
                        self.volt_id.author, self.volt_id.name
                    )),
                );
                return;
            };

            resp.send(StartLspServerResult { id: plugin_id.0 });
        }
    }
}

/// Information that a plugin associates with a spawned language server.
struct SpawnedLspInfo {
    /// The response sender to use when the lsp is initialized.
    resp: Option<ResponseSender>,
}

fn get_document_content_change(
    text: &Rope,
    delta: &RopeDelta,
) -> Option<TextDocumentContentChangeEvent> {
    let (interval, _) = delta.summary();
    let (start, end) = interval.start_end();

    let text = RopeTextRef::new(text);

    // TODO: Handle more trivial cases like typing when there's a selection or
    // transpose
    if let Some(node) = delta.as_simple_insert() {
        let (start, end) = interval.start_end();
        let Ok(start) = text.offset_to_position(start) else {
            error!("{start}");
            return None;
        };
        let Ok(end) = text.offset_to_position(end) else {
            error!("{end}");
            return None;
        };

        let text = String::from(node);
        let text_document_content_change_event = TextDocumentContentChangeEvent {
            range: Some(Range { start, end }),
            range_length: None,
            text,
        };

        return Some(text_document_content_change_event);
    }
    // Or a simple delete
    else if delta.is_simple_delete() {
        let Ok(end_position) = text.offset_to_position(end) else {
            error!("{end}");
            return None;
        };
        let Ok(start) = text.offset_to_position(start) else {
            error!("{start}");
            return None;
        };
        let text_document_content_change_event = TextDocumentContentChangeEvent {
            range:        Some(Range {
                start,
                end: end_position,
            }),
            range_length: None,
            text:         String::new(),
        };

        return Some(text_document_content_change_event);
    }

    None
}

fn format_semantic_styles(
    text: &Rope,
    semantic_tokens_provider: Option<&SemanticTokensServerCapabilities>,
    tokens: &SemanticTokens,
) -> Option<(Vec<LineStyle>, Option<String>)> {
    let semantic_tokens_provider = semantic_tokens_provider?;
    let semantic_legends = semantic_tokens_legend(semantic_tokens_provider);

    let text = RopeTextRef::new(text);
    let mut highlights = Vec::new();
    let mut line = 0;
    let mut start = 0;
    let mut last_start = 0;
    for semantic_token in &tokens.data {
        if semantic_token.delta_line > 0 {
            line += semantic_token.delta_line as usize;
            let Ok(line) = text.offset_of_line(line) else {
                error!("{line}");
                continue;
            };
            start = line;
        }

        let sub_text = text.char_indices_iter(start..);
        start += offset_utf16_to_utf8(sub_text, semantic_token.delta_start as usize);

        let sub_text = text.char_indices_iter(start..);
        let end =
            start + offset_utf16_to_utf8(sub_text, semantic_token.length as usize);

        let kind = semantic_legends.token_types[semantic_token.token_type as usize]
            .as_str()
            .to_string();
        if start < last_start {
            continue;
        }
        last_start = start;
        // todo what is the `text`
        highlights.push(LineStyle {
            start,
            end,
            text: None,
            style: Style {
                fg_color: Some(kind),
            },
        });
    }

    Some((highlights, tokens.result_id.clone()))
}

fn semantic_tokens_legend(
    semantic_tokens_provider: &SemanticTokensServerCapabilities,
) -> &SemanticTokensLegend {
    match semantic_tokens_provider {
        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
            &options.legend
        },
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
            options,
        ) => &options.semantic_tokens_options.legend,
    }
}

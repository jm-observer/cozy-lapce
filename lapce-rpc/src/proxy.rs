use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use indexmap::IndexMap;
use lapce_xi_rope::RopeDelta;
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CodeAction, CodeActionResponse,
    CodeLens, CompletionItem, Diagnostic, DocumentHighlight, DocumentSymbolResponse,
    FoldingRange, GotoDefinitionResponse, Hover, InlayHint,
    InlineCompletionResponse, InlineCompletionTriggerKind, Location, Position,
    PrepareRenameResponse, SelectionRange, SymbolInformation, TextDocumentItem,
    TextEdit, WorkspaceEdit,
    request::{GotoImplementationResponse, GotoTypeDefinitionResponse},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::plugin::VoltID;
use crate::{
    RequestId, RpcError, RpcMessage, RpcResult,
    buffer::BufferId,
    dap_types::{self, DapId, RunDebugConfig, SourceBreakpoint, ThreadId},
    file::{FileNodeItem, PathObject},
    file_line::FileLine,
    plugin::{PluginId, VoltInfo, VoltMetadata},
    rust_module_resolve::CargoContext,
    source_control::FileDiff,
    style::SemanticStyles,
    terminal::{TermId, TerminalProfile},
};

#[allow(clippy::large_enum_variant)]
pub enum ProxyRpc {
    Request(RequestId, ProxyRequest),
    Notification(ProxyNotification),
    Shutdown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ProxyStatus {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct SearchMatch {
    pub line:         usize,
    pub start:        usize,
    pub end:          usize,
    pub line_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "method", content = "params")]
pub enum ProxyLspRequest {
    CompletionResolve {
        plugin_id:       PluginId,
        completion_item: Box<CompletionItem>,
    },
    CodeActionResolve {
        plugin_id:   PluginId,
        action_item: Box<CodeAction>,
    },
    GetHover {
        request_id: usize,
        path:       PathBuf,
        position:   Position,
    },
    GetTypeDefinition {
        request_id: usize,
        path:       PathBuf,
        position:   Position,
    },
    GetInlayHints {
        path: PathBuf,
    },
    GetInlineCompletions {
        path:         PathBuf,
        position:     Position,
        trigger_kind: InlineCompletionTriggerKind,
    },
    GetSemanticTokens {
        path: PathBuf,
    },
    GetSemanticTokensDelta {
        path:               PathBuf,
        previous_result_id: String,
    },
    LspFoldingRange {
        path: PathBuf,
    },
    GetCodeActions {
        path:        PathBuf,
        position:    Position,
        diagnostics: Vec<Diagnostic>,
    },
    GetCodeLens {
        path: PathBuf,
    },
    GetCodeLensResolve {
        code_lens: CodeLens,
        path:      PathBuf,
    },
    GetDocumentFormatting {
        path: PathBuf,
    },
    OnEnter {
        path:     PathBuf,
        position: Position,
    },
    OnTypeFormatting {
        path:     PathBuf,
        position: Position,
        ch:       String,
    },
    GetDocumentSymbols {
        path: PathBuf,
    },
    GetReferences {
        path:     PathBuf,
        position: Position,
    },
    GotoImplementation {
        path:     PathBuf,
        position: Position,
    },
    GetDefinition {
        request_id: usize,
        path:       PathBuf,
        position:   Position,
    },
    ShowCallHierarchy {
        path:     PathBuf,
        position: Position,
    },
    DocumentHighlight {
        path:     PathBuf,
        position: Position,
    },
    CallHierarchyIncoming {
        path:                PathBuf,
        call_hierarchy_item: CallHierarchyItem,
    },
    GetSelectionRange {
        path:      PathBuf,
        positions: Vec<Position>,
    },
    Completion {
        request_id: usize,
        path:       PathBuf,
        input:      String,
        position:   Position,
    },
    SignatureHelp {
        request_id: usize,
        path:       PathBuf,
        position:   Position,
    },
}

impl From<ProxyLspRequest> for ProxyRequest {
    fn from(value: ProxyLspRequest) -> Self {
        Self::LspRequest(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "method", content = "params")]
pub enum ProxyRequest {
    GetSignature {
        buffer_id: BufferId,
        position:  Position,
    },
    NewBuffer {
        buffer_id:       BufferId,
        path:            PathBuf,
        check_if_exists: bool,
    },
    // ReloadBuffer {
    //     buffer_id: BufferId,
    //     path:      PathBuf,
    // },
    BufferHead {
        path: PathBuf,
    },
    GetAbsolutePath {
        path: PathBuf,
    },
    GlobalSearch {
        pattern:        String,
        case_sensitive: bool,
        whole_word:     bool,
        is_regex:       bool,
    },
    GitGetRemoteFileUrl {
        file: PathBuf,
    },
    LspRequest(ProxyLspRequest),
    PrepareRename {
        path:     PathBuf,
        position: Position,
    },
    Rename {
        path:     PathBuf,
        position: Position,
        new_name: String,
    },
    GetWorkspaceSymbols {
        /// The search query
        query: String,
    },
    GetOpenFilesContent {},
    GetFiles {
        path: String,
    },
    ReadDir {
        path: PathBuf,
    },
    Save {
        rev:            u64,
        path:           PathBuf,
        /// Whether to create the parent directories if they do not exist.
        create_parents: bool,
    },
    SaveBufferAs {
        buffer_id:      BufferId,
        path:           PathBuf,
        rev:            u64,
        content:        String,
        /// Whether to create the parent directories if they do not exist.
        create_parents: bool,
    },
    CreateFile {
        path: PathBuf,
    },
    CreateDirectory {
        path: PathBuf,
    },
    TrashPath {
        path: PathBuf,
    },
    DuplicatePath {
        existing_path: PathBuf,
        new_path:      PathBuf,
    },
    RenamePath {
        from: PathBuf,
        to:   PathBuf,
    },
    TestCreateAtPath {
        path: PathBuf,
    },
    DapVariable {
        dap_id:    DapId,
        reference: usize,
    },
    DapGetScopes {
        dap_id:   DapId,
        frame_id: usize,
    },
    ReferencesResolve {
        items: Vec<Location>,
    },
    InstallVolt {
        volt: VoltInfo,
    },
    RemoveVolt {
        volt: VoltMetadata,
    },
    ReloadVolt {
        volt: VoltMetadata,
    },
    DisableVolt {
        volt: VoltInfo,
    },
    EnableVolt {
        volt: VoltInfo,
    },
    Initialize {
        workspace:             Option<PathBuf>,
        disabled_volts:        Vec<VoltID>,
        /// Paths to extra plugins that should be loaded
        extra_plugin_paths:    Vec<PathBuf>,
        plugin_configurations: HashMap<String, HashMap<String, serde_json::Value>>,
        window_id:             usize,
        tab_id:                usize,
    },
    FindFileFromLog {
        log: String,
    },
    FindLogModulesFromPath {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "method", content = "params")]
pub enum ProxyNotification {
    OpenFileChanged {
        path: PathBuf,
    },
    OpenPaths {
        paths: Vec<PathObject>,
    },
    Shutdown {},
    Update {
        path:  PathBuf,
        delta: RopeDelta,
        rev:   u64,
    },
    UpdatePluginConfigs {
        configs: HashMap<String, HashMap<String, serde_json::Value>>,
    },
    NewTerminal {
        term_id: TermId,
        raw_id:  u64,
        profile: TerminalProfile,
    },
    GitCommit {
        message: String,
        diffs:   Vec<FileDiff>,
    },
    GitCheckout {
        reference: String,
    },
    GitDiscardFilesChanges {
        files: Vec<PathBuf>,
    },
    GitDiscardWorkspaceChanges {},
    GitInit {},
    LspCancel {
        id: i32,
    },
    TerminalWrite {
        term_id: TermId,
        raw_id:  u64,
        content: String,
    },
    TerminalResize {
        term_id: TermId,
        width:   usize,
        height:  usize,
    },
    TerminalClose {
        term_id: TermId,
        raw_id:  u64,
    },
    DapStart {
        config:      RunDebugConfig,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
    },
    DapProcessId {
        dap_id:     DapId,
        process_id: Option<u32>,
        term_id:    TermId,
    },
    DapContinue {
        dap_id:    DapId,
        thread_id: ThreadId,
    },
    DapStepOver {
        dap_id:    DapId,
        thread_id: ThreadId,
    },
    DapStepInto {
        dap_id:    DapId,
        thread_id: ThreadId,
    },
    DapStepOut {
        dap_id:    DapId,
        thread_id: ThreadId,
    },
    DapPause {
        dap_id:    DapId,
        thread_id: ThreadId,
    },
    DapStop {
        dap_id: DapId,
    },
    DapDisconnect {
        dap_id: DapId,
    },
    DapRestart {
        config:      RunDebugConfig,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
    },
    DapSetBreakpoints {
        dap_id:      DapId,
        path:        PathBuf,
        breakpoints: Vec<SourceBreakpoint>,
    },
    RustBuild {
        rev:       u64,
        command:   String,
        arguments: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "method", content = "params")]
pub enum ProxyResponse {
    GitGetRemoteFileUrl {
        file_url: String,
    },
    NewBufferResponse {
        rs: RpcResult<(String, bool, Option<PathBuf>)>,
        // content:   String,
        // read_only: bool,
    },
    BufferHeadResponse {
        version: String,
        content: String,
    },
    ReadDirResponse {
        items: Vec<FileNodeItem>,
    },
    CompletionResolveResponse {
        item: Box<CompletionItem>,
    },
    CodeActionResolveResponse {
        item: Box<CodeAction>,
    },
    HoverResponse {
        request_id: usize,
        hover:      Hover,
    },
    GetDefinitionResponse {
        request_id: usize,
        definition: GotoDefinitionResponse,
    },
    ShowCallHierarchyResponse {
        items: Option<Vec<CallHierarchyItem>>,
    },
    DocumentHighlightResponse {
        items: Option<Vec<DocumentHighlight>>,
    },
    CallHierarchyIncomingResponse {
        items: Option<Vec<CallHierarchyIncomingCall>>,
    },
    GetTypeDefinition {
        request_id: usize,
        definition: GotoTypeDefinitionResponse,
    },
    GetReferencesResponse {
        references: Vec<Location>,
    },
    GetCodeActionsResponse {
        plugin_id: PluginId,
        resp:      CodeActionResponse,
    },
    LspFoldingRangeResponse {
        plugin_id: PluginId,
        resp:      Option<Vec<FoldingRange>>,
    },
    GetCodeLensResponse {
        plugin_id: PluginId,
        resp:      Option<Vec<CodeLens>>,
    },
    GetCodeLensResolveResponse {
        plugin_id: PluginId,
        resp:      CodeLens,
    },
    GotoImplementationResponse {
        plugin_id: PluginId,
        resp:      Option<GotoImplementationResponse>,
    },
    OnEnterResponse {
        plugin_id: PluginId,
        resp:      Option<Vec<crate::SnippetTextEdit>>,
    },
    GetFilesResponse {
        items: Vec<PathBuf>,
    },
    GetDocumentFormatting {
        edits: Vec<TextEdit>,
    },
    OnTypeFormatting {
        edits: Option<Vec<TextEdit>>,
    },
    GetDocumentSymbols {
        resp: DocumentSymbolResponse,
    },
    GetWorkspaceSymbols {
        symbols: Vec<SymbolInformation>,
    },
    GetSelectionRange {
        ranges: Vec<SelectionRange>,
    },
    GetInlayHints {
        hints: Vec<InlayHint>,
    },
    GetInlineCompletions {
        completions: InlineCompletionResponse,
    },
    GetSemanticTokens {
        styles:    SemanticStyles,
        result_id: Option<String>,
    },
    PrepareRename {
        resp: PrepareRenameResponse,
    },
    Rename {
        edit: WorkspaceEdit,
    },
    GetOpenFilesContentResponse {
        items: Vec<TextDocumentItem>,
    },
    GlobalSearchResponse {
        matches: IndexMap<PathBuf, Vec<SearchMatch>>,
    },
    DapVariableResponse {
        varialbes: Vec<dap_types::Variable>,
    },
    DapGetScopesResponse {
        scopes: Vec<(dap_types::Scope, Vec<dap_types::Variable>)>,
    },
    CreatePathResponse {
        path: PathBuf,
    },
    Success {},
    SaveResponse {},
    ReferencesResolveResponse {
        items: Vec<FileLine>,
    },
    FindFileFromLogResponse {
        rs: RpcResult<FileAndLine>,
    },
    FindLogModulesFromPathResponse {
        rs: RpcResult<String>,
    },
    GetAbsolutePathResponse {
        path: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAndLine {
    pub file: PathBuf,
    pub line: u32,
}

pub type ProxyMessage = RpcMessage<ProxyRequest, ProxyNotification, ProxyResponse>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadDirResponse {
    pub items: HashMap<PathBuf, FileNodeItem>,
}

pub trait ProxyCallback:
    Send + FnOnce((u64, Result<ProxyResponse, RpcError>)) {
}

impl<F: Send + FnOnce((u64, Result<ProxyResponse, RpcError>))> ProxyCallback for F {}

enum ResponseHandler {
    Callback(Box<dyn ProxyCallback>),
    Chan(Sender<(u64, Result<ProxyResponse, RpcError>)>),
}

impl ResponseHandler {
    fn invoke(self, id: u64, result: Result<ProxyResponse, RpcError>) {
        match self {
            ResponseHandler::Callback(f) => f((id, result)),
            ResponseHandler::Chan(tx) => {
                if let Err(err) = tx.send((id, result)) {
                    log::error!("{:?}", err);
                }
            },
        }
    }
}

pub trait ProxyHandler {
    fn handle_notification(
        &mut self,
        rpc: ProxyNotification,
    ) -> impl std::future::Future<Output = ()>;
    fn handle_request(
        &mut self,
        id: RequestId,
        rpc: ProxyRequest,
        workspace: &mut WorkspaceContext,
    ) -> impl std::future::Future<Output = ()>;
}

#[derive(Clone)]
pub struct ProxyRpcHandler {
    tx:      Sender<ProxyRpc>,
    rx:      Receiver<ProxyRpc>,
    id:      Arc<AtomicU64>,
    pending: Arc<Mutex<HashMap<u64, ResponseHandler>>>,
}

impl ProxyRpcHandler {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx,
            rx,
            id: Arc::new(AtomicU64::new(0)),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn rx(&self) -> &Receiver<ProxyRpc> {
        &self.rx
    }

    pub async fn mainloop<H>(
        &self,
        handler: &mut H,
        workspace: &mut WorkspaceContext,
    ) where
        H: ProxyHandler, {
        use ProxyRpc::*;
        for msg in &self.rx {
            match msg {
                Request(id, request) => {
                    // info!("Request: {request:?}");
                    handler.handle_request(id, request, workspace).await;
                },
                Notification(notification) => {
                    // info!("Notification: {notification:?}");
                    handler.handle_notification(notification).await;
                },
                Shutdown => {
                    return;
                },
            }
        }
    }

    fn request_common(&self, request: ProxyRequest, rh: ResponseHandler) -> u64 {
        let id = self.id.fetch_add(1, Ordering::Relaxed);

        self.pending.lock().insert(id, rh);

        if let Err(err) = self.tx.send(ProxyRpc::Request(id, request)) {
            log::error!("{:?}", err);
        }
        id
    }

    fn request(&self, request: ProxyRequest) -> Result<ProxyResponse, RpcError> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.request_common(request, ResponseHandler::Chan(tx));
        rx.recv()
            .map_err(|_| RpcError {
                code:    0,
                message: "io error".to_string(),
            })?
            .1
    }

    pub fn request_async(
        &self,
        request: impl Into<ProxyRequest>,
        f: impl ProxyCallback + 'static,
    ) -> u64 {
        self.request_common(request.into(), ResponseHandler::Callback(Box::new(f)))
    }

    pub fn handle_response(
        &self,
        id: RequestId,
        result: Result<ProxyResponse, RpcError>,
    ) {
        let handler = { self.pending.lock().remove(&id) };
        if let Some(handler) = handler {
            handler.invoke(id, result);
        }
    }

    pub fn notification(&self, notification: ProxyNotification) {
        if let Err(err) = self.tx.send(ProxyRpc::Notification(notification)) {
            log::error!("{:?}", err);
        }
    }

    pub fn lsp_cancel(&self, id: u64) {
        log::info!("lsp_cancel {}", id);
        self.notification(ProxyNotification::LspCancel { id: id as i32 });
    }

    pub fn git_init(&self) {
        self.notification(ProxyNotification::GitInit {});
    }

    pub fn git_commit(&self, message: String, diffs: Vec<FileDiff>) {
        self.notification(ProxyNotification::GitCommit { message, diffs });
    }

    pub fn git_checkout(&self, reference: String) {
        self.notification(ProxyNotification::GitCheckout { reference });
    }

    pub fn install_volt(&self, volt: VoltInfo) {
        self.request_async(ProxyRequest::InstallVolt { volt }, |_| {});
    }

    pub fn reload_volt(&self, volt: VoltMetadata) {
        self.request_async(ProxyRequest::ReloadVolt { volt }, |_| {});
    }

    pub fn remove_volt(&self, volt: VoltMetadata) {
        self.request_async(ProxyRequest::RemoveVolt { volt }, |_| {});
    }

    pub fn disable_volt(&self, volt: VoltInfo) {
        self.request_async(ProxyRequest::DisableVolt { volt }, |_| {});
    }

    pub fn enable_volt(&self, volt: VoltInfo) {
        self.request_async(ProxyRequest::EnableVolt { volt }, |_| {});
    }

    pub fn shutdown(&self) {
        self.notification(ProxyNotification::Shutdown {});
        if let Err(err) = self.tx.send(ProxyRpc::Shutdown) {
            log::error!("{:?}", err);
        }
    }

    pub fn initialize(
        &self,
        workspace: Option<PathBuf>,
        disabled_volts: Vec<VoltID>,
        extra_plugin_paths: Vec<PathBuf>,
        plugin_configurations: HashMap<String, HashMap<String, serde_json::Value>>,
        window_id: usize,
        tab_id: usize,
    ) {
        self.request_async(
            ProxyRequest::Initialize {
                workspace,
                disabled_volts,
                extra_plugin_paths,
                plugin_configurations,
                window_id,
                tab_id,
            },
            |_| {},
        );
    }

    pub fn completion(
        &self,
        request_id: usize,
        path: PathBuf,
        input: String,
        position: Position,
    ) {
        self.request_async(
            ProxyLspRequest::Completion {
                request_id,
                path,
                input,
                position,
            },
            |_| {},
        );
    }

    pub fn signature_help(
        &self,
        request_id: usize,
        path: PathBuf,
        position: Position,
    ) {
        self.request_async(
            ProxyLspRequest::SignatureHelp {
                request_id,
                path,
                position,
            },
            |_| {},
        );
    }

    pub fn new_terminal(
        &self,
        term_id: TermId,
        raw_id: u64,
        profile: TerminalProfile,
    ) {
        self.notification(ProxyNotification::NewTerminal {
            term_id,
            raw_id,
            profile,
        })
    }

    pub fn terminal_close(&self, term_id: TermId, raw_id: u64) {
        self.notification(ProxyNotification::TerminalClose { term_id, raw_id });
    }

    pub fn terminal_resize(&self, term_id: TermId, width: usize, height: usize) {
        self.notification(ProxyNotification::TerminalResize {
            term_id,
            width,
            height,
        });
    }

    pub fn terminal_write(&self, term_id: TermId, raw_id: u64, content: String) {
        self.notification(ProxyNotification::TerminalWrite {
            term_id,
            raw_id,
            content,
        });
    }

    pub fn new_buffer(
        &self,
        buffer_id: BufferId,
        path: PathBuf,
        check_if_exists: bool,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyRequest::NewBuffer {
                buffer_id,
                path,
                check_if_exists,
            },
            f,
        );
    }

    pub fn get_buffer_head(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::BufferHead { path }, f);
    }

    pub fn create_file(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::CreateFile { path }, f);
    }

    pub fn find_file_from_log(&self, log: String, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::FindFileFromLog { log }, f);
    }

    pub fn find_log_modules(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::FindLogModulesFromPath { path }, f);
    }

    pub fn create_directory(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::CreateDirectory { path }, f);
    }

    pub fn trash_path(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::TrashPath { path }, f);
    }

    pub fn duplicate_path(
        &self,
        existing_path: PathBuf,
        new_path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyRequest::DuplicatePath {
                existing_path,
                new_path,
            },
            f,
        );
    }

    pub fn rename_path(
        &self,
        from: PathBuf,
        to: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::RenamePath { from, to }, f);
    }

    pub fn test_create_at_path(
        &self,
        path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::TestCreateAtPath { path }, f);
    }

    pub fn save_buffer_as(
        &self,
        buffer_id: BufferId,
        path: PathBuf,
        rev: u64,
        content: String,
        create_parents: bool,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyRequest::SaveBufferAs {
                buffer_id,
                path,
                rev,
                content,
                create_parents,
            },
            f,
        );
    }

    pub fn global_search(
        &self,
        pattern: String,
        case_sensitive: bool,
        whole_word: bool,
        is_regex: bool,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyRequest::GlobalSearch {
                pattern,
                case_sensitive,
                whole_word,
                is_regex,
            },
            f,
        );
    }

    pub fn save(
        &self,
        rev: u64,
        path: PathBuf,
        create_parents: bool,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyRequest::Save {
                rev,
                path,
                create_parents,
            },
            f,
        );
    }

    pub fn get_absolute_path(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::GetAbsolutePath { path }, f);
    }

    pub fn get_files(&self, f: impl ProxyCallback + 'static) {
        self.request_async(
            ProxyRequest::GetFiles {
                path: "path".into(),
            },
            f,
        );
    }

    pub fn get_open_files_content(&self) -> Result<ProxyResponse, RpcError> {
        self.request(ProxyRequest::GetOpenFilesContent {})
    }

    pub fn read_dir(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyRequest::ReadDir { path }, f);
    }

    pub fn completion_resolve(
        &self,
        plugin_id: PluginId,
        completion_item: CompletionItem,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::CompletionResolve {
                plugin_id,
                completion_item: Box::new(completion_item),
            },
            f,
        );
    }

    pub fn code_action_resolve(
        &self,
        action_item: CodeAction,
        plugin_id: PluginId,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::CodeActionResolve {
                action_item: Box::new(action_item),
                plugin_id,
            },
            f,
        );
    }

    pub fn get_hover(
        &self,
        request_id: usize,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetHover {
                request_id,
                path,
                position,
            },
            f,
        );
    }

    pub fn get_definition(
        &self,
        request_id: usize,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetDefinition {
                request_id,
                path,
                position,
            },
            f,
        );
    }

    pub fn show_call_hierarchy(
        &self,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::ShowCallHierarchy { path, position }, f);
    }

    pub fn document_highlight(
        &self,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) -> u64 {
        self.request_async(ProxyLspRequest::DocumentHighlight { path, position }, f)
    }

    pub fn call_hierarchy_incoming(
        &self,
        path: PathBuf,
        call_hierarchy_item: CallHierarchyItem,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::CallHierarchyIncoming {
                path,
                call_hierarchy_item,
            },
            f,
        );
    }

    pub fn get_type_definition(
        &self,
        request_id: usize,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetTypeDefinition {
                request_id,
                path,
                position,
            },
            f,
        );
    }

    pub fn get_lsp_folding_range(
        &self,
        path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::LspFoldingRange { path }, f);
    }

    pub fn get_references(
        &self,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::GetReferences { path, position }, f);
    }

    pub fn references_resolve(
        &self,
        items: Vec<Location>,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::ReferencesResolve { items }, f);
    }

    pub fn go_to_implementation(
        &self,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GotoImplementation { path, position },
            f,
        );
    }

    pub fn get_code_actions(
        &self,
        path: PathBuf,
        position: Position,
        diagnostics: Vec<Diagnostic>,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetCodeActions {
                path,
                position,
                diagnostics,
            },
            f,
        );
    }

    pub fn get_code_lens(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyLspRequest::GetCodeLens { path }, f);
    }

    pub fn get_code_lens_resolve(
        &self,
        code_lens: CodeLens,
        path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetCodeLensResolve { code_lens, path },
            f,
        );
    }

    pub fn get_document_formatting(
        &self,
        path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::GetDocumentFormatting { path }, f);
    }

    pub fn on_enter(
        &self,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::OnEnter { path, position }, f);
    }

    pub fn on_type_formatting(
        &self,
        path: PathBuf,
        position: Position,
        ch: String,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::OnTypeFormatting { path, position, ch },
            f,
        );
    }

    pub fn get_semantic_tokens(
        &self,
        path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::GetSemanticTokens { path }, f);
    }

    pub fn get_semantic_tokens_delta(
        &self,
        path: PathBuf,
        previous_result_id: String,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetSemanticTokensDelta {
                path,
                previous_result_id,
            },
            f,
        );
    }

    pub fn get_document_symbols(
        &self,
        path: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyLspRequest::GetDocumentSymbols { path }, f);
    }

    pub fn get_workspace_symbols(
        &self,
        query: String,
        f: impl ProxyCallback + 'static,
    ) -> u64 {
        self.request_async(ProxyRequest::GetWorkspaceSymbols { query }, f)
    }

    pub fn prepare_rename(
        &self,
        path: PathBuf,
        position: Position,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::PrepareRename { path, position }, f);
    }

    pub fn git_get_remote_file_url(
        &self,
        file: PathBuf,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::GitGetRemoteFileUrl { file }, f);
    }

    pub fn rename(
        &self,
        path: PathBuf,
        position: Position,
        new_name: String,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyRequest::Rename {
                path,
                position,
                new_name,
            },
            f,
        );
    }

    pub fn get_inlay_hints(&self, path: PathBuf, f: impl ProxyCallback + 'static) {
        self.request_async(ProxyLspRequest::GetInlayHints { path }, f);
    }

    pub fn get_inline_completions(
        &self,
        path: PathBuf,
        position: Position,
        trigger_kind: InlineCompletionTriggerKind,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetInlineCompletions {
                path,
                position,
                trigger_kind,
            },
            f,
        );
    }

    pub fn update(&self, path: PathBuf, delta: RopeDelta, rev: u64) {
        self.notification(ProxyNotification::Update { path, delta, rev });
    }

    pub fn update_plugin_configs(
        &self,
        configs: HashMap<String, HashMap<String, serde_json::Value>>,
    ) {
        self.notification(ProxyNotification::UpdatePluginConfigs { configs });
    }

    pub fn git_discard_files_changes(&self, files: Vec<PathBuf>) {
        self.notification(ProxyNotification::GitDiscardFilesChanges { files });
    }

    pub fn git_discard_workspace_changes(&self) {
        self.notification(ProxyNotification::GitDiscardWorkspaceChanges {});
    }

    pub fn get_selection_range(
        &self,
        path: PathBuf,
        positions: Vec<Position>,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(
            ProxyLspRequest::GetSelectionRange { path, positions },
            f,
        );
    }

    pub fn dap_start(
        &self,
        config: RunDebugConfig,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
    ) {
        self.notification(ProxyNotification::DapStart {
            config,
            breakpoints,
        })
    }

    pub fn dap_process_id(
        &self,
        dap_id: DapId,
        process_id: Option<u32>,
        term_id: TermId,
    ) {
        self.notification(ProxyNotification::DapProcessId {
            dap_id,
            process_id,
            term_id,
        })
    }

    pub fn dap_restart(
        &self,
        config: RunDebugConfig,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
    ) {
        self.notification(ProxyNotification::DapRestart {
            config,
            breakpoints,
        })
    }

    pub fn dap_continue(&self, dap_id: DapId, thread_id: ThreadId) {
        self.notification(ProxyNotification::DapContinue { dap_id, thread_id })
    }

    pub fn dap_step_over(&self, dap_id: DapId, thread_id: ThreadId) {
        self.notification(ProxyNotification::DapStepOver { dap_id, thread_id })
    }

    pub fn dap_step_into(&self, dap_id: DapId, thread_id: ThreadId) {
        self.notification(ProxyNotification::DapStepInto { dap_id, thread_id })
    }

    pub fn dap_step_out(&self, dap_id: DapId, thread_id: ThreadId) {
        self.notification(ProxyNotification::DapStepOut { dap_id, thread_id })
    }

    pub fn dap_pause(&self, dap_id: DapId, thread_id: ThreadId) {
        self.notification(ProxyNotification::DapPause { dap_id, thread_id })
    }

    pub fn dap_stop(&self, dap_id: DapId) {
        self.notification(ProxyNotification::DapStop { dap_id })
    }

    pub fn dap_disconnect(&self, dap_id: DapId) {
        self.notification(ProxyNotification::DapDisconnect { dap_id })
    }

    pub fn dap_set_breakpoints(
        &self,
        dap_id: DapId,
        path: PathBuf,
        breakpoints: Vec<SourceBreakpoint>,
    ) {
        self.notification(ProxyNotification::DapSetBreakpoints {
            dap_id,
            path,
            breakpoints,
        })
    }

    pub fn dap_variable(
        &self,
        dap_id: DapId,
        reference: usize,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::DapVariable { dap_id, reference }, f);
    }

    pub fn dap_get_scopes(
        &self,
        dap_id: DapId,
        frame_id: usize,
        f: impl ProxyCallback + 'static,
    ) {
        self.request_async(ProxyRequest::DapGetScopes { dap_id, frame_id }, f);
    }
}

impl Default for ProxyRpcHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct WorkspaceContext {
    pub cargo_context: Option<CargoContext>,
}

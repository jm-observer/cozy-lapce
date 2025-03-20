use std::path::PathBuf;

use doc::lines::line_ending::LineEnding;
use lapce_core::{
    debug::RunDebugMode,
    workspace::{LapceWorkspace, SshHost, WslHost},
};
use lapce_rpc::dap_types::RunDebugConfig;
use lsp_types::{Range, SymbolKind};

use crate::{
    command::{LapceCommand, LapceWorkbenchCommand},
    editor::location::EditorLocation,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    pub content:     PaletteItemContent,
    pub filter_text: String,
    pub score:       u32,
    pub indices:     Vec<usize>,
    pub run_id:      u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteItemContent {
    PaletteHelp {
        cmd: LapceWorkbenchCommand,
    },
    File {
        path:      PathBuf,
        full_path: PathBuf,
    },
    Line {
        line:    usize,
        content: String,
    },
    Command {
        cmd: LapceCommand,
    },
    Workspace {
        workspace: LapceWorkspace,
    },
    Reference {
        path:     PathBuf,
        location: EditorLocation,
    },
    DocumentSymbol {
        kind:           SymbolKind,
        name:           String,
        range:          Range,
        container_name: Option<String>,
    },
    WorkspaceSymbol {
        kind:           SymbolKind,
        name:           String,
        container_name: Option<String>,
        location:       EditorLocation,
    },
    SshHost {
        host: SshHost,
    },
    #[cfg(windows)]
    WslHost {
        host: WslHost,
    },
    RunAndDebug {
        mode:   RunDebugMode,
        config: RunDebugConfig,
    },
    ColorTheme {
        name: String,
    },
    IconTheme {
        name: String,
    },
    Language {
        name: String,
    },
    LineEnding {
        kind: LineEnding,
    },
    SCMReference {
        name: String,
    },
    TerminalProfile {
        name:    String,
        profile: lapce_rpc::terminal::TerminalProfile,
    },
}

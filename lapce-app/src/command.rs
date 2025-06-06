use std::{path::PathBuf, rc::Rc, sync::Arc};

use doc::lines::{command::*, editor_command::Command};
use floem::{ViewId, keyboard::Modifiers, menu::MenuItem, peniko::kurbo::Vec2};
use indexmap::IndexMap;
use lapce_core::{
    debug::RunDebugMode,
    id::*,
    main_split::{SplitDirection, SplitMoveDirection, TabCloseKind},
    workspace::LapceWorkspace,
};
use lapce_rpc::{
    dap_types::{DapId, RunDebugConfig},
    plugin::{PluginId, VoltID},
    proxy::ProxyStatus,
    terminal::TerminalProfile,
};
use log::LevelFilter;
use lsp_types::{CodeActionOrCommand, Position, WorkspaceEdit};
use serde_json::Value;
use strum::{EnumMessage, IntoEnumIterator};
use strum_macros::{Display, EnumIter, EnumMessage, EnumString, IntoStaticStr};

use crate::{
    alert::AlertButton,
    doc::Doc,
    editor::{
        DocSignal,
        location::{EditorLocation, EditorMaybeRelativeLocation},
    },
    editor_tab::EditorTabChildId,
    listener::Listener,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LapceCommand {
    pub kind: CommandKind,
    pub data: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Workbench(LapceWorkbenchCommand),
    Edit(EditCommand),
    Move(MoveCommand),
    Scroll(ScrollCommand),
    Focus(FocusCommand),
    MotionMode(MotionModeCommand),
    MultiSelection(MultiSelectionCommand),
    Other(OtherCommand),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OtherCommand {
    RightMenuRunCodeLen { id: Id, title: String },
}

impl OtherCommand {
    pub fn desc(&self) -> Option<&'static str> {
        match &self {
            Self::RightMenuRunCodeLen { .. } => Some("RightMenuRunCodeLen"),
        }
    }

    pub fn str(&self) -> &'static str {
        match &self {
            Self::RightMenuRunCodeLen { .. } => "RightMenuRunCodeLen",
        }
    }

    pub fn title(&self) -> String {
        match self {
            OtherCommand::RightMenuRunCodeLen { title, .. } => title.clone(),
        }
    }
}

impl CommandKind {
    pub fn to_menu(self, lapce_command: Listener<LapceCommand>) -> MenuItem {
        MenuItem::new(self.title()).action(move || {
            lapce_command.send(LapceCommand {
                kind: self.clone(),
                data: None,
            })
        })
    }

    pub fn desc(&self) -> Option<&'static str> {
        match &self {
            CommandKind::Workbench(cmd) => cmd.get_detailed_message(),
            CommandKind::Edit(cmd) => cmd.get_detailed_message(),
            CommandKind::Move(cmd) => cmd.get_detailed_message(),
            CommandKind::Scroll(cmd) => cmd.get_detailed_message(),
            CommandKind::Focus(cmd) => cmd.get_detailed_message(),
            CommandKind::MotionMode(cmd) => cmd.get_detailed_message(),
            CommandKind::MultiSelection(cmd) => cmd.get_detailed_message(),
            CommandKind::Other(cmd) => cmd.desc(),
        }
    }

    pub fn title(&self) -> String {
        if let Self::Other(cmd) = self {
            cmd.title()
        } else {
            self.desc().unwrap_or_else(|| self.str()).to_owned()
        }
    }

    pub fn str(&self) -> &'static str {
        match &self {
            CommandKind::Workbench(cmd) => cmd.into(),
            CommandKind::Edit(cmd) => cmd.into(),
            CommandKind::Move(cmd) => cmd.into(),
            CommandKind::Scroll(cmd) => cmd.into(),
            CommandKind::Focus(cmd) => cmd.into(),
            CommandKind::MotionMode(cmd) => cmd.into(),
            CommandKind::MultiSelection(cmd) => cmd.into(),
            CommandKind::Other(cmd) => cmd.str(),
        }
    }
}
impl From<Command> for CommandKind {
    fn from(cmd: Command) -> Self {
        use Command::*;
        match cmd {
            Edit(edit) => CommandKind::Edit(edit),
            Move(movement) => CommandKind::Move(movement),
            Scroll(scroll) => CommandKind::Scroll(scroll),
            MotionMode(motion_mode) => CommandKind::MotionMode(motion_mode),
            MultiSelection(multi_selection) => {
                CommandKind::MultiSelection(multi_selection)
            },
        }
    }
}

pub fn lapce_internal_commands() -> IndexMap<String, LapceCommand> {
    let mut commands = IndexMap::new();

    for c in LapceWorkbenchCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::Workbench(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    for c in EditCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::Edit(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    for c in MoveCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::Move(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    for c in ScrollCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::Scroll(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    for c in FocusCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::Focus(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    for c in MotionModeCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::MotionMode(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    for c in MultiSelectionCommand::iter() {
        let command = LapceCommand {
            kind: CommandKind::MultiSelection(c.clone()),
            data: None,
        };
        commands.insert(c.to_string(), command);
    }

    commands
}

#[derive(
    Display,
    EnumString,
    EnumIter,
    Clone,
    PartialEq,
    Eq,
    Debug,
    EnumMessage,
    IntoStaticStr,
)]
pub enum LapceWorkbenchCommand {
    #[strum(serialize = "enable_modal_editing")]
    #[strum(message = "Enable Modal Editing")]
    EnableModal,

    #[strum(serialize = "disable_modal_editing")]
    #[strum(message = "Disable Modal Editing")]
    DisableModal,

    #[strum(serialize = "open_folder")]
    #[strum(message = "Open Folder")]
    OpenFolder,

    #[strum(serialize = "close_folder")]
    #[strum(message = "Close Folder")]
    CloseFolder,

    #[strum(serialize = "open_file")]
    #[strum(message = "Open File")]
    OpenFile,

    #[strum(serialize = "show_call_hierarchy")]
    #[strum(message = "Show Call Hierarchy")]
    ShowCallHierarchy,

    #[strum(serialize = "find_references")]
    #[strum(message = "Find References")]
    FindReferences,

    #[strum(serialize = "go_to_implementation")]
    #[strum(message = "Go to Implementation")]
    GoToImplementation,

    #[strum(serialize = "reveal_in_panel")]
    #[strum(message = "Reveal in Panel")]
    RevealInPanel,

    #[strum(serialize = "source_control_open_active_file_remote_url")]
    #[strum(message = "Source Control: Open Remote File Url")]
    SourceControlOpenActiveFileRemoteUrl,

    #[cfg(not(target_os = "macos"))]
    #[strum(serialize = "reveal_in_file_explorer")]
    #[strum(message = "Reveal in System File Explorer")]
    RevealInFileExplorer,

    #[cfg(target_os = "macos")]
    #[strum(serialize = "reveal_in_file_explorer")]
    #[strum(message = "Reveal in Finder")]
    RevealInFileExplorer,

    #[strum(serialize = "run_in_terminal")]
    #[strum(message = "Run in Terminal")]
    RunInTerminal,

    #[strum(serialize = "reveal_active_file_in_file_explorer")]
    #[strum(message = "Reveal Active File in File Explorer")]
    RevealActiveFileInFileExplorer,

    #[strum(serialize = "open_ui_inspector")]
    #[strum(message = "Open Internal UI Inspector")]
    OpenUIInspector,

    #[strum(serialize = "show_env")]
    #[strum(message = "Show Environment")]
    ShowEnvironment,

    #[strum(serialize = "change_color_theme")]
    #[strum(message = "Change Color Theme")]
    ChangeColorTheme,

    #[strum(serialize = "change_icon_theme")]
    #[strum(message = "Change Icon Theme")]
    ChangeIconTheme,

    #[strum(serialize = "open_settings")]
    #[strum(message = "Open Settings")]
    OpenSettings,

    #[strum(serialize = "open_settings_file")]
    #[strum(message = "Open Settings File")]
    OpenSettingsFile,

    #[strum(serialize = "open_run_and_debug_file")]
    #[strum(message = "Open Run And Debug File")]
    OpenRunAndDebugFile,

    #[strum(serialize = "open_settings_directory")]
    #[strum(message = "Open Settings Directory")]
    OpenSettingsDirectory,

    #[strum(serialize = "open_theme_color_settings")]
    #[strum(message = "Open Theme Color Settings")]
    OpenThemeColorSettings,

    #[strum(serialize = "open_keyboard_shortcuts")]
    #[strum(message = "Open Keyboard Shortcuts")]
    OpenKeyboardShortcuts,

    #[strum(serialize = "open_keyboard_shortcuts_file")]
    #[strum(message = "Open Keyboard Shortcuts File")]
    OpenKeyboardShortcutsFile,

    #[strum(serialize = "open_log_file")]
    #[strum(message = "Open Log File")]
    OpenLogFile,

    #[strum(serialize = "open_logs_directory")]
    #[strum(message = "Open Logs Directory")]
    OpenLogsDirectory,

    #[strum(serialize = "open_proxy_directory")]
    #[strum(message = "Open Proxy Directory")]
    OpenProxyDirectory,

    #[strum(serialize = "open_themes_directory")]
    #[strum(message = "Open Themes Directory")]
    OpenThemesDirectory,

    #[strum(serialize = "open_plugins_directory")]
    #[strum(message = "Open Plugins Directory")]
    OpenPluginsDirectory,

    #[strum(serialize = "open_grammars_directory")]
    #[strum(message = "Open Grammars Directory")]
    OpenGrammarsDirectory,

    #[strum(serialize = "open_queries_directory")]
    #[strum(message = "Open Queries Directory")]
    OpenQueriesDirectory,

    #[strum(serialize = "zoom_in")]
    #[strum(message = "Zoom In")]
    ZoomIn,

    #[strum(serialize = "zoom_out")]
    #[strum(message = "Zoom Out")]
    ZoomOut,

    #[strum(serialize = "zoom_reset")]
    #[strum(message = "Reset Zoom")]
    ZoomReset,

    #[strum(serialize = "close_window_tab")]
    #[strum(message = "Close Current Window Tab")]
    CloseWindowTab,

    #[strum(serialize = "new_window_tab")]
    #[strum(message = "Create New Window Tab")]
    NewWindowTab,

    #[strum(serialize = "new_terminal_tab")]
    #[strum(message = "Create New Terminal Tab")]
    NewTerminalTab,

    #[strum(serialize = "close_terminal_tab")]
    #[strum(message = "Close Terminal Tab")]
    CloseTerminalTab,

    #[strum(serialize = "next_terminal_tab")]
    #[strum(message = "Next Terminal Tab")]
    NextTerminalTab,

    #[strum(serialize = "previous_terminal_tab")]
    #[strum(message = "Previous Terminal Tab")]
    PreviousTerminalTab,

    #[strum(serialize = "next_window_tab")]
    #[strum(message = "Go To Next Window Tab")]
    NextWindowTab,

    #[strum(serialize = "previous_window_tab")]
    #[strum(message = "Go To Previous Window Tab")]
    PreviousWindowTab,

    #[strum(serialize = "reload_window")]
    #[strum(message = "Reload Window")]
    ReloadWindow,

    #[strum(message = "New Window")]
    #[strum(serialize = "new_window")]
    NewWindow,

    #[strum(message = "Close Window")]
    #[strum(serialize = "close_window")]
    CloseWindow,

    #[strum(message = "New File")]
    #[strum(serialize = "new_file")]
    NewFile,

    #[strum(serialize = "connect_ssh_host")]
    #[strum(message = "Connect to SSH Host")]
    ConnectSshHost,

    #[cfg(windows)]
    #[strum(serialize = "connect_wsl_host")]
    #[strum(message = "Connect to WSL Host")]
    ConnectWslHost,

    #[strum(serialize = "disconnect_remote")]
    #[strum(message = "Disconnect From Remote")]
    DisconnectRemote,

    #[strum(message = "Go To Line")]
    #[strum(serialize = "palette.line")]
    PaletteLine,

    #[strum(serialize = "palette")]
    #[strum(message = "Palette(Go to File)")]
    Palette,

    #[strum(message = "Go To Symbol In File")]
    #[strum(serialize = "palette.symbol")]
    PaletteSymbol,

    #[strum(message = "Go To Symbol In Workspace")]
    #[strum(serialize = "palette.workspace_symbol")]
    PaletteWorkspaceSymbol,

    #[strum(message = "Command Palette")]
    #[strum(serialize = "palette.command")]
    PaletteCommand,

    #[strum(message = "Open Recent Workspace")]
    #[strum(serialize = "palette.workspace")]
    PaletteWorkspace,

    #[strum(message = "Run and Debug")]
    #[strum(serialize = "palette.run_and_debug")]
    PaletteRunAndDebug,

    #[strum(message = "Source Control: Checkout")]
    #[strum(serialize = "palette.scm_references")]
    PaletteSCMReferences,

    #[strum(message = "List Palette Types")]
    #[strum(serialize = "palette.palette_help")]
    PaletteHelp,

    #[strum(message = "List Palette Types and Files")]
    #[strum(serialize = "palette.palette_help_and_file")]
    PaletteHelpAndFile,

    #[strum(message = "Run and Debug Restart Current Running")]
    #[strum(serialize = "palette.run_and_debug_restart")]
    RunAndDebugRestart,

    #[strum(message = "Run and Debug Stop Current Running")]
    #[strum(serialize = "palette.run_and_debug_stop")]
    RunAndDebugStop,

    #[strum(serialize = "source_control.checkout_reference")]
    CheckoutReference,

    #[strum(serialize = "toggle_maximized_panel")]
    ToggleMaximizedPanel,

    #[strum(serialize = "hide_panel")]
    HidePanel,

    #[strum(serialize = "show_panel")]
    ShowPanel,

    /// Toggles the panel passed in parameter.
    #[strum(serialize = "toggle_panel_focus")]
    TogglePanelFocus,

    /// Toggles the panel passed in parameter.
    #[strum(serialize = "toggle_panel_visual")]
    TogglePanelVisual,

    #[strum(message = "Toggle Left Panel")]
    #[strum(serialize = "toggle_panel_left_visual")]
    TogglePanelLeftVisual,

    #[strum(message = "Toggle Right Panel")]
    #[strum(serialize = "toggle_panel_right_visual")]
    TogglePanelRightVisual,

    #[strum(message = "Toggle Bottom Panel")]
    #[strum(serialize = "toggle_panel_bottom_visual")]
    TogglePanelBottomVisual,

    // Focus toggle commands
    #[strum(message = "Toggle Terminal Focus")]
    #[strum(serialize = "toggle_terminal_focus")]
    ToggleTerminalFocus,

    #[strum(serialize = "toggle_source_control_focus")]
    ToggleSourceControlFocus,

    #[strum(message = "Toggle Plugin Focus")]
    #[strum(serialize = "toggle_plugin_focus")]
    TogglePluginFocus,

    #[strum(message = "Toggle File Explorer Focus")]
    #[strum(serialize = "toggle_file_explorer_focus")]
    ToggleFileExplorerFocus,

    #[strum(message = "Toggle Problem Focus")]
    #[strum(serialize = "toggle_problem_focus")]
    ToggleProblemFocus,

    #[strum(message = "Toggle Search Focus")]
    #[strum(serialize = "toggle_search_focus")]
    ToggleSearchFocus,

    // Visual toggle commands
    #[strum(serialize = "toggle_terminal_visual")]
    ToggleTerminalVisual,

    #[strum(serialize = "toggle_source_control_visual")]
    ToggleSourceControlVisual,

    #[strum(serialize = "toggle_plugin_visual")]
    TogglePluginVisual,

    #[strum(serialize = "toggle_file_explorer_visual")]
    ToggleFileExplorerVisual,

    #[strum(serialize = "toggle_problem_visual")]
    ToggleProblemVisual,

    #[strum(serialize = "toggle_debug_visual")]
    ToggleDebugVisual,

    #[strum(serialize = "toggle_search_visual")]
    ToggleSearchVisual,

    #[strum(serialize = "focus_editor")]
    FocusEditor,

    #[strum(serialize = "focus_terminal")]
    FocusTerminal,

    #[strum(message = "Source Control: Init")]
    #[strum(serialize = "source_control_init")]
    SourceControlInit,

    #[strum(serialize = "source_control_commit")]
    SourceControlCommit,

    #[strum(message = "Source Control: Copy Remote File Url")]
    #[strum(serialize = "source_control_copy_active_file_remote_url")]
    SourceControlCopyActiveFileRemoteUrl,

    #[strum(message = "Source Control: Discard File Changes")]
    #[strum(serialize = "source_control_discard_active_file_changes")]
    SourceControlDiscardActiveFileChanges,

    #[strum(serialize = "source_control_discard_target_file_changes")]
    SourceControlDiscardTargetFileChanges,

    #[strum(message = "Source Control: Discard Workspace Changes")]
    #[strum(serialize = "source_control_discard_workspace_changes")]
    SourceControlDiscardWorkspaceChanges,

    #[strum(serialize = "export_current_theme_settings")]
    #[strum(message = "Export current settings to a theme file")]
    ExportCurrentThemeSettings,

    #[strum(serialize = "install_theme")]
    #[strum(message = "Install current theme file")]
    InstallTheme,

    #[strum(serialize = "change_file_language")]
    #[strum(message = "Change current file language")]
    ChangeFileLanguage,

    #[strum(serialize = "change_file_line_ending")]
    #[strum(message = "Change current file line ending")]
    ChangeFileLineEnding,

    #[strum(serialize = "next_editor_tab")]
    #[strum(message = "Next Editor Tab")]
    NextEditorTab,

    #[strum(serialize = "previous_editor_tab")]
    #[strum(message = "Previous Editor Tab")]
    PreviousEditorTab,

    #[strum(serialize = "toggle_inlay_hints")]
    #[strum(message = "Toggle Inlay Hints")]
    ToggleInlayHints,

    #[strum(serialize = "restart_to_update")]
    RestartToUpdate,

    #[strum(serialize = "show_about")]
    #[strum(message = "About Lapce")]
    ShowAbout,

    #[strum(message = "Save All Files")]
    #[strum(serialize = "save_all")]
    SaveAll,

    #[cfg(target_os = "macos")]
    #[strum(message = "Install Lapce to PATH")]
    #[strum(serialize = "install_to_path")]
    InstallToPATH,

    #[cfg(target_os = "macos")]
    #[strum(message = "Uninstall Lapce from PATH")]
    #[strum(serialize = "uninstall_from_path")]
    UninstallFromPATH,

    #[strum(serialize = "jump_location_backward")]
    JumpLocationBackward,

    #[strum(serialize = "jump_location_forward")]
    JumpLocationForward,

    #[strum(serialize = "jump_location_backward_local")]
    JumpLocationBackwardLocal,

    #[strum(serialize = "jump_location_forward_local")]
    JumpLocationForwardLocal,

    #[strum(message = "Next Error in Workspace")]
    #[strum(serialize = "next_error")]
    NextError,

    #[strum(message = "Previous Error in Workspace")]
    #[strum(serialize = "previous_error")]
    PreviousError,

    #[strum(message = "Next Warn in Workspace")]
    #[strum(serialize = "next_warn")]
    NextWarn,

    #[strum(message = "Previous Warn in Workspace")]
    #[strum(serialize = "previous_warn")]
    PreviousWarn,

    #[strum(message = "Diff Files")]
    #[strum(serialize = "diff_files")]
    DiffFiles,

    #[strum(serialize = "quit")]
    #[strum(message = "Quit Editor")]
    Quit,

    #[strum(serialize = "go_to_location")]
    #[strum(message = "Go to Location")]
    GoToLocation,

    #[strum(serialize = "add_run_debug_config")]
    #[strum(message = "Add Run Debug Config")]
    AddRunDebugConfig,

    #[strum(serialize = "reveal_in_document_symbol_panel")]
    #[strum(message = "Reveal In Document Symbol Panel")]
    RevealInDocumentSymbolPanel,

    #[strum(serialize = "fold_the_current_scope_of_the_code")]
    #[strum(message = "Fold The Current Scope Of The Code")]
    FoldCode,

    #[strum(message = "Inspect Semantic Type")]
    #[strum(serialize = "inspect_semantic_type")]
    InspectSemanticType,
    #[strum(message = "Inspect Click Info")]
    #[strum(serialize = "inspect_click_info")]
    InspectClickInfo,
    #[strum(message = "Inspect Log Module")]
    #[strum(serialize = "inspect_log_module")]
    InspectLogModule,
}

#[derive(Clone, Debug)]
pub enum InternalCommand {
    ReloadConfig,
    OpenFile {
        path: PathBuf,
    },
    OpenAndConfirmedFile {
        path: PathBuf,
    },
    OpenFileInNewTab {
        path: PathBuf,
    },
    MakeConfirmed,
    OpenFileChanges {
        path: PathBuf,
    },
    ReloadFileExplorer,
    /// Test whether a file/directory can be created at that path
    TestPathCreation {
        new_path: PathBuf,
    },
    FinishRenamePath {
        current_path: PathBuf,
        new_path:     PathBuf,
    },
    FinishNewNode {
        is_dir: bool,
        path:   PathBuf,
    },
    FinishDuplicate {
        source: PathBuf,
        path:   PathBuf,
    },
    GoToLocation {
        location: EditorLocation,
    },
    JumpToLocation {
        location: EditorLocation,
    },
    JumpToMaybeRelativeLocation {
        location: EditorMaybeRelativeLocation,
    },
    PaletteReferences {
        references: Vec<EditorLocation>,
    },
    SaveJumpLocation {
        path:          PathBuf,
        offset:        usize,
        scroll_offset: Vec2,
    },
    Split {
        direction:     SplitDirection,
        editor_tab_id: EditorTabManageId,
    },
    SplitMove {
        direction:     SplitMoveDirection,
        editor_tab_id: EditorTabManageId,
    },
    SplitExchange {
        editor_tab_id: EditorTabManageId,
    },
    NewTerminal {
        profile: Option<TerminalProfile>,
    },
    // SplitTerminal {
    //     term_id: TermId,
    // },
    // SplitTerminalPrevious {
    //     term_id: TermId,
    // },
    // SplitTerminalNext {
    //     term_id: TermId,
    // },
    // SplitTerminalExchange {
    //     term_id: TermId,
    // },
    EditorTabClose {
        editor_tab_id: EditorTabManageId,
    },
    EditorTabChildClose {
        editor_tab_id: EditorTabManageId,
        child:         EditorTabChildId,
    },
    EditorTabCloseByKind {
        editor_tab_id: EditorTabManageId,
        child:         EditorTabChildId,
        kind:          TabCloseKind,
    },
    ShowCodeActions {
        offset:       usize,
        mouse_click:  bool,
        plugin_id:    PluginId,
        code_actions: im::Vector<CodeActionOrCommand>,
    },
    RunCodeAction {
        plugin_id: PluginId,
        action:    CodeActionOrCommand,
    },
    ApplyWorkspaceEdit {
        edit: WorkspaceEdit,
    },
    RunAndDebug {
        mode:   RunDebugMode,
        config: RunDebugConfig,
    },
    StartRename {
        path:        PathBuf,
        placeholder: String,
        start:       usize,
        position:    Position,
    },
    Search {
        pattern: Option<String>,
    },
    FindEditorReceiveChar {
        s: String,
    },
    ReplaceEditorReceiveChar {
        s: String,
    },
    FindEditorCommand {
        command: LapceCommand,
        count:   Option<usize>,
        mods:    Modifiers,
    },
    ReplaceEditorCommand {
        command: LapceCommand,
        count:   Option<usize>,
        mods:    Modifiers,
    },
    FocusEditorTab {
        editor_tab_id: EditorTabManageId,
    },

    SetColorTheme {
        name: String,
        /// Whether to save the theme to the config file
        save: bool,
    },
    SetIconTheme {
        name: String,
        /// Whether to save the theme to the config file
        save: bool,
    },
    SetModal {
        modal: bool,
    },
    UpdateLogLevel {
        level: LevelFilter,
    },
    OpenWebUri {
        uri: String,
    },
    ShowAlert {
        title:   String,
        msg:     String,
        buttons: Vec<AlertButton>,
    },
    HideAlert,
    SaveScratchDoc {
        doc: Rc<Doc>,
    },
    SaveScratchDoc2 {
        doc: Rc<Doc>,
    },
    UpdateProxyStatus {
        status: ProxyStatus,
    },
    DapFrameScopes {
        dap_id:   DapId,
        frame_id: usize,
    },
    OpenVoltView {
        volt_id: VoltID,
    },
    ResetBlinkCursor,
    BlinkCursor,
    OpenDiffFiles {
        left_path:  PathBuf,
        right_path: PathBuf,
    },
    ExecuteProcess {
        program:   String,
        arguments: Vec<String>,
    },
    CallHierarchyIncoming {
        root_id: ViewId,
        item_id: ViewId,
    },
    ClearTerminalBuffer {
        view_id:     ViewId,
        terminal_id: TerminalTabId,
    },
    StopTerminal {
        terminal_id: TerminalTabId,
    },
    RestartTerminal {
        terminal_id: TerminalTabId,
    },
    DocumentHighlight,
    AddOrRemoveBreakPoint {
        doc:      DocSignal,
        line_num: usize,
    },
    ShowStatusMessage {
        message: String,
    },
}

#[derive(Clone)]
pub enum WindowCommand {
    SetWorkspace {
        workspace: Arc<LapceWorkspace>,
    },
    CloseWorkspaceTab {
        index: Option<usize>,
    },
    NewWorkspaceTab {
        workspace: Arc<LapceWorkspace>,
        end:       bool,
    },
    NextWorkspaceTab,
    PreviousWorkspaceTab,
    NewWindow,
    CloseWindow,
}

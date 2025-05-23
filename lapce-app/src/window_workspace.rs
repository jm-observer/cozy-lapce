use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use alacritty_terminal::vte::ansi::Handler;
use anyhow::{Result, anyhow, bail};
use cozy_floem::views::{panel::DocStyle, tree_with_panel::data::TreePanelData};
use doc::lines::{
    buffer::rope_text::RopeText, command::FocusCommand,
    editor_command::CommandExecuted, mode::Mode, register::Register,
};
use floem::{
    ViewId,
    action::{TimerToken, exec_after},
    ext_event::create_ext_action,
    file::FileDialogOptions,
    file_action::open_file,
    keyboard::Modifiers,
    kurbo::Size,
    peniko::kurbo::{Point, Rect, Vec2},
    reactive::{
        Memo, RwSignal, Scope, SignalGet, SignalTrack, SignalUpdate, SignalWith,
        WriteSignal, batch, use_context,
    },
    text::{Attrs, AttrsList, FamilyOwned, LineHeightValue, TextLayout},
};
use im::HashMap;
use indexmap::IndexMap;
use itertools::Itertools;
use lapce_core::{
    debug::{LapceBreakpoint, RunDebugConfigs, RunDebugMode, RunDebugProcess},
    directory::Directory,
    doc::DocContent,
    id::{Id, TerminalTabId, WindowTabId},
    main_split::{
        SplitContent, SplitContentInfo, SplitDirection, SplitMoveDirection,
    },
    panel::{PanelContainerPosition, PanelKind, PanelSection, default_panel_order},
    workspace::{LapceWorkspace, LapceWorkspaceType, WorkspaceInfo},
};
use lapce_rpc::{
    RpcError,
    core::CoreNotification,
    dap_types::{ConfigSource, RunDebugConfig, SourceBreakpoint},
    file::{Naming, PathObject},
    plugin::PluginId,
    proxy::{ProxyResponse, ProxyStatus},
    source_control::FileDiff,
    terminal::TermId,
};
use lapce_xi_rope::Rope;
use log::{debug, error, trace, warn};
use lsp_types::{
    CodeActionOrCommand, CodeLens, Diagnostic, DiagnosticSeverity, MessageType,
    NumberOrString, ProgressParams, ProgressToken, ShowMessageParams,
    WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressEnd,
};
use serde_json::Value;

use crate::{
    about::AboutData,
    alert::{AlertBoxData, AlertButton},
    code_action::{CodeActionData, CodeActionStatus},
    command::{
        CommandKind, InternalCommand, LapceCommand, LapceWorkbenchCommand,
        OtherCommand, WindowCommand,
    },
    common::call_back::find_log_modules_call_back,
    completion::{CompletionData, CompletionStatus},
    config::{LapceConfig, WithLapceConfig},
    db::LapceDb,
    debug::{BreakPoints, DapData, update_breakpoints},
    doc::Doc,
    editor::location::{EditorLocation, EditorPosition},
    editor_tab::EditorTabChildId,
    file_explorer::data::FileExplorerData,
    find::Find,
    global_search::GlobalSearchData,
    hover::HoverData,
    inline_completion::InlineCompletionData,
    keypress::{EventRef, KeyPressData, KeyPressFocus, condition::Condition},
    listener::Listener,
    local_task::LocalTaskRequester,
    lsp::path_from_url,
    main_split::{MainSplitData, SplitData},
    palette::{DEFAULT_RUN_TOML, PaletteData, PaletteStatus, kind::PaletteKind},
    panel::{
        call_hierarchy_view::CallHierarchyItemData, data::PanelData,
        document_symbol::MatchDocumentSymbol,
    },
    plugin::PluginData,
    proxy::{ProxyData, new_proxy},
    rename::RenameData,
    source_control::SourceControlData,
    terminal::panel::TerminalPanelData,
    window::{CursorBlink, WindowCommonData},
};

#[derive(Clone, Debug)]
pub struct SignalManager<T>(RwSignal<T>, bool);

impl<T: Clone> Copy for SignalManager<T> {}

impl<T: Debug + Clone + 'static> SignalManager<T> {
    pub fn new(signal: RwSignal<T>) -> Self {
        Self(signal, false)
    }

    pub fn new_with_tracing(signal: RwSignal<T>) -> Self {
        Self(signal, true)
    }

    pub fn get(&self) -> T {
        self.0.get()
    }

    pub fn get_untracked(&self) -> T {
        self.0.get_untracked()
    }

    pub fn set(&self, signal: T) {
        if self.1 {
            log::debug!("set {:?} to {:?} ", self.0.get_untracked(), signal);
        }
        self.0.set(signal);
    }

    pub fn with_untracked<O>(&self, f: impl FnOnce(&T) -> O) -> O {
        self.0.with_untracked(f)
    }

    pub fn with<O>(&self, f: impl FnOnce(&T) -> O) -> O {
        self.0.with(f)
    }

    pub fn try_get_untracked(&self) -> Option<T> {
        self.0.try_get_untracked()
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        if self.1 {
            log::debug!("update");
            // panic!("ad");
        }
        self.0.update(f)
    }

    pub fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O> {
        if self.1 {
            log::debug!("set");
            // panic!("ad");
        }
        self.0.try_update(f)
    }

    pub fn try_with_untracked<O>(&self, f: impl FnOnce(Option<&T>) -> O) -> O {
        self.0.try_with_untracked(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    Workbench,
    Palette,
    CodeAction,
    Rename,
    AboutPopup,
    Panel(PanelKind),
}

#[derive(Clone)]
pub enum DragContent {
    Panel(PanelKind),
    EditorTab(EditorTabChildId),
}

impl DragContent {
    pub fn is_panel(&self) -> bool {
        matches!(self, DragContent::Panel(_))
    }
}

#[derive(Clone)]
pub struct WorkProgress {
    pub token:      ProgressToken,
    pub title:      String,
    pub message:    Option<String>,
    pub percentage: Option<u32>,
}

#[derive(Clone)]
pub struct CommonData {
    pub workspace:             Arc<LapceWorkspace>,
    pub scope:                 Scope,
    pub focus:                 SignalManager<Focus>,
    pub keypress:              RwSignal<KeyPressData>,
    pub completion:            RwSignal<CompletionData>,
    pub inline_completion:     RwSignal<InlineCompletionData>,
    pub hover:                 HoverData,
    pub register:              RwSignal<Register>,
    pub find:                  Find,
    pub workbench_size:        RwSignal<Size>,
    pub window_origin:         RwSignal<Point>,
    pub internal_command:      Listener<InternalCommand>,
    pub lapce_command:         Listener<LapceCommand>,
    pub workbench_command:     Listener<LapceWorkbenchCommand>,
    pub proxy:                 ProxyData,
    pub local_task:            LocalTaskRequester,
    pub view_id:               RwSignal<ViewId>,
    pub ui_line_height:        Memo<f64>,
    pub dragging:              RwSignal<Option<DragContent>>,
    pub config:                WithLapceConfig,
    pub proxy_status:          RwSignal<Option<ProxyStatus>>,
    pub mouse_hover_timer:     RwSignal<TimerToken>,
    pub breakpoints:           BreakPoints,
    // the current focused view which will receive keyboard events
    pub keyboard_focus:        RwSignal<Option<ViewId>>,
    pub window_common:         Rc<WindowCommonData>,
    pub directory:             Directory,
    pub offset_line_from_top:  RwSignal<Option<f64>>,
    pub sync_document_symbol:  RwSignal<bool>,
    pub document_highlight_id: RwSignal<u64>,
    pub find_view_id:          RwSignal<Option<ViewId>>,
    pub inspect_info:          RwSignal<String>,
    pub run_debug_configs:     RwSignal<RunDebugConfigs>,
    pub code_len_selected:     RwSignal<Option<Id>>,
}

impl std::fmt::Debug for CommonData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommonData")
            .field("workspace", &self.workspace)
            .finish()
    }
}

impl CommonData {
    pub fn source_breakpoints(
        &self,
    ) -> std::collections::HashMap<PathBuf, Vec<SourceBreakpoint>> {
        self.breakpoints.source_breakpoints_untracked()
    }

    pub fn update_run_debug_configs(
        &self,
        doc: &Doc,
        run_toml: &Path,
        action: &Option<impl Fn(RunDebugConfigs) + 'static>,
    ) {
        let content = doc.lines.with_untracked(|x| x.buffer().to_string());
        if content.is_empty() {
            doc.reload(Rope::from(DEFAULT_RUN_TOML), false);
            self.internal_command.send(InternalCommand::OpenFile {
                path: run_toml.to_path_buf(),
            });
        } else {
            let configs: Option<RunDebugConfigs> = toml::from_str(&content).ok();
            if let Some(mut configs) = configs {
                configs.loaded = true;
                if let Some(action) = action.as_ref() {
                    (*action)(configs.clone());
                }
                self.run_debug_configs.set(configs);
            } else {
                self.internal_command.send(InternalCommand::OpenFile {
                    path: run_toml.to_path_buf(),
                });
            }
        }
    }

    pub fn show_status_message(&self, message: String) {
        self.internal_command
            .send(InternalCommand::ShowStatusMessage { message });
    }

    pub fn show_popup_message(
        &self,
        title: String,
        typ: MessageType,
        message: String,
    ) {
        self.proxy
            .core_rpc
            .show_message(title, ShowMessageParams { typ, message });
    }
}

#[derive(Clone)]
pub struct WindowWorkspaceData {
    pub scope:                     Scope,
    pub window_tab_id:             WindowTabId,
    pub workspace:                 Arc<LapceWorkspace>,
    pub palette:                   PaletteData,
    pub main_split:                MainSplitData,
    pub file_explorer:             FileExplorerData,
    pub panel:                     PanelData,
    pub terminal:                  TerminalPanelData,
    pub plugin:                    PluginData,
    pub code_action:               RwSignal<CodeActionData>,
    pub code_lens:                 RwSignal<Option<ViewId>>,
    pub source_control:            SourceControlData,
    pub rename:                    RenameData,
    pub global_search:             GlobalSearchData,
    pub about_data:                AboutData,
    pub alert_data:                AlertBoxData,
    pub layout_rect:               RwSignal<Rect>,
    pub title_height:              RwSignal<f64>,
    pub status_height:             RwSignal<f64>,
    pub proxy:                     ProxyData,
    pub set_config:                WriteSignal<LapceConfig>,
    pub update_in_progress:        RwSignal<bool>,
    pub progresses:                RwSignal<IndexMap<ProgressToken, WorkProgress>>,
    pub messages:                  RwSignal<Vec<(String, ShowMessageParams)>>,
    pub common:                    Rc<CommonData>,
    pub document_symbol_scroll_to: RwSignal<Option<f64>>,
    pub build_data:                TreePanelData,
    pub cursor_blink:              CursorBlink,
    pub keymap_query:              RwSignal<String>,
    pub setting_query:             RwSignal<String>,
    pub theme_query:               RwSignal<String>,
}

impl std::fmt::Debug for WindowWorkspaceData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowTabData")
            .field("window_tab_id", &self.window_tab_id)
            .finish()
    }
}

impl KeyPressFocus for WindowWorkspaceData {
    fn get_mode(&self) -> Mode {
        Mode::Normal
    }

    fn check_condition(&self, condition: Condition) -> bool {
        match condition {
            Condition::PanelFocus => {
                matches!(self.common.focus.get_untracked(), Focus::Panel(_))
            },
            Condition::SourceControlFocus => {
                self.common.focus.get_untracked()
                    == Focus::Panel(PanelKind::SourceControl)
            },
            _ => false,
        }
    }

    fn run_command(
        &self,
        command: &LapceCommand,
        _count: Option<usize>,
        _mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(cmd) => {
                if let Err(err) = self.run_workbench_command(cmd.clone(), None) {
                    error!("{err:?}");
                }
            },
            CommandKind::Focus(cmd) => {
                if self.common.focus.get_untracked() == Focus::Workbench {
                    match cmd {
                        FocusCommand::SplitClose => {
                            self.main_split.editor_tab_child_close_active();
                        },
                        FocusCommand::SplitVertical => {
                            self.main_split.split_active(SplitDirection::Vertical);
                        },
                        FocusCommand::SplitHorizontal => {
                            self.main_split.split_active(SplitDirection::Horizontal);
                        },
                        FocusCommand::SplitRight => {
                            self.main_split
                                .split_move_active(SplitMoveDirection::Right);
                        },
                        FocusCommand::SplitLeft => {
                            self.main_split
                                .split_move_active(SplitMoveDirection::Left);
                        },
                        FocusCommand::SplitUp => {
                            self.main_split
                                .split_move_active(SplitMoveDirection::Up);
                        },
                        FocusCommand::SplitDown => {
                            self.main_split
                                .split_move_active(SplitMoveDirection::Down);
                        },
                        FocusCommand::SplitExchange => {
                            self.main_split.split_exchange_active();
                        },
                        _ => {
                            return CommandExecuted::No;
                        },
                    }
                }
            },
            CommandKind::Other(cmd) => {
                if let Err(err) = self.run_other_command(cmd.clone(), None) {
                    error!("{err:?}");
                }
            },
            _ => {
                return CommandExecuted::No;
            },
        }

        CommandExecuted::Yes
    }

    fn receive_char(&self, _c: &str) {}
}

impl WindowWorkspaceData {
    pub fn new(
        cx: Scope,
        workspace: Arc<LapceWorkspace>,
        window_common: Rc<WindowCommonData>,
        directory: &Directory,
        local_task: LocalTaskRequester,
        config: RwSignal<LapceConfig>,
    ) -> Result<Self> {
        let cx = cx.create_child();
        let db: Arc<LapceDb> = use_context().unwrap();

        let disabled_volts = db.get_disabled_volts().unwrap_or_default();
        let workspace_disabled_volts = db
            .get_workspace_disabled_volts(&workspace)
            .unwrap_or_default();
        let mut all_disabled_volts = disabled_volts.clone();
        all_disabled_volts.extend(workspace_disabled_volts.clone());

        let workspace_info = if workspace.path().is_some() {
            db.get_workspace_info(&workspace).ok()
        } else {
            let mut info = db.get_workspace_info(&workspace).ok();
            if let Some(info) = info.as_mut() {
                info.split.children.clear();
            }
            info
        };
        // let config = LapceConfig::load(
        //     &workspace,
        //     &all_disabled_volts,
        //     &window_common.extra_plugin_paths, directory
        // );
        let config_val = config.get_untracked();
        let lapce_command = Listener::new_empty(cx);
        let workbench_command = Listener::new_empty(cx);
        let internal_command = Listener::new_empty(cx);
        let keypress =
            cx.create_rw_signal(KeyPressData::new(cx, &config_val, directory));
        let proxy_status = cx.create_rw_signal(None);

        // let (term_tx, term_rx) = std::sync::mpsc::channel();
        // let (term_notification_tx, term_notification_rx) =
        //     std::sync::mpsc::channel();
        // {
        //     let term_notification_tx = term_notification_tx.clone();
        //     std::thread::Builder::new()
        //         .name("terminal update process".to_owned())
        //         .spawn(move || {
        //             terminal_update_process(term_rx, term_notification_tx);
        //         })
        //         .unwrap();
        // }

        let read_config = config.read_only();
        let write_only = config.write_only();
        let proxy = new_proxy(
            workspace.clone(),
            all_disabled_volts,
            window_common.extra_plugin_paths.as_ref().clone(),
            config_val.plugins.clone(),
            directory,
        );
        // let (config, set_config) = cx.create_signal(config);

        let focus =
            SignalManager::new_with_tracing(cx.create_rw_signal(Focus::Workbench));
        let config = WithLapceConfig::new(cx, read_config);
        let completion = cx.create_rw_signal(CompletionData::new(cx, config));
        let inline_completion = cx.create_rw_signal(InlineCompletionData::new(cx));
        let hover = HoverData::new(cx);

        let register = cx.create_rw_signal(Register::default());
        let view_id = cx.create_rw_signal(ViewId::new());
        let find = Find::new(cx);

        let ui_line_height = cx.create_memo(move |_| {
            let (font_family, font_size) = config.signal(|config| {
                (config.ui.font_family.signal(), config.ui.font_size.signal())
            });

            let family: Vec<FamilyOwned> = font_family.get().0;
            let font_size = font_size.get() as f32;
            let attrs = Attrs::new()
                .family(&family)
                .font_size(font_size)
                .line_height(LineHeightValue::Normal(1.8));
            let attrs_list = AttrsList::new(attrs);
            TextLayout::new_with_text("W", attrs_list).size().height
        });

        let common = Rc::new(CommonData {
            workspace: workspace.clone(),
            local_task,
            scope: cx,
            keypress,
            focus,
            completion,
            inline_completion,
            hover,
            register,
            find,
            internal_command,
            lapce_command,
            workbench_command,
            proxy: proxy.clone(),
            view_id,
            ui_line_height,
            dragging: cx.create_rw_signal(None),
            workbench_size: cx.create_rw_signal(Size::ZERO),
            config,
            proxy_status,
            mouse_hover_timer: cx.create_rw_signal(TimerToken::INVALID),
            window_origin: cx.create_rw_signal(Point::ZERO),
            breakpoints: BreakPoints {
                breakpoints: cx.create_rw_signal(BTreeMap::new()),
            },
            keyboard_focus: cx.create_rw_signal(None),
            window_common: window_common.clone(),
            directory: directory.clone(),
            offset_line_from_top: cx.create_rw_signal(None),
            sync_document_symbol: cx.create_rw_signal(true),
            document_highlight_id: cx.create_rw_signal(0),
            find_view_id: cx.create_rw_signal(None),
            inspect_info: cx.create_rw_signal(String::new()),
            run_debug_configs: cx.create_rw_signal(RunDebugConfigs::default()),
            code_len_selected: cx.create_rw_signal(None),
        });

        let main_split = MainSplitData::new(cx, common.clone());
        let code_action =
            cx.create_rw_signal(CodeActionData::new(cx, common.clone()));
        let source_control =
            SourceControlData::new(cx, main_split.editors, common.clone());
        let file_explorer = FileExplorerData::new(cx, common.clone());

        if let Some(info) = workspace_info.as_ref() {
            let root_split = main_split.root_split;
            SplitData::to_data(&info.split, main_split.clone(), None, root_split);
        } else {
            let root_split = main_split.root_split;
            let root_split_data = {
                let cx = cx.create_child();
                let root_split_data = SplitData {
                    scope:         cx,
                    parent_split:  None,
                    split_id:      root_split,
                    children:      Vec::new(),
                    direction:     SplitDirection::Horizontal,
                    window_origin: Point::ZERO,
                    layout_rect:   Rect::ZERO,
                };
                cx.create_rw_signal(root_split_data)
            };
            main_split.splits.update(|splits| {
                splits.insert(root_split, root_split_data);
            });
        }

        let palette = PaletteData::new(
            cx,
            workspace.clone(),
            main_split.clone(),
            keypress.read_only(),
            source_control.clone(),
            common.clone(),
        );

        let hide_cursor = window_common.hide_cursor;

        let title_height = cx.create_rw_signal(0.0);
        let status_height = cx.create_rw_signal(0.0);
        let panel_available_size = cx.create_memo(move |_| {
            let title_height = title_height.get();
            let status_height = status_height.get();
            let window_size = window_common.size.get();
            Size::new(
                window_size.width,
                window_size.height - title_height - status_height,
            )
        });
        let panel = workspace_info
            .as_ref()
            .map(|i| {
                let panel_order = db
                    .get_panel_orders()
                    .unwrap_or_else(|_| default_panel_order());
                PanelData {
                    panels:         cx.create_rw_signal(panel_order),
                    styles:         cx.create_rw_signal(i.panel.styles.clone()),
                    size:           cx.create_rw_signal(i.panel.size.clone()),
                    available_size: panel_available_size,
                    sections:       cx.create_rw_signal(
                        i.panel
                            .sections
                            .iter()
                            .map(|(key, value)| (*key, cx.create_rw_signal(*value)))
                            .collect(),
                    ),
                    common:         common.clone(),
                }
            })
            .unwrap_or_else(|| {
                let panel_order = db
                    .get_panel_orders()
                    .unwrap_or_else(|_| default_panel_order());
                PanelData::new(
                    cx,
                    panel_order,
                    panel_available_size,
                    im::HashMap::new(),
                    common.clone(),
                )
            });

        let terminal = TerminalPanelData::new(
            workspace.clone(),
            common
                .config
                .with_untracked(|config| config.terminal.get_default_profile()),
            common.clone(),
            main_split.clone(),
            view_id,
        );
        if let Some(workspace_info) = workspace_info.as_ref() {
            terminal.common.breakpoints.set(
                workspace_info
                    .breakpoints
                    .clone()
                    .into_iter()
                    .map(|(path, breakpoints)| {
                        (
                            path,
                            breakpoints
                                .into_iter()
                                .map(|b| (b.line, b))
                                .collect::<BTreeMap<usize, LapceBreakpoint>>(),
                        )
                    })
                    .collect(),
            );
        }

        let rename = RenameData::new(cx, common.clone());
        let global_search = GlobalSearchData::new(cx, main_split.clone());

        let plugin = PluginData::new(
            cx,
            HashSet::from_iter(disabled_volts),
            HashSet::from_iter(workspace_disabled_volts),
            common.clone(),
            proxy.core_rpc.clone(),
        );

        let about_data = AboutData::new(cx, common.focus);
        let alert_data = AlertBoxData::new(cx, common.clone());
        let build_data = TreePanelData::new(cx, DocStyle::default());
        let cursor_blink_timer = cx.create_rw_signal(TimerToken::INVALID);
        let cursor_blink = CursorBlink {
            hide_cursor,
            blink_timer: cursor_blink_timer,
            blink_interval: cx.create_rw_signal(0),
            common_data: common.clone(),
        };

        let cursor_blink_clone = cursor_blink.clone();
        cx.create_effect(move |_| {
            let blink_interval = config
                .signal(|config| config.editor.blink_interval.signal())
                .get();
            // log::debug!("update blink_interval {}", blink_interval);
            cursor_blink_clone.blink_interval.set(blink_interval);
            cursor_blink_clone.blink(None);
        });

        let window_tab_data = Self {
            scope: cx,
            window_tab_id: WindowTabId::next(),
            workspace,
            palette,
            main_split,
            terminal,
            panel,
            file_explorer,
            code_action,
            code_lens: cx.create_rw_signal(None),
            source_control,
            plugin,
            rename,
            global_search,
            about_data,
            alert_data,
            layout_rect: cx.create_rw_signal(Rect::ZERO),
            title_height,
            status_height,
            proxy,
            set_config: write_only,
            update_in_progress: cx.create_rw_signal(false),
            progresses: cx.create_rw_signal(IndexMap::new()),
            messages: cx.create_rw_signal(Vec::new()),
            common,
            document_symbol_scroll_to: cx.create_rw_signal(None),
            build_data,
            cursor_blink,
            keymap_query: cx.create_rw_signal(String::new()),
            setting_query: cx.create_rw_signal(String::new()),
            theme_query: cx.create_rw_signal(String::new()),
        };

        {
            let focus = window_tab_data.common.focus;
            let active_editor = window_tab_data.main_split.active_editor;
            let rename_active = window_tab_data.rename.active;
            let internal_command = window_tab_data.common.internal_command;
            cx.create_effect(move |_| {
                let focus = focus.get();
                active_editor.track();
                internal_command.send(InternalCommand::ResetBlinkCursor);

                if focus != Focus::Rename && rename_active.get_untracked() {
                    rename_active.set(false);
                }
            });
        }

        {
            let window_tab_data = window_tab_data.clone();
            window_tab_data.common.lapce_command.listen(move |cmd| {
                window_tab_data.run_lapce_command(cmd);
            });
        }

        {
            let window_tab_data = window_tab_data.clone();
            window_tab_data.common.workbench_command.listen(move |cmd| {
                if let Err(err) = window_tab_data.run_workbench_command(cmd, None) {
                    error!("{err:?}");
                }
            });
        }

        {
            let window_tab_data = window_tab_data.clone();
            let internal_command = window_tab_data.common.internal_command;
            internal_command.listen(move |cmd| {
                if let Err(err) = window_tab_data.run_internal_command(cmd) {
                    error!("{}", err);
                }
            });
        }

        {
            let window_tab_data = window_tab_data.clone();
            let notification = window_tab_data.proxy.notification;
            cx.create_effect(move |_| {
                notification.with(|rpc| {
                    if let Some(rpc) = rpc.as_ref() {
                        window_tab_data.handle_core_notification(rpc);
                    }
                });
            });
        }

        Ok(window_tab_data)
    }

    pub fn reload_config(&self) {
        log::debug!("reload_config");
        let db: Arc<LapceDb> = use_context().unwrap();

        let disabled_volts = db.get_disabled_volts().unwrap_or_default();
        let workspace_disabled_volts = db
            .get_workspace_disabled_volts(&self.workspace)
            .unwrap_or_default();
        let mut all_disabled_volts = disabled_volts;
        all_disabled_volts.extend(workspace_disabled_volts);

        let config = LapceConfig::load(
            &self.workspace,
            &all_disabled_volts,
            &self.common.window_common.extra_plugin_paths,
            &self.common.directory,
        );
        self.common.keypress.update(|keypress| {
            keypress.update_keymaps(&config);
        });

        let mut change_plugins = Vec::new();
        for (key, configs) in self
            .common
            .config
            .with_untracked(|x| x.plugins.clone())
            .iter()
        {
            if config
                .plugins
                .get(key)
                .map(|x| x != configs)
                .unwrap_or_default()
            {
                change_plugins.push(key.clone());
            }
        }
        self.set_config.set(config.clone());
        if !change_plugins.is_empty() {
            self.common
                .proxy
                .proxy_rpc
                .update_plugin_configs(config.plugins.clone());
            if config.core.auto_reload_plugin {
                let mut plugin_metas: HashMap<
                    String,
                    lapce_rpc::plugin::VoltMetadata,
                > = self
                    .plugin
                    .installed
                    .get_untracked()
                    .values()
                    .map(|x| {
                        let meta = x.meta.get_untracked();
                        (meta.name.clone(), meta)
                    })
                    .collect();
                for name in change_plugins {
                    if let Some(meta) = plugin_metas.remove(&name) {
                        self.common.proxy.proxy_rpc.reload_volt(meta);
                    } else {
                        log::error!("not found volt metadata of {}", name);
                    }
                }
            }
        }
    }

    pub fn run_lapce_command(&self, cmd: LapceCommand) {
        match cmd.kind {
            CommandKind::Workbench(command) => {
                if let Err(err) = self.run_workbench_command(command, cmd.data) {
                    error!("{err:?}");
                }
            },
            CommandKind::Other(command) => {
                if let Err(err) = self.run_other_command(command, cmd.data) {
                    error!("{err:?}");
                }
            },
            CommandKind::Scroll(_)
            | CommandKind::Focus(_)
            | CommandKind::Edit(_)
            | CommandKind::Move(_) => {
                if self.palette.status.get_untracked() != PaletteStatus::Inactive {
                    self.palette.run_command(&cmd, None, Modifiers::empty());
                } else if let Some(editor_data) =
                    self.main_split.active_editor.get_untracked()
                {
                    editor_data.run_command(&cmd, None, Modifiers::empty());
                } else {
                    // TODO: dispatch to current focused view?
                }
            },
            CommandKind::MotionMode(_) => {},
            CommandKind::MultiSelection(_) => {},
        }
    }

    pub fn run_other_command(
        &self,
        cmd: OtherCommand,
        _data: Option<Value>,
    ) -> Result<()> {
        use OtherCommand::*;
        match cmd {
            RightMenuRunCodeLen { id, .. } => {
                if let Some(editor_data) =
                    self.main_split.active_editor.get_untracked()
                    && let Some(code_len) =
                        editor_data.doc().code_lens.with_untracked(|x| {
                            x.values()
                                .find_map(|x| x.2.iter().find(|x| x.0 == id))
                                .cloned()
                        })
                    && let Some(command) = code_len.1.command
                {
                    self.main_split.run_code_lens(
                        &command.command,
                        command.arguments.unwrap_or_default(),
                    );
                } else {
                    debug!("code len {id:?} not found or command is none");
                }
            },
        }
        Ok(())
    }

    pub fn run_workbench_command(
        &self,
        cmd: LapceWorkbenchCommand,
        data: Option<Value>,
    ) -> Result<()> {
        use LapceWorkbenchCommand::*;
        match cmd {
            // ==== Modal ====
            EnableModal => {
                        let internal_command = self.common.internal_command;
                        internal_command.send(InternalCommand::SetModal { modal: true });
                    }
            DisableModal => {
                        let internal_command = self.common.internal_command;
                        internal_command.send(InternalCommand::SetModal { modal: false });
                    }
            OpenFolder => {
                        if !self.workspace.kind().is_remote() {
                            let window_command = self.common.window_common.window_command;
                            let mut options = FileDialogOptions::new().select_directories();
                            options = if let Some(parent) = self.workspace.path().and_then(|x| x.parent()) {
                                options.force_starting_directory(parent)
                            } else {
                                options
                            };
                            open_file(options, move |file| {
                                if let Some(mut file) = file {
                                    let workspace = LapceWorkspace::new(
                                         LapceWorkspaceType::Local,
                                         Some(if let Some(path) = file.path.pop() {
                                            path
                                        } else {
                                            log::error!("No path");
                                            return;
                                        }),
                                         std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs(),
                                    ).into();
                                    window_command
                                        .send(WindowCommand::SetWorkspace { workspace });
                                }
                            });
                        }
                    }
            CloseFolder => {
                        if !self.workspace.kind().is_remote() {
                            let window_command = self.common.window_common.window_command;
                            let workspace = LapceWorkspace::new(LapceWorkspaceType::Local, None, 0).into();
                            window_command.send(WindowCommand::SetWorkspace { workspace });
                        }
                    }
            OpenFile => {
                        if !self.workspace.kind().is_remote() {
                            let internal_command = self.common.internal_command;
                            let options = FileDialogOptions::new();
                            open_file(options, move |file| {
                                if let Some(mut file) = file {
                                    internal_command.send(InternalCommand::OpenFile {
                                        path: if let Some(path) = file.path.pop() {
                                            path
                                        } else {
                                            log::error!("No path");
                                            return;
                                        },
                                    })
                                }
                            });
                        }
                    }
            NewFile => {
                        self.main_split.new_file();
                    }
            RevealActiveFileInFileExplorer => {
                        if let Some(editor_data) = self.main_split.active_editor.get() {
                            let doc = editor_data.doc();
                            let path = if let DocContent::File { path, .. } =
                                doc.content.get_untracked()
                            {
                                Some(path)
                            } else {
                                None
                            };
                            let Some(path) = path else { return Ok(()) };
                            let path = path.parent().unwrap_or(&path);

                            open_uri(path);
                        }
                    }
            SaveAll => {
                        self.main_split.editors.with_editors_untracked(|editors| {
                            let mut paths = HashSet::new();
                            for (_, editor_data) in editors.iter() {
                                let doc = editor_data.doc();
                                let should_save = if let DocContent::File { path, .. } =
                                    doc.content.get_untracked()
                                {
                                    if paths.contains(&path) {
                                        false
                                    } else {
                                        paths.insert(path.clone());

                                        true
                                    }
                                } else {
                                    false
                                };

                                if should_save && let Err(err) = editor_data.save(true, || {}) {
                                        error!("{err}");
                                }
                            }
                        });
                    }
            OpenSettings => {
                        self.main_split.open_settings();
                    }
            OpenSettingsFile => {
                        if let Some(path) = LapceConfig::settings_file(&self.common.directory.config_directory) {
                            self.main_split.jump_to_location(
                                EditorLocation {
                                    path,
                                    position: None,
                                    scroll_offset: None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab: false,
                                },
                                None,
                            );
                        }
                    }
            OpenSettingsDirectory => {
                        open_uri(&self.common.directory.config_directory);
                    }
            OpenThemeColorSettings => {
                        self.main_split.open_theme_color_settings();
                    }
            OpenKeyboardShortcuts => {
                        self.main_split.open_keymap();
                    }
            OpenKeyboardShortcutsFile => {
                        if let Some(path) = LapceConfig::keymaps_file(&self.common.directory.config_directory) {
                            self.main_split.jump_to_location(
                                EditorLocation {
                                    path,
                                    position: None,
                                    scroll_offset: None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab: false,
                                },
                                None,
                            );
                        }
                    }
            OpenLogFile => {
                        self.open_paths(&[PathObject::from_path(
                            self.common.directory.logs_directory.join(format!(
                                "lapce.{}.log",
                                chrono::prelude::Local::now().format("%Y-%m-%d")
                            )),
                            false,
                        )]);
                    }
            OpenLogsDirectory => {
                        open_uri(&self.common.directory.logs_directory);
                    }
            OpenProxyDirectory => {
                        open_uri(&self.common.directory.proxy_directory);
                    }
            OpenThemesDirectory => {
                        open_uri(&self.common.directory.themes_directory);
                    }
            OpenPluginsDirectory => {
                        open_uri(&self.common.directory.plugins_directory);
                    }
            OpenGrammarsDirectory => {
                        open_uri(&self.common.directory.grammars_directory);
                    }
            OpenQueriesDirectory => {
                        open_uri(&self.common.directory.queries_directory);
                    }
            InstallTheme => {}
            ExportCurrentThemeSettings => {
                        self.main_split.export_theme();
                    }
            ToggleInlayHints => {}
            ReloadWindow => {
                        self.common.window_common.window_command.send(
                            WindowCommand::SetWorkspace {
                                workspace: self.workspace.clone(),
                            },
                        );
                    }
            NewWindow => {
                        self.common
                            .window_common
                            .window_command
                            .send(WindowCommand::NewWindow);
                    }
            CloseWindow => {
                        self.common
                            .window_common
                            .window_command
                            .send(WindowCommand::CloseWindow);
                    }
            NewWindowTab => {
                        self.common.window_common.window_command.send(
                            WindowCommand::NewWorkspaceTab {
                                workspace: LapceWorkspace::default().into(),
                                end: false,
                            },
                        );
                    }
            CloseWindowTab => {
                        self.common
                            .window_common
                            .window_command
                            .send(WindowCommand::CloseWorkspaceTab { index: None });
                    }
            NextWindowTab => {
                        self.common
                            .window_common
                            .window_command
                            .send(WindowCommand::NextWorkspaceTab);
                    }
            PreviousWindowTab => {
                        self.common
                            .window_common
                            .window_command
                            .send(WindowCommand::PreviousWorkspaceTab);
                    }
            NextEditorTab => {
                        if let Some(editor_tab_id) =
                            self.main_split.active_editor_tab.get_untracked()
                        {
                            self.main_split.editor_tabs.with_untracked(|editor_tabs| {
                                let Some(editor_tab) = editor_tabs.get(&editor_tab_id)
                                else {
                                    return;
                                };

                                let new_index = editor_tab.with_untracked(|editor_tab| {
                                    if editor_tab.children.is_empty() {
                                        None
                                    } else if editor_tab.active
                                        == editor_tab.children.len() - 1
                                    {
                                        Some(0)
                                    } else {
                                        Some(editor_tab.active + 1)
                                    }
                                });

                                if let Some(new_index) = new_index {
                                    editor_tab.update(|editor_tab| {
                                        editor_tab.active = new_index;
                                    });
                                }
                            });
                        }
                    }
            PreviousEditorTab => {
                        if let Some(editor_tab_id) =
                            self.main_split.active_editor_tab.get_untracked()
                        {
                            self.main_split.editor_tabs.with_untracked(|editor_tabs| {
                                let Some(editor_tab) = editor_tabs.get(&editor_tab_id)
                                else {
                                    return;
                                };

                                let new_index = editor_tab.with_untracked(|editor_tab| {
                                    if editor_tab.children.is_empty() {
                                        None
                                    } else if editor_tab.active == 0 {
                                        Some(editor_tab.children.len() - 1)
                                    } else {
                                        Some(editor_tab.active - 1)
                                    }
                                });

                                if let Some(new_index) = new_index {
                                    editor_tab.update(|editor_tab| {
                                        editor_tab.active = new_index;
                                    });
                                }
                            });
                        }
                    }
            NewTerminalTab => {
                        self.terminal.new_tab(
                            self.common
                                .config
                                .with_untracked(|x| x
                                    .terminal
                                    .get_default_profile()),
                        );
                        if !self.panel.is_panel_visible(&PanelKind::Terminal) {
                            self.panel.show_panel(&PanelKind::Terminal);
                        }
                        self.common.focus.set(Focus::Panel(PanelKind::Terminal));
                    }
            CloseTerminalTab => {
                        self.terminal.close_tab(None);
                        if self
                            .terminal
                            .tab_infos
                            .with_untracked(|info| info.tabs.is_empty())
                        {
                            if self.panel.is_panel_visible(&PanelKind::Terminal) {
                                self.panel.hide_panel(&PanelKind::Terminal);
                            }
                            self.common.focus.set(Focus::Workbench);
                        } else {
                            if !self.panel.is_panel_visible(&PanelKind::Terminal) {
                                self.panel.show_panel(&PanelKind::Terminal);
                            }
                            self.common.focus.set(Focus::Panel(PanelKind::Terminal));
                        }
                    }
            NextTerminalTab => {
                        self.terminal.next_tab();
                        if !self.panel.is_panel_visible(&PanelKind::Terminal) {
                            self.panel.show_panel(&PanelKind::Terminal);
                        }
                        self.common.focus.set(Focus::Panel(PanelKind::Terminal));
                    }
            PreviousTerminalTab => {
                        self.terminal.previous_tab();
                        if !self.panel.is_panel_visible(&PanelKind::Terminal) {
                            self.panel.show_panel(&PanelKind::Terminal);
                        }
                        self.common.focus.set(Focus::Panel(PanelKind::Terminal));
                    }
            ConnectSshHost => {
                        self.palette.run(PaletteKind::SshHost);
                    }
            #[cfg(windows)]
                    ConnectWslHost => {
                        self.palette.run(PaletteKind::WslHost);
                    }
            DisconnectRemote => {
                        self.common.window_common.window_command.send(
                            WindowCommand::SetWorkspace {
                                workspace: LapceWorkspace::new(LapceWorkspaceType::Local, None, 0).into(),
                            },
                        );
                    }
            PaletteHelp => self.palette.run(PaletteKind::PaletteHelp),
            PaletteHelpAndFile => self.palette.run(PaletteKind::HelpAndFile),
            PaletteLine => {
                        self.palette.run(PaletteKind::Line);
                    }
            Palette => {
                        self.palette.run(PaletteKind::HelpAndFile);
                    }
            PaletteSymbol => {
                        self.palette.run(PaletteKind::DocumentSymbol);
                    }
            PaletteWorkspaceSymbol => {}
            PaletteCommand => {
                        self.palette.run(PaletteKind::Command);
                    }
            PaletteWorkspace => {
                        self.palette.run(PaletteKind::Workspace);
                    }
            PaletteRunAndDebug => {
                        self.palette.run(PaletteKind::RunAndDebug);
                    }
            PaletteSCMReferences => {
                        self.palette.run(PaletteKind::SCMReferences);
                    }
            ChangeColorTheme => {
                        self.palette.run(PaletteKind::ColorTheme);
                    }
            ChangeIconTheme => {
                        self.palette.run(PaletteKind::IconTheme);
                    }
            ChangeFileLanguage => {
                        self.palette.run(PaletteKind::Language);
                    }
            ChangeFileLineEnding => {
                        self.palette.run(PaletteKind::LineEnding);
                    }
            DiffFiles => self.palette.run(PaletteKind::DiffFiles),
            RunAndDebugRestart => {
                        let active_term = self.terminal.debug.active_term.get_untracked();
                        if let Some(term_id) = active_term {
                            if let Err(err) = self.restart_run_program_in_terminal(term_id) {
                                error!("RestartTerminal {err:?}");
                            }
                            self.panel.show_panel(&PanelKind::Terminal);
                            let terminal = self.terminal.get_terminal(term_id).ok_or(anyhow!("get_terminal {:?} fail", term_id))?;
                            let Some(is_debug) = terminal
                                .data
                                .with_untracked(|x| {
                                    x.run_debug.as_ref().map(|x| x.mode == RunDebugMode::Debug)
                                }) else {
                                        return Ok(());
                                };
                            if is_debug {
                                self.panel.show_panel(&PanelKind::Debug);
                            }
                        } else {
                            self.palette.run(PaletteKind::RunAndDebug);
                        }
                    }
            RunAndDebugStop => {
                        let active_term = self.terminal.debug.active_term.get_untracked();
                        if let Some(term_id) = active_term {
                            self.terminal.manual_stop_run_debug(term_id);
                        }
                    }
            ZoomIn => {
                        let mut scale =
                            self.common.window_common.window_scale.get_untracked();
                        scale += 0.1;
                        if scale > 4.0 {
                            scale = 4.0
                        }
                        self.common.window_common.window_scale.set(scale);

                        LapceConfig::update_file(
                            "ui",
                            "scale",
                            toml_edit::Value::from(scale), self.common.clone(),
                        );
                    }
            ZoomOut => {
                        let mut scale =
                            self.common.window_common.window_scale.get_untracked();
                        scale -= 0.1;
                        if scale < 0.1 {
                            scale = 0.1
                        }
                        self.common.window_common.window_scale.set(scale);

                        LapceConfig::update_file(
                            "ui",
                            "scale",
                            toml_edit::Value::from(scale), self.common.clone(),
                        );
                    }
            ZoomReset => {
                        self.common.window_common.window_scale.set(1.0);

                        LapceConfig::update_file(
                            "ui",
                            "scale",
                            toml_edit::Value::from(1.0), self.common.clone(),
                        );
                    }
            ToggleMaximizedPanel => {
                        if let Some(data) = data {
                            if let Ok(kind) = serde_json::from_value::<PanelKind>(data) {
                                self.panel.toggle_maximize(&kind);
                            }
                        } else {
                            self.panel.toggle_active_maximize();
                        }
                    }
            HidePanel => {
                        if let Some(data) = data && let Ok(kind) = serde_json::from_value::<PanelKind>(data) {
                                self.hide_panel(kind);
                        }
                    }
            ShowPanel => {
                        if let Some(data) = data && let Ok(kind) = serde_json::from_value::<PanelKind>(data) {
                                self.show_panel(kind);
                        }
                    }
            TogglePanelFocus => {
                        if let Some(data) = data && let Ok(kind) = serde_json::from_value::<PanelKind>(data) {
                                self.toggle_panel_focus(kind);
                        }
                    }
            TogglePanelVisual => {
                        if let Some(data) = data && let Ok(kind) = serde_json::from_value::<PanelKind>(data) {
                                self.toggle_panel_visual(kind);
                        }
                    }
            TogglePanelLeftVisual => {
                        self.toggle_container_visual(&PanelContainerPosition::Left);
                    }
            TogglePanelRightVisual => {
                        self.toggle_container_visual(&PanelContainerPosition::Right);
                    }
            TogglePanelBottomVisual => {
                        self.toggle_container_visual(&PanelContainerPosition::Bottom);
                    }
            ToggleTerminalFocus => {
                        self.toggle_panel_focus(PanelKind::Terminal);
                    }
            ToggleSourceControlFocus => {
                        self.toggle_panel_focus(PanelKind::SourceControl);
                    }
            TogglePluginFocus => {
                        self.toggle_panel_focus(PanelKind::Plugin);
                    }
            ToggleFileExplorerFocus => {
                        self.toggle_panel_focus(PanelKind::FileExplorer);
                    }
            ToggleProblemFocus => {
                        self.toggle_panel_focus(PanelKind::Problem);
                    }
            ToggleSearchFocus => {
                        self.toggle_panel_focus(PanelKind::Search);
                    }
            ToggleTerminalVisual => {
                        self.toggle_panel_visual(PanelKind::Terminal);
                    }
            ToggleSourceControlVisual => {
                        self.toggle_panel_visual(PanelKind::SourceControl);
                    }
            TogglePluginVisual => {
                        self.toggle_panel_visual(PanelKind::Plugin);
                    }
            ToggleFileExplorerVisual => {
                        self.toggle_panel_visual(PanelKind::FileExplorer);
                    }
            ToggleProblemVisual => {
                        self.toggle_panel_visual(PanelKind::Problem);
                    }
            ToggleDebugVisual => {
                        self.toggle_panel_visual(PanelKind::Debug);
                    }
            ToggleSearchVisual => {
                        self.toggle_panel_visual(PanelKind::Search);
                    }
            FocusEditor => {
                        self.common.focus.set(Focus::Workbench);
                    }
            FocusTerminal => {
                        self.common.focus.set(Focus::Panel(PanelKind::Terminal));
                    }
            OpenUIInspector => {
                        crate::log::log(self);
                        self.common.view_id.get_untracked().inspect();
                    }
            ShowEnvironment => {
                        self.main_split.show_env();
                    }
            SourceControlInit => {
                        self.proxy.proxy_rpc.git_init();
                    }
            CheckoutReference => match data {
                        Some(reference) => {
                            if let Some(reference) = reference.as_str() {
                                self.proxy.proxy_rpc.git_checkout(reference.to_string());
                            }
                        }
                        None => error!("No ref provided"),
                    },
            SourceControlCommit => {
                        self.source_control.commit();
                    }
            SourceControlCopyActiveFileRemoteUrl => {
                        // TODO:
                    }
            SourceControlDiscardActiveFileChanges => {
                        // TODO:
                    }
            SourceControlDiscardTargetFileChanges => {
                        if let Some(diff) = data
                            .and_then(|data| serde_json::from_value::<FileDiff>(data).ok())
                        {
                            match diff {
                                FileDiff::Added(path) => {
                                    self.common.proxy.proxy_rpc
                    .trash_path(path, Box::new(|(_, _)| {}));
                                }
                                FileDiff::Modified(path) | FileDiff::Deleted(path) => {
                                    self.common.proxy.proxy_rpc
                    .git_discard_files_changes(vec![path]);
                                }
                                FileDiff::Renamed(old_path, new_path) => {
                                    self.common
                                        .proxy
                                        .proxy_rpc.git_discard_files_changes(vec![old_path]);
                                    self.common.proxy.proxy_rpc.trash_path(new_path, Box::new(|(_, _)| {}));
                                }
                            }
                        }
                    }
            SourceControlDiscardWorkspaceChanges => {
                        // TODO:
                    }
            ShowAbout => {
                        self.about_data.open();
                    }
            RestartToUpdate => {
                        log::error!("todo restart to update");
                        // if let Some(release) = self
                        //     .common
                        //     .window_common
                        //     .latest_release
                        //     .get_untracked()
                        //     .as_ref()
                        // {
                        //     let release = release.clone();
                        //     let update_in_progress = self.update_in_progress;
                        //     if release.version != *meta::VERSION {
                        //         if let Ok(process_path) = env::current_exe() {
                        //             update_in_progress.set(true);
                        //             let send = create_ext_action(
                        //                 self.common.scope,
                        //                 move |_started| {
                        //                     update_in_progress.set(false);
                        //                 },
                        //             );
                        //             let updates_directory = self.common.directory.updates_directory.clone();
                        //             // todo remove thread
                        //             std::thread::Builder::new().name("RestartToUpdate".to_owned()).spawn(move || {
                        //                 let do_update = || -> anyhow::Result<()> {
                        //                     let src =
                        //                         crate::update::download_release(&release, updates_directory.as_ref())?;
                        //
                        //                     let path =
                        //                         crate::update::extract(&src, &process_path)?;
                        //
                        //                     crate::update::restart(&path)?;
                        //
                        //                     Ok(())
                        //                 };
                        //
                        //                 if let Err(err) = do_update() {
                        //                     error!("Failed to update: {err}");
                        //                 }
                        //
                        //                 send(false);
                        //             }).unwrap();
                        //         }
                        //     }
                        // }
                    }
            #[cfg(target_os = "macos")]
                    InstallToPATH => {
                        self.common.internal_command.send(
                            InternalCommand::ExecuteProcess {
                                program: String::from("osascript"),
                                arguments: vec![String::from("-e"), format!(r#"do shell script "ln -sf '{}' /usr/local/bin/lapce" with administrator privileges"#, std::env::args().next().unwrap())],
                            }
                        )
                    }
            #[cfg(target_os = "macos")]
                    UninstallFromPATH => {
                        self.common.internal_command.send(
                            InternalCommand::ExecuteProcess {
                                program: String::from("osascript"),
                                arguments: vec![String::from("-e"), String::from(r#"do shell script "rm /usr/local/bin/lapce" with administrator privileges"#)],
                            }
                        )
                    }
            JumpLocationForward => {
                        self.main_split.jump_location_forward(false);
                    }
            JumpLocationBackward => {
                        self.main_split.jump_location_backward(false);
                    }
            JumpLocationForwardLocal => {
                        self.main_split.jump_location_forward(true);
                    }
            JumpLocationBackwardLocal => {
                        self.main_split.jump_location_backward(true);
                    }
            NextError => {
                        self.main_split.next_error(DiagnosticSeverity::ERROR);
                    }
            PreviousError => {
                        self.main_split.prev_error(DiagnosticSeverity::ERROR);
                    }
            NextWarn => {
                        self.main_split.next_error(DiagnosticSeverity::WARNING);
                    }
            PreviousWarn => {
                        self.main_split.prev_error(DiagnosticSeverity::WARNING);
                    }
            Quit => {
                        floem::quit_app();
                    }
            RevealInPanel => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            self.show_panel(PanelKind::FileExplorer);
                            self.panel
                                .section_open(PanelSection::FileExplorer).update(|x| {
                                *x = true;
                            });
                            if let DocContent::File { path, .. } = editor_data.doc().content.get_untracked() {
                                self.file_explorer.reveal_in_file_tree(path);
                            }
                        }
                    }
            RevealInDocumentSymbolPanel => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            self.show_panel(PanelKind::DocumentSymbol);
                            let offset = editor_data.cursor().with_untracked(|c| c.offset());
                            let doc = editor_data.doc();
                            let line = match doc.lines.with_untracked(|x| {
                                x.buffer().offset_to_line_col(offset).map(|x| x.0)
                            }) {
                                Ok(rs) => { rs }
                                Err(err) => {
                                    error!("{err:?}");
                                    return Ok(());
                                }
                            };
                            let rs = doc.document_symbol_data.virtual_list.with_untracked(|x| {
                                x.match_line_with_children(line as u32)
                            });
                            if let Some(MatchDocumentSymbol::MatchSymbol(id, index)) = rs {
                                batch(|| {
                                    doc.document_symbol_data.select.set(Some(id));
                                    doc.document_symbol_data.scroll_to.set(Some(index as f64));
                                })
                            }
                        }
                    }
            SourceControlOpenActiveFileRemoteUrl => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked() && let DocContent::File { path, .. } = editor_data.doc().content.get_untracked() {
                                let offset = editor_data.cursor().with_untracked(|c| c.offset());
                                let line = editor_data.doc()
                                    .lines.with_untracked(|x| x.buffer().line_of_offset(offset));
                                self.common.proxy.proxy_rpc.git_get_remote_file_url(
                                    path,
                                    create_ext_action(self.scope, move |(_, result)| {
                                        if let Ok(ProxyResponse::GitGetRemoteFileUrl {
                                                      file_url
                                                  }) = result
                                        && let Err(err) = open::that(format!("{file_url}#L{line}",)) {
                                                error!("Failed to open file in github: {err}",  );
                                        }
                                    }),
                                );
                        }
                    }
            RevealInFileExplorer => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        && let DocContent::File { path, .. } = editor_data.doc().content.get_untracked() {
                                let path = path.parent().unwrap_or(&path);
                                if !path.exists() {
                                    return Ok(());
                                }
                        if let Err(err) = open::that(path) {
                            error!(
                            "Failed to reveal file in system file explorer: {err}",
                        );
                        }
                            }
                    }
            FoldCode => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            editor_data.fold_code()?;
                        }
                    }
            ShowCallHierarchy => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            editor_data.call_hierarchy(self.clone())?;
                        }
                    }
            FindReferences => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            editor_data.find_refenrence(self.clone())?;
                        }
                    }
            GoToImplementation => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            editor_data.go_to_implementation(self.clone())?;
                        }
                    }
            RunInTerminal => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            let name = editor_data.word_at_cursor();
                            if !name.is_empty() {
                                let mut args_str = name.split(" ");
                                let program = args_str.next().map(|x| x.to_string()).unwrap();
                                let args: Vec<String> = args_str.map(|x| x.to_string()).collect();
                                let args = if args.is_empty() {
                                    None
                                } else {
                                    Some(args)
                                };

                                let config = RunDebugConfig {
                                    ty: None,
                                    name,
                                    program,
                                    args,
                                    cwd: None,
                                    env: None,
                                    prelaunch: None,
                                    debug_command: None,
                                    dap_id: Default::default(),
                                    tracing_output: false,
                                    config_source: ConfigSource::RunInTerminal,
                                };
                                self.common
                                    .internal_command
                                    .send(InternalCommand::RunAndDebug { mode: RunDebugMode::Run, config });
                            }
                        }
                    }
            GoToLocation => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            let doc = editor_data.doc();
                            let path = match if doc.loaded() {
                                doc.content.with_untracked(|c| c.path().cloned())
                            } else {
                                None
                            } {
                                Some(path) => path,
                                None => return Ok(()),
                            };
                            let offset = editor_data.cursor().with_untracked(|c| c.offset());
                            let internal_command = self.common.internal_command;

                            internal_command.send(InternalCommand::MakeConfirmed);
                            internal_command.send(InternalCommand::GoToLocation {
                                location: EditorLocation {
                                    path,
                                    position: Some(EditorPosition::Offset(offset)),
                                    scroll_offset: None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab: false,
                                }
                            });
                        }
                    }
            AddRunDebugConfig => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            editor_data.receive_char(DEFAULT_RUN_TOML);
                        }
                    }
            OpenRunAndDebugFile => {
                        if let Some(path) = self.workspace.run_and_debug_path()? {
                            self.common
                                .internal_command
                                .send(InternalCommand::OpenFile { path });
                        }
                    }
            InspectSemanticType => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            let offset = editor_data.cursor().with_untracked(|x| x.offset());
                            let Some((word, semantic)) = editor_data.doc().lines.with_untracked(|x| {
                                if let Some((_, styles)) = &x.semantic_styles {
                                    let semantic = styles.iter().find_map(|x| {
                                        if x.0.contains(offset) {
                                            Some(x.1.clone())
                                        } else {
                                            None
                                        }
                                    })?;
                                    let (start, end) = x.buffer().select_word(offset);
                                    Some((x.buffer().slice_to_cow(start..end).to_string(), semantic))
                                } else {
                                    None
                                }
                            }) else {
                                return Ok(())
                            };
                            // log::debug!("{word} {semantic}");
                            self.show_message(&word, &ShowMessageParams {
                                typ: MessageType::INFO,
                                message: semantic,
                            });
                        }
                    }
            InspectLogModule => {
                        if let Some(editor_data) =
                            self.main_split.active_editor.get_untracked()
                        {
                            if let Some(path) = editor_data.doc.get_untracked().content.get_untracked().path() {
                                self.proxy.proxy_rpc.find_log_modules(path.to_path_buf(), find_log_modules_call_back(self.scope, self.common.internal_command));
                            } else {
                                self.common.internal_command.send(
                                        InternalCommand::ShowStatusMessage {
                                            message: "not a file".to_string(),
                                        },
                                    )
                            }
                        }
                    }
            InspectClickInfo => {
                            // log::debug!("{word} {semantic}");
                            self.show_message("InspectClickInfo", &ShowMessageParams {
                                typ: MessageType::INFO,
                                message: self.common.inspect_info.get_untracked(),
                            });
                    }
        }

        Ok(())
    }

    pub fn run_internal_command(&self, cmd: InternalCommand) -> Result<()> {
        let cx = self.scope;
        match cmd {
            InternalCommand::ReloadConfig => {
                                        self.reload_config();
                                    }
            InternalCommand::UpdateLogLevel { level } => {
                                        // TODO: implement logging panel, runtime log level change
                                        debug!("{level}");
                                    }
            InternalCommand::MakeConfirmed => {
                                        if let Some(tab_id) = self.main_split.active_editor_tab.get_untracked() && let Some(confirmed) = self.main_split.editor_tabs.with_untracked(|x| {
                                                x.get(&tab_id).map(|data| {
                                                    data.with_untracked(|manage| {
                                                        manage.active_child().confirmed_mut()
                                                    })
                                                })
                                            }) {
                                                confirmed.set(true);
                                        }
                                        // if let Some(editor) = self.main_split.active_editor.get_untracked() {
                                        //     editor.confirmed.set(true);
                                        // }
                                    }
            InternalCommand::OpenFile { path } => {
                                        self.main_split.jump_to_location(
                                            EditorLocation {
                                                path,
                                                position: None,
                                                scroll_offset: None,
                                                ignore_unconfirmed: false,
                                                same_editor_tab: false,
                                            },
                                            None,
                                        );
                                    }
            InternalCommand::OpenAndConfirmedFile { path } => {
                                        self.main_split.jump_to_location(
                                            EditorLocation {
                                                path,
                                                position: None,
                                                scroll_offset: None,
                                                ignore_unconfirmed: false,
                                                same_editor_tab: false,
                                            },
                                            None,
                                        );
                                        if let Some(tab_id) = self.main_split.active_editor_tab.get_untracked() && let Some(confirmed) = self.main_split.editor_tabs.with_untracked(|x| {
                                                x.get(&tab_id).map(|data| {
                                                    data.with_untracked(|manage| {
                                                        manage.active_child().confirmed_mut()
                                                    })
                                                })
                                            }) {
                                                confirmed.set(true);
                                        }
                                        // if let Some(editor) = self.main_split.active_editor.get_untracked() {
                                        //     editor.confirmed.set(true);
                                        // }
                                    }
            InternalCommand::OpenFileInNewTab { path } => {
                                        self.main_split.jump_to_location(
                                            EditorLocation {
                                                path,
                                                position: None,
                                                scroll_offset: None,
                                                ignore_unconfirmed: true,
                                                same_editor_tab: false,
                                            },
                                            None,
                                        );
                                    }
            InternalCommand::OpenFileChanges { path } => {
                                        self.main_split.open_file_changes(path);
                                    }
            InternalCommand::ReloadFileExplorer => {
                                        self.file_explorer.reload();
                                    }
            InternalCommand::TestPathCreation { new_path } => {
                                        let naming = self.file_explorer.naming;

                                        let send = create_ext_action(
                                            self.scope,
                                            move |(_, response): (u64, Result<ProxyResponse, RpcError>)| {
                                                match response {
                                                    Ok(_) => {
                                                        naming.update(Naming::set_ok);
                                                    }
                                                    Err(err) => {
                                                        naming.update(|naming| naming.set_err(err.message));
                                                    }
                                                }
                                            },
                                        );

                                        self.common.proxy.proxy_rpc.test_create_at_path(new_path, send);
                                    }
            InternalCommand::FinishRenamePath {
                                        current_path,
                                        new_path
                                    } => {
                                        let send_current_path = current_path.clone();
                                        let send_new_path = new_path.clone();
                                        let file_explorer = self.file_explorer.clone();
                                        let editors = self.main_split.editors;

                                        let send = create_ext_action(
                                            self.scope,
                                            move |(_, response): (u64, Result<ProxyResponse, RpcError>)| {
                                                match response {
                                                    Ok(response) => {
                                                        // Get the canonicalized new path from the proxy.
                                                        let new_path =
                                                            if let ProxyResponse::CreatePathResponse {
                                                                path
                                                            } = response
                                                            {
                                                                path
                                                            } else {
                                                                send_new_path
                                                            };

                                                        // If the renamed item is a file, update any editors
                                                        // the file is open
                                                        // in to use the new path.
                                                        // If the renamed item is a directory, update any
                                                        // editors in which a
                                                        // file the renamed directory is an ancestor of is
                                                        // open to use the
                                                        // file's new path.
                                                        let renamed_editors_content: Vec<_> = editors
                                                            .with_editors_untracked(|editors| {
                                                                editors
                                                                    .values()
                                                                    .map(|editor| editor.doc().content)
                                                                    .filter(|content| {
                                                                        content.with_untracked(|content| {
                                                                            match content {
                                                                                DocContent::File {
                                                                                    path,
                                                                                    ..
                                                                                } => path.starts_with(
                                                                                    &send_current_path
                                                                                ),
                                                                                _ => false
                                                                            }
                                                                        })
                                                                    })
                                                                    .collect()
                                                            });

                                                        for content in renamed_editors_content {
                                                            content.update(|content| {
                                                                if let DocContent::File { path, .. } =
                                                                    content
                                                                && let Ok(suffix) =
                                                                        path.strip_prefix(&send_current_path)
                                                                    {
                                                                        *path = new_path.join(suffix);
                                                                }
                                                            });
                                                        }

                                                        file_explorer.reload();
                                                        file_explorer.naming.set(Naming::None);
                                                    }
                                                    Err(err) => {
                                                        file_explorer
                                                            .naming
                                                            .update(|naming| naming.set_err(err.message));
                                                    }
                                                }
                                            },
                                        );

                                        self.file_explorer.naming.update(Naming::set_pending);
                                        self.common
                                            .proxy
                                            .proxy_rpc.rename_path(current_path.clone(), new_path, send);
                                    }
            InternalCommand::FinishNewNode { is_dir, path } => {
                                        let file_explorer = self.file_explorer.clone();
                                        let internal_command = self.common.internal_command;

                                        let send = create_ext_action(
                                            self.scope,
                                            move |(_id, response): (
                                                u64,
                                                Result<ProxyResponse, RpcError>
                                            )| {
                                                match response {
                                                    Ok(response) => {
                                                        file_explorer.reload();
                                                        file_explorer.naming.set(Naming::None);

                                                        // Open a new file in the editor
                                                        if let ProxyResponse::CreatePathResponse { path } =
                                                            response
                                                        && !is_dir {
                                                                internal_command.send(
                                                                    InternalCommand::OpenFile { path }
                                                                );
                                                        }
                                                    }
                                                    Err(err) => {
                                                        file_explorer
                                                            .naming
                                                            .update(|naming| naming.set_err(err.message));
                                                    }
                                                }
                                            },
                                        );

                                        self.file_explorer.naming.update(Naming::set_pending);
                                        if is_dir {
                                            self.common.proxy.proxy_rpc.create_directory(path, send);
                                        } else {
                                            self.common.proxy.proxy_rpc.create_file(path, send);
                                        }
                                    }
            InternalCommand::FinishDuplicate { source, path } => {
                                        let file_explorer = self.file_explorer.clone();

                                        let send = create_ext_action(
                                            self.scope,
                                            move |(_id, response): (
                                                u64,
                                                Result<ProxyResponse, RpcError>
                                            )| {
                                                if let Err(err) = response {
                                                    file_explorer
                                                        .naming
                                                        .update(|naming| naming.set_err(err.message));
                                                } else {
                                                    file_explorer.reload();
                                                    file_explorer.naming.set(Naming::None);
                                                }
                                            },
                                        );

                                        self.file_explorer.naming.update(Naming::set_pending);
                                        self.common.proxy.proxy_rpc.duplicate_path(source, path, send);
                                    }
            InternalCommand::GoToLocation { location } => {
                                        if let Err(err) = self.main_split.go_to_location(location, None) {
                                            error!("{err:?}");
                                        }
                                    }
            InternalCommand::JumpToLocation { location } => {
                                        self.main_split.jump_to_location(location, None);
                                    }
            InternalCommand::PaletteReferences { references } => {
                                        self.palette.references.set(references);
                                        self.palette.run(PaletteKind::Reference);
                                    }
            InternalCommand::Split {
                                        direction,
                                        editor_tab_id
                                    } => {
                                        self.main_split.split(direction, editor_tab_id);
                                    }
            InternalCommand::SplitMove {
                                        direction,
                                        editor_tab_id
                                    } => {
                                        self.main_split.split_move(direction, editor_tab_id);
                                    }
            InternalCommand::SplitExchange { editor_tab_id } => {
                                        self.main_split.split_exchange(editor_tab_id);
                                    }
            InternalCommand::EditorTabClose { editor_tab_id } => {
                                        self.main_split.editor_tab_close(editor_tab_id);
                                    }
            InternalCommand::EditorTabChildClose {
                                        editor_tab_id,
                                        child
                                    } => {
                                        self.main_split
                                            .editor_tab_child_close(editor_tab_id, child, false);
                                    }
            InternalCommand::EditorTabCloseByKind {
                                        editor_tab_id,
                                        child,
                                        kind
                                    } => {
                                        self.main_split.editor_tab_child_close_by_kind(
                                            editor_tab_id,
                                            child,
                                            kind,
                                        );
                                    }
            InternalCommand::ShowCodeActions {
                                        offset,
                                        mouse_click,
                                        plugin_id,
                                        code_actions
                                    } => {
                                        let mut code_action = self.code_action.get_untracked();
                                        code_action.show(plugin_id, code_actions, offset, mouse_click);
                                        self.code_action.set(code_action);
                                    }
            InternalCommand::RunCodeAction { plugin_id, action } => {
                                        self.main_split.run_code_action(plugin_id, action);
                                    }
            InternalCommand::ApplyWorkspaceEdit { edit } => {
                                        self.main_split.apply_workspace_edit(&edit);
                                    }
            InternalCommand::SaveJumpLocation {
                                        path,
                                        offset,
                                        scroll_offset
                                    } => {
                                        self.main_split
                                            .save_jump_location(path, offset, scroll_offset);
                                    }
            InternalCommand::StartRename {
                                        path,
                                        placeholder,
                                        position,
                                        start
                                    } => {
                                        self.rename.start(path, placeholder, start, position);
                                    }
            InternalCommand::Search { pattern } => {
                                        self.main_split.set_find_pattern(pattern);
                                    }
            InternalCommand::FindEditorReceiveChar { s } => {
                                        error!("FindEditorReceiveChar {s}");
                                        // self.main_split.find_editor.receive_char(&s);
                                        // self.main_split.find_str.update(|x| {
                                        //     x.push_str(&s);
                                        // });
                                    }
            InternalCommand::ReplaceEditorReceiveChar { s } => {
                                        error!("ReplaceEditorReceiveChar {s}");
                                        //
                                        // self.main_split.replace_editor.receive_char(&s);
                                    }
            InternalCommand::FindEditorCommand {
                                        command, ..
                                        // count,
                                        // mods
                                    } => {
                                        log::error!("todo FindEditorCommand {command:?}");
                                        // self.main_split
                                        //     .find_editor
                                        //     .run_command(&command, count, mods);
                                    }
            InternalCommand::ReplaceEditorCommand {
                                        command, ..
                                    } => {
                                        log::error!("todo ReplaceEditorCommand {command:?}");
                                        //
                                        // self.main_split
                                        //     .replace_editor
                                        //     .run_command(&command, count, mods);
                                    }
            InternalCommand::FocusEditorTab { editor_tab_id } => {
                                        self.main_split.active_editor_tab.set(Some(editor_tab_id));
                                    }
            InternalCommand::SetColorTheme { name, save } => {
                                        if save {
                                            // The config file is watched
                                            LapceConfig::update_file(
                                                "core",
                                                "color-theme",
                                                toml_edit::Value::from(name), self.common.clone(),
                                            );
                                        } else {
                                            let mut new_config = self.common.config.get_untracked();
                                            new_config.set_color_theme(&self.workspace, &name, &self.common.directory.config_directory);
                                            self.set_config.set(new_config);
                                        }
                                    }
            InternalCommand::SetIconTheme { name, save } => {
                                        if save {
                                            // The config file is watched
                                            LapceConfig::update_file(
                                                "core",
                                                "icon-theme",
                                                toml_edit::Value::from(name), self.common.clone(),
                                            );
                                        } else {
                                            let mut new_config = self.common.config.get_untracked();
                                            new_config
                                                .set_icon_theme(&self.workspace, &name, &self.common.directory.config_directory);
                                            self.set_config.set(new_config);
                                        }
                                    }
            InternalCommand::SetModal { modal } => {
                                        LapceConfig::update_file(
                                            "core",
                                            "modal",
                                            toml_edit::Value::from(modal), self.common.clone(),
                                        );
                                    }
            InternalCommand::OpenWebUri { uri } => {
                                        if !uri.is_empty() {
                                            match open::that(&uri) {
                                                Ok(_) => {
                                                    trace!("opened web uri: {uri:?}");
                                                }
                                                Err(e) => {
                                                    trace!("failed to open web uri: {uri:?}, error: {e}");
                                                }
                                            }
                                        }
                                    }
            InternalCommand::ShowAlert {
                                        title,
                                        msg,
                                        buttons
                                    } => {
                                        self.show_alert(title, msg, buttons);
                                    }
            InternalCommand::HideAlert => {
                                        self.alert_data.active.set(false);
                                    }
            InternalCommand::SaveScratchDoc { doc } => {
                                        self.main_split.save_scratch_doc(doc);
                                    }
            InternalCommand::SaveScratchDoc2 { doc } => {
                                        self.main_split.save_scratch_doc2(doc);
                                    }
            InternalCommand::UpdateProxyStatus { status } => {
                                        self.common.proxy_status.set(Some(status));
                                    }
            InternalCommand::DapFrameScopes { dap_id, frame_id } => {
                                        self.terminal.dap_frame_scopes(dap_id, frame_id);
                                    }
            InternalCommand::OpenVoltView { volt_id } => {
                                        self.main_split.save_current_jump_location();
                                        self.main_split.open_volt_view(volt_id);
                                    }
            InternalCommand::ResetBlinkCursor => {
                                        // All the editors share the blinking information and logic, so we
                                        // can just reset one of them.
                                        self.cursor_blink.blink_right_now();
                                        // if let Some(e_data) = self.main_split.active_editor.get_untracked() {
                                        //     // e_data.editor.cursor_info.reset();
                                        // }
                                    }
            InternalCommand::BlinkCursor => {
                                        // All the editors share the blinking information and logic, so we
                                        // can just reset one of them.
                                        match self.common.focus.get_untracked() {
                                            Focus::Panel(PanelKind::Terminal) => {
                                                if let Some(tab) = self.terminal.active_tab_untracked() && let Some(id) = tab.data.with_untracked(|x| x.view_id) {
                                                        // log::debug!("BlinkCursor Terminal {:?}", id.data().as_ffi());
                                                        id.request_paint();
                                                }
                                            }
                                            Focus::Workbench => {
                                                if let Some(e_data) = self.main_split.active_editor.get_untracked() && let Some(id) = e_data.editor_view_id.get_untracked() {
                                                        id.request_paint();
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
            InternalCommand::OpenDiffFiles {
                                        left_path,
                                        right_path
                                    } => self.main_split.open_diff_files(left_path, right_path),
            InternalCommand::ExecuteProcess { program, arguments } => {
                                        let mut cmd = std::process::Command::new(program)
                                            .args(arguments)
                                            .spawn()?;
                                        // {
                                        //     Ok(v) => v,
                                        //     Err(e) => return error!("Failed to spawn process: {e}")
                                        // };
                cmd.wait()?;
                                        // match  {
                                        //     Ok(v) => trace!("Process exited with status {v}"),
                                        //     Err(e) => {
                                        //         error!("Proces exited with an error: {e}")
                                        //     }
                                        // };
                                    }
            InternalCommand::ClearTerminalBuffer {
                                        view_id,
                                        terminal_id
                                    } => {
                                        let Some(tab) = self.terminal.tab_infos.with_untracked(|x| {
                                            x.tabs.iter().find_map(|data| {
                                                if data.term_id == terminal_id {
                                                    Some(data.clone())
                                                } else {
                                                    None
                                                }
                                            })
                                        }) else {
                                            bail!(
                                                "cound not find terminal tab data: terminal_id={terminal_id:?}"
                                            );
                                        };
                                        tab.data.update(|x| x.raw.term.reset_state());
                                        view_id.request_paint();
                                    }
            InternalCommand::StopTerminal { terminal_id } => {
                                        self.terminal.manual_stop_run_debug(terminal_id);
                                    }
            InternalCommand::RestartTerminal { terminal_id } => {
                                        if let Err(err) = self.restart_run_program_in_terminal(terminal_id) {
                                            error!("RestartTerminal {err}");
                                        }
                                    }
            InternalCommand::NewTerminal { profile } => {
                                        self.terminal.new_tab(profile);
                                    }
            InternalCommand::RunAndDebug { mode, mut config } => {
                                        if let Some(workspace) = self.workspace.path() {
                                            config.update_by_workspace(workspace.to_string_lossy().as_ref());
                                        }
                                        self.run_and_debug(cx, mode, config);
                                    }
            InternalCommand::CallHierarchyIncoming { item_id, root_id } => {
                                        self.call_hierarchy_incoming(root_id, item_id);
                                    }
            InternalCommand::DocumentHighlight => {
                                        if let Some(e_data) = self.main_split.active_editor.get_untracked() && let Err(err) = e_data.document_highlight(self.clone()) {
                                                error!("DocumentHighlight {err}");
                                        }
                                    }
            InternalCommand::AddOrRemoveBreakPoint { doc, line_num } =>  {
                                let (offset, content) =  doc.with_untracked(|x| {
                                    (x.lines.with_untracked(|x| x.buffer().offset_of_line(line_num)),
                                    x.content.get_untracked())
                                });
                                    let offset = offset?;
                                if let Some(path) = content.path() {
                                    let breakpoints = self.common.breakpoints;
                                let proxy = self.common.proxy.proxy_rpc.clone();
                                let daps = self.terminal.debug.daps;
                                update_breakpoints(daps, proxy, breakpoints, lapce_core::debug::BreakpointAction::AddOrRemove { path, line: line_num, offset  });
                                }
                            },
            InternalCommand::ShowStatusMessage { message } => self.show_status_message(message),
            InternalCommand::JumpToMaybeRelativeLocation { location } => {
                let path = location.relative_path.clone();
                let common = self.common.clone();
                let send = create_ext_action(self.scope, move |result| match result {
                Ok(resp) => {
                    if let ProxyResponse::GetAbsolutePathResponse { path } = resp && let Some(real_path) = path {
                        common.internal_command.send(
                                InternalCommand::JumpToLocation {
                                    location: EditorLocation {
                                        path:               real_path,
                                        position:           location.position,
                                        scroll_offset:      location.scroll_offset,
                                        ignore_unconfirmed: location.ignore_unconfirmed,
                                        same_editor_tab:    location.same_editor_tab,
                                    },
                                },
                            );
                    } else {
                        common.show_status_message(format!("get absolute path fail: {:?}", location.relative_path));
                    }
                },
                Err(err) => error!("{err}"),
            });
            self.proxy.proxy_rpc.get_absolute_path(path, move |(_, result)| {
                    send(result);
                });
            },
        }
        Ok(())
    }

    fn handle_core_notification(&self, rpc: &CoreNotification) {
        let cx = self.scope;
        match rpc {
            CoreNotification::ProxyStatus { status } => {
                self.common.proxy_status.set(Some(status.to_owned()));
            },
            CoreNotification::DiffInfo { diff } => {
                self.source_control.branch.set(diff.head.clone());
                self.source_control
                    .branches
                    .set(diff.branches.iter().cloned().collect());
                self.source_control
                    .tags
                    .set(diff.tags.iter().cloned().collect());
                self.source_control.file_diffs.update(|file_diffs| {
                    *file_diffs = diff
                        .diffs
                        .iter()
                        .cloned()
                        .map(|diff| {
                            let checked =
                                file_diffs.get(diff.path()).is_none_or(|(_, c)| *c);
                            (diff.path().clone(), (diff, checked))
                        })
                        .collect();
                });

                let docs = self.main_split.docs.get_untracked();
                for (_, doc) in docs {
                    doc.retrieve_head();
                }
            },
            CoreNotification::CompletionResponse {
                request_id,
                input,
                resp,
                plugin_id,
            } => {
                self.common.completion.update(|completion| {
                    completion.receive(*request_id, input, resp, *plugin_id);
                });

                let completion = self.common.completion.get_untracked();
                let editor_data = completion
                    .latest_editor_id
                    .and_then(|id| self.main_split.editors.editor_untracked(id));
                if let Some(editor_data) = editor_data {
                    let cursor_offset =
                        editor_data.cursor().with_untracked(|c| c.offset());
                    completion
                        .update_document_completion(&editor_data, cursor_offset);
                }
            },
            CoreNotification::PublishDiagnostics {
                diagnostics: diagnostic_params,
            } => {
                let path = path_from_url(&diagnostic_params.uri);
                let diagnostics: im::Vector<Diagnostic> = diagnostic_params
                    .diagnostics
                    .clone()
                    .into_iter()
                    .sorted_by_key(|d| d.range.start)
                    .collect();

                // for diag in &diagnostics {
                //     if let Some(Value::Object(data)) = &diag.data {
                //         if let Some(Value::String(rendered)) =
                // data.get("rendered") {             log::error!(
                //                 "contains_ansi {} {}",
                //                 contains_ansi(rendered),
                //                 rendered
                //             );
                //         }
                //     }
                //     error!("{:?}", diag.data);
                // }
                log::debug!("PublishDiagnostics {path:?} {}", diagnostics.len());
                let diag = self.main_split.get_diagnostic_data(&path);
                let old_len = diag.diagnostics.with_untracked(|x| x.len());
                let task_id = diag.id.with_untracked(|x| {
                    x.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    x.load(std::sync::atomic::Ordering::Relaxed)
                });
                if diagnostics.is_empty() && old_len > 0 {
                    let time = match old_len {
                        1 => 600u64,
                        _ => 2000,
                    };
                    let docs = self.main_split.docs;
                    exec_after(Duration::from_millis(time), move |_| {
                        let now_id = diag.id.with_untracked(|x| {
                            x.load(std::sync::atomic::Ordering::Relaxed)
                        });
                        if now_id == task_id {
                            debug!(
                                "PublishDiagnostics equal exec_after {path:?} \
                                 {now_id}={task_id}",
                            );
                            diag.diagnostics.set(diagnostics);
                            let doc_content = DocContent::File {
                                path:      path.clone(),
                                read_only: false,
                            };
                            if let Some(doc) = docs.with_untracked(|docs| {
                                docs.get(&doc_content).cloned()
                            }) {
                                // warn!("PublishDiagnostics docs {:?}", path);
                                doc.init_diagnostics();
                            }
                        } else {
                            debug!(
                                "PublishDiagnostics exec_after {path:?} \
                                 now_id={now_id} id={task_id}",
                            );
                        }
                    });
                    return;
                }

                diag.diagnostics.set(diagnostics);

                let doc_content = DocContent::File {
                    path:      path.clone(),
                    read_only: false,
                };

                // inform the document about the diagnostics
                if let Some(doc) = self
                    .main_split
                    .docs
                    .with_untracked(|docs| docs.get(&doc_content).cloned())
                {
                    // warn!("PublishDiagnostics docs {:?}", path);
                    doc.init_diagnostics();
                }
            },
            CoreNotification::ServerStatus { params } => {
                if params.is_ok() {
                    // todo filter by language
                    self.main_split.docs.with_untracked(|x| {
                        for doc in x.values() {
                            if doc.content.get_untracked().is_local() {
                                continue;
                            }
                            doc.get_code_lens();
                            doc.get_document_symbol();
                            doc.get_semantic_styles();
                            doc.get_folding_range();
                            doc.get_inlay_hints();
                        }
                    });
                }
            },
            CoreNotification::TerminalProcessStopped { term_id, exit_code } => {
                debug!("TerminalProcessStopped {:?}, {:?}", term_id, exit_code);
                self.terminal.terminal_stopped(term_id, *exit_code, false);
                if self
                    .terminal
                    .tab_infos
                    .with_untracked(|info| info.tabs.is_empty())
                {
                    if self.panel.is_panel_visible(&PanelKind::Terminal) {
                        self.panel.hide_panel(&PanelKind::Terminal);
                    }
                    self.common.focus.set(Focus::Workbench);
                }
            },
            CoreNotification::TerminalProcessStoppedByDap { term_id, exit_code } => {
                debug!("TerminalProcessStoppedByDap {:?}, {:?}", term_id, exit_code);
                self.terminal.terminal_stopped(term_id, *exit_code, true);
                if self
                    .terminal
                    .tab_infos
                    .with_untracked(|info| info.tabs.is_empty())
                {
                    if self.panel.is_panel_visible(&PanelKind::Terminal) {
                        self.panel.hide_panel(&PanelKind::Terminal);
                    }
                    self.common.focus.set(Focus::Workbench);
                }
            },
            CoreNotification::TerminalUpdateContent { term_id, content } => {
                self.terminal.terminal_update_content(term_id, content);
                if self
                    .terminal
                    .tab_infos
                    .with_untracked(|info| info.tabs.is_empty())
                {
                    if self.panel.is_panel_visible(&PanelKind::Terminal) {
                        self.panel.hide_panel(&PanelKind::Terminal);
                    }
                    self.common.focus.set(Focus::Workbench);
                }
            },
            CoreNotification::TerminalSetTitle { term_id, title } => {
                debug!("TerminalSetTitle {term_id:?}");
                self.terminal.set_title(term_id, title);
            },
            CoreNotification::TerminalRequestPaint => {
                debug!("TerminalRequestPaint");
                self.terminal.request_paint()
            },
            CoreNotification::TerminalLaunchFailed { term_id, error } => {
                self.terminal.launch_failed(term_id, error);
            },
            CoreNotification::DapRunInTerminal { config } => {
                let dap_id = config.dap_id;
                if let Some(dap_data) = self
                    .terminal
                    .debug
                    .daps
                    .with_untracked(|x| x.get(&dap_id).cloned())
                {
                    let Some(term_id) = dap_data.term_id else {
                        self.run_program_in_terminal(
                            cx,
                            &RunDebugMode::Debug,
                            config,
                            true,
                        );
                        return;
                    };
                    if let Some(terminal) = self.terminal.get_terminal(term_id) {
                        let Some(origin_config) =
                            terminal.data.with_untracked(|x| {
                                x.run_debug.as_ref().map(|x| x.origin_config.clone())
                            })
                        else {
                            error!("no found terminal {term_id:?}");
                            return;
                        };
                        debug!("{:?}", origin_config);
                        terminal.new_process(Some(RunDebugProcess {
                            mode: RunDebugMode::Debug,
                            origin_config,
                            config: config.clone(),
                            stopped: false,
                            created: Instant::now(),
                            is_prelaunch: false,
                        }));
                    } else {
                        error!("no found terminal {term_id:?}");
                    }
                } else {
                    error!("no found dap_data {dap_id:?}");
                }
            },
            CoreNotification::TerminalProcessId {
                term_id,
                process_id,
            } => {
                self.terminal.set_process_id(term_id, *process_id);
            },
            CoreNotification::DapStopped {
                dap_id,
                stopped,
                stack_frames,
                variables,
            } => {
                self.show_panel(PanelKind::Debug);
                self.terminal
                    .dap_stopped(dap_id, stopped, stack_frames, variables);
            },
            CoreNotification::OpenPaths { paths } => {
                self.open_paths(paths);
            },
            CoreNotification::DapContinued { dap_id } => {
                self.terminal.dap_continued(dap_id);
            },
            CoreNotification::DapBreakpointsResp {
                path, breakpoints, ..
            } => {
                log::debug!("DapBreakpointsResp {path:?} {breakpoints:?}");
                self.terminal
                    .common
                    .breakpoints
                    .update_by_dap_resp(path, breakpoints);
            },
            CoreNotification::OpenFileChanged { path, content } => {
                self.main_split.open_file_changed(path, content);
            },
            CoreNotification::VoltInstalled { volt, icon } => {
                self.plugin.volt_installed(volt, icon);
            },
            CoreNotification::VoltRemoved { volt, .. } => {
                self.plugin.volt_removed(volt);
            },
            CoreNotification::WorkDoneProgress { progress } => {
                self.update_progress(progress);
            },
            CoreNotification::ShowStatusMessage { message } => {
                self.show_status_message(message.clone());
            },
            CoreNotification::ShowMessage { title, message } => {
                self.show_message(title, message);
            },
            CoreNotification::Log {
                level,
                message,
                target,
            } => {
                use lapce_rpc::core::LogLevel;
                use log::{Level, log};

                let target = target.clone().unwrap_or(String::from("unknown"));

                match level {
                    LogLevel::Trace => {
                        log!(target: &target, Level::Trace, "{}", message);
                    },
                    LogLevel::Debug => {
                        log!(target: &target, Level::Debug, "{}", message);
                    },
                    LogLevel::Info => {
                        log!(target: &target, Level::Info, "{}", message);
                    },
                    LogLevel::Warn => {
                        log!(target: &target, Level::Warn, "{}", message);
                    },
                    LogLevel::Error => {
                        log!(target: &target, Level::Error, "{}", message);
                    },
                }
            },
            CoreNotification::LogMessage { message, target } => {
                use lsp_types::MessageType;
                match message.typ {
                    MessageType::ERROR => {
                        error!("{} {}", target, message.message)
                    },
                    MessageType::WARNING => {
                        warn!("{} {}", target, message.message)
                    },
                    MessageType::INFO => {
                        debug!("{} {}", target, message.message)
                    },
                    MessageType::DEBUG => {
                        debug!("{} {}", target, message.message)
                    },
                    MessageType::LOG => {
                        trace!("{} {}", target, message.message)
                    },
                    _ => {},
                }
            },
            CoreNotification::WorkspaceFileChange => {
                self.file_explorer.reload();
            },
            _ => {},
        }
    }

    pub fn show_status_message(&self, message: String) {
        let msg = WorkDoneProgressBegin {
            title:       message,
            cancellable: None,
            message:     None,
            percentage:  None,
        };
        let token =
            NumberOrString::String(format!("StatusMessage {}", Id::next().to_raw()));
        let end_token = token.clone();
        let progress = ProgressParams {
            token,
            value: lsp_types::ProgressParamsValue::WorkDone(
                WorkDoneProgress::Begin(msg),
            ),
        };
        self.update_progress(&progress);
        let workspace = self.clone();
        exec_after(Duration::from_secs(10), move |_| {
            let progress = ProgressParams {
                token: end_token,
                value: lsp_types::ProgressParamsValue::WorkDone(
                    WorkDoneProgress::End(WorkDoneProgressEnd { message: None }),
                ),
            };
            workspace.update_progress(&progress);
        });
    }

    pub fn key_down<'a>(&self, event: impl Into<EventRef<'a>> + Copy) -> bool {
        if self.alert_data.active.get_untracked() {
            return false;
        }
        let focus = self.common.focus.get_untracked();
        let keypress = self.common.keypress.get_untracked();
        log::debug!("key_down {:?}", focus);
        let handle = match focus {
            Focus::Workbench => self.main_split.key_down(event, &keypress),
            Focus::Palette => Some(keypress.key_down(event, &self.palette)),
            Focus::CodeAction => {
                let code_action = self.code_action.get_untracked();
                Some(keypress.key_down(event, &code_action))
            },
            Focus::Rename => Some(keypress.key_down(event, &self.rename)),
            Focus::AboutPopup => Some(keypress.key_down(event, &self.about_data)),
            Focus::Panel(PanelKind::Terminal) => {
                self.terminal.key_down(event, &keypress)
            },
            Focus::Panel(PanelKind::Search) => {
                Some(keypress.key_down(event, &self.global_search))
            },
            Focus::Panel(PanelKind::Plugin) => {
                Some(keypress.key_down(event, &self.plugin))
            },
            Focus::Panel(PanelKind::SourceControl) => {
                Some(keypress.key_down(event, &self.source_control))
            },
            _ => None,
        };

        if let Some(handle) = handle {
            if handle.handled {
                true
            } else {
                keypress
                    .handle_keymatch(self, handle.keymatch, handle.keypress)
                    .handled
            }
        } else {
            keypress.key_down(event, self).handled
        }
    }

    pub fn workspace_info(&self) -> WorkspaceInfo {
        let main_split_data = self
            .main_split
            .splits
            .get_untracked()
            .get(&self.main_split.root_split)
            .cloned()
            .unwrap();
        WorkspaceInfo {
            split:       main_split_data.get_untracked().split_info(self),
            panel:       self.panel.panel_info(),
            breakpoints: self.terminal.common.breakpoints.clone_for_hashmap(),
        }
    }

    pub fn hover_origin(&self) -> Option<Point> {
        if !self.common.hover.active.get_untracked() {
            return None;
        }

        let editor_id = self.common.hover.editor_id.get_untracked();
        let editor_data = self.main_split.editors.editor(editor_id)?;

        let window_origin = editor_data.window_origin();
        let viewport = editor_data.viewport;

        let hover_offset = self.common.hover.offset.get_untracked();
        // TODO(minor): affinity should be gotten from where the hover was started
        // at.
        let (point_above, point_below) =
            editor_data.points_of_offset(hover_offset).ok()?;

        let window_origin =
            window_origin.get() - self.common.window_origin.get().to_vec2();
        let viewport = viewport.get();
        let hover_size = self.common.hover.layout_rect.get().size();
        let layout_rect = self.layout_rect.get().size();
        let offset = 4.0;

        log::debug!(
            "hover_origin hover_offset={hover_offset} point_above={point_above:?} \
             point_below={point_below:?} viewport={viewport:?} \
             window_origin={window_origin:?}"
        );
        // top right corner of word
        let mut origin = window_origin
            + Vec2::new(
                point_below.x - viewport.x0 - offset,
                (point_above.y - viewport.y0) - hover_size.height + offset,
            );
        if origin.y < 0.0 {
            // bottom right corner of word
            origin.y = window_origin.y + point_below.y - viewport.y0 - offset;
        }
        if origin.x + hover_size.width + 1.0 > layout_rect.width {
            // On the far left side within the window.
            origin.x = layout_rect.width - hover_size.width - 1.0;
        }
        if origin.x <= 0.0 {
            origin.x = 0.0;
        }

        Some(origin)
    }

    pub fn completion_origin(&self) -> Result<Point> {
        let completion = self.common.completion.get();
        if completion.status == CompletionStatus::Inactive {
            return Ok(Point::ZERO);
        }
        let line_height = self.common.ui_line_height.get();
        let editor_data =
            if let Some(editor) = self.main_split.active_editor.get_untracked() {
                editor
            } else {
                return Ok(Point::ZERO);
            };

        let (window_origin, viewport) =
            (editor_data.window_origin(), editor_data.viewport);

        // TODO(minor): What affinity should we use for this? Probably just use the
        // cursor's original affinity..
        let (point_above, point_below) =
            editor_data.points_of_offset(completion.offset)?;

        let window_origin =
            window_origin.get() - self.common.window_origin.get().to_vec2();
        let viewport = viewport.get();
        let completion_size = completion.layout_rect.size();
        let tab_size = self.layout_rect.get().size();

        let mut origin = window_origin
            + Vec2::new(
                point_below.x - viewport.x0 - line_height - 5.0,
                point_below.y - viewport.y0,
            );
        if origin.y + completion_size.height > tab_size.height {
            origin.y = window_origin.y + (point_above.y - viewport.y0)
                - completion_size.height;
        }
        if origin.x + completion_size.width + 1.0 > tab_size.width {
            origin.x = tab_size.width - completion_size.width - 1.0;
        }
        if origin.x <= 0.0 {
            origin.x = 0.0;
        }

        Ok(origin)
    }

    pub fn code_action_origin(&self) -> Result<Point> {
        let code_action = self.code_action.get();
        let line_height = self.common.ui_line_height.get();
        if code_action.status.get_untracked() == CodeActionStatus::Inactive {
            return Ok(Point::ZERO);
        }

        let tab_size = self.layout_rect.get().size();
        let code_action_size = code_action.layout_rect.size();

        let Some(editor_data) = self.main_split.active_editor.get_untracked() else {
            return Ok(Point::ZERO);
        };

        let (window_origin, viewport) =
            (editor_data.window_origin(), editor_data.viewport);

        // TODO(minor): What affinity should we use for this?
        let (_point_above, point_below) =
            editor_data.points_of_offset(code_action.offset)?;

        let window_origin =
            window_origin.get() - self.common.window_origin.get().to_vec2();
        let viewport = viewport.get();

        let mut origin = window_origin
            + Vec2::new(
                if code_action.mouse_click {
                    0.0
                } else {
                    point_below.x - viewport.x0
                },
                point_below.y - viewport.y0,
            );

        if origin.y + code_action_size.height > tab_size.height {
            origin.y = origin.y - line_height - code_action_size.height;
        }
        if origin.x + code_action_size.width + 1.0 > tab_size.width {
            origin.x = tab_size.width - code_action_size.width - 1.0;
        }
        if origin.x <= 0.0 {
            origin.x = 0.0;
        }

        Ok(origin)
    }

    pub fn rename_origin(&self) -> Result<Point> {
        let line_height = self.common.ui_line_height.get();
        if !self.rename.active.get() {
            return Ok(Point::ZERO);
        }

        let tab_size = self.layout_rect.get().size();
        let rename_size = self.rename.layout_rect.get().size();

        let editor_data =
            if let Some(editor) = self.main_split.active_editor.get_untracked() {
                editor
            } else {
                return Ok(Point::ZERO);
            };

        let (window_origin, viewport) =
            (editor_data.window_origin(), editor_data.viewport);

        // TODO(minor): What affinity should we use for this?
        let (_point_above, point_below) =
            editor_data.points_of_offset(self.rename.start.get_untracked())?;

        let window_origin =
            window_origin.get() - self.common.window_origin.get().to_vec2();
        let viewport = viewport.get();

        let mut origin = window_origin
            + Vec2::new(point_below.x - viewport.x0, point_below.y - viewport.y0);

        if origin.y + rename_size.height > tab_size.height {
            origin.y = origin.y - line_height - rename_size.height;
        }
        if origin.x + rename_size.width + 1.0 > tab_size.width {
            origin.x = tab_size.width - rename_size.width - 1.0;
        }
        if origin.x <= 0.0 {
            origin.x = 0.0;
        }

        Ok(origin)
    }

    /// Get the mode for the current editor or terminal
    pub fn mode(&self) -> Mode {
        if self.common.config.signal(|x| x.core.modal.signal()).get() {
            let mode = if self.common.focus.get() == Focus::Workbench {
                self.main_split
                    .active_editor
                    .get()
                    .map(|editor| editor.cursor().with(|c| c.mode().simply_mode()))
            } else {
                None
            };

            mode.unwrap_or(Mode::Normal)
        } else {
            Mode::Insert
        }
    }

    pub fn toggle_panel_visual(&self, kind: PanelKind) {
        if self.panel.is_panel_visible(&kind) {
            self.hide_panel(kind);
        } else {
            self.show_panel(kind);
        }
    }

    /// Toggle a specific kind of panel.
    fn toggle_panel_focus(&self, kind: PanelKind) {
        let should_hide = match kind {
            PanelKind::FileExplorer
            | PanelKind::Plugin
            | PanelKind::Problem
            | PanelKind::Debug
            | PanelKind::CallHierarchy
            | PanelKind::DocumentSymbol
            | PanelKind::References
            | PanelKind::Implementation
            | PanelKind::Build => {
                // Some panels don't accept focus (yet). Fall back to visibility
                // check in those cases.
                self.panel.is_panel_visible(&kind)
            },
            PanelKind::Terminal | PanelKind::SourceControl | PanelKind::Search => {
                self.is_panel_focused(kind)
            },
        };
        if should_hide {
            self.hide_panel(kind);
        } else {
            self.show_panel(kind);
        }
    }

    /// Toggle a panel on one of the sides.
    fn toggle_container_visual(&self, position: &PanelContainerPosition) {
        let shown = !self.panel.is_container_shown(position, false);
        self.panel.set_shown(position, shown);

        if let Some((kind, _)) = self.panel.active_panel_at_position(position, false)
        {
            if shown {
                self.show_panel(kind);
            } else {
                self.hide_panel(kind);
            }
        }
    }

    fn is_panel_focused(&self, kind: PanelKind) -> bool {
        // Moving between e.g. Search and Problems doesn't affect focus, so we need
        // to also check visibility.
        self.common.focus.get_untracked() == Focus::Panel(kind)
            && self.panel.is_panel_visible(&kind)
    }

    fn hide_panel(&self, kind: PanelKind) {
        self.panel.hide_panel(&kind);
        self.common.focus.set(Focus::Workbench);
    }

    pub fn show_panel(&self, kind: PanelKind) {
        if kind == PanelKind::Terminal
            && self
                .terminal
                .tab_infos
                .with_untracked(|info| info.tabs.is_empty())
        {
            self.terminal.new_tab(
                self.common
                    .config
                    .with_untracked(|x| x.terminal.get_default_profile()),
            );
        }
        self.panel.show_panel(&kind);
        if kind == PanelKind::Search {
            if self.common.focus.get_untracked() == Focus::Workbench {
                let active_editor = self.main_split.active_editor.get_untracked();
                let word = active_editor.map(|editor| editor.word_at_cursor());
                if let Some(word) = word
                    && !word.is_empty()
                {
                    self.global_search.set_pattern(word);
                }
            }
            self.global_search
                .view_id
                .with_untracked(|x| x.request_focus());
        }
        self.common.focus.set(Focus::Panel(kind));
    }

    fn run_and_debug(&self, cx: Scope, mode: RunDebugMode, config: RunDebugConfig) {
        debug!("{:?}", config);
        match mode {
            RunDebugMode::Run => {
                self.run_program_in_terminal(cx, &mode, &config, false);
            },
            RunDebugMode::Debug => {
                let dap_id = config.dap_id;
                let dap_data = DapData::new(cx, dap_id, None, self.common.clone());
                self.terminal.debug.daps.update(|x| {
                    x.insert(dap_id, dap_data);
                });
                if config.prelaunch.is_some() {
                    self.run_program_in_terminal(cx, &mode, &config, false);
                } else {
                    self.terminal.dap_start(config);
                };
                if !self.panel.is_panel_visible(&PanelKind::Debug) {
                    self.panel.show_panel(&PanelKind::Debug);
                }
            },
        }
    }

    fn run_program_in_terminal(
        &self,
        _cx: Scope,
        mode: &RunDebugMode,
        config: &RunDebugConfig,
        from_dap: bool,
    ) -> TermId {
        // if not from dap, then run prelaunch first
        let is_prelaunch = !from_dap;
        let term_id = if let Some(terminal) =
            self.terminal.get_stopped_run_debug_terminal(mode, config)
        {
            terminal.new_process(Some(RunDebugProcess {
                mode: *mode,
                origin_config: config.clone(),
                config: config.clone(),
                stopped: false,
                created: Instant::now(),
                is_prelaunch,
            }));

            terminal.term_id
        } else {
            let new_terminal_tab = self.terminal.new_tab_run_debug(
                Some(RunDebugProcess {
                    origin_config: config.clone(),
                    mode: *mode,
                    config: config.clone(),
                    stopped: false,
                    created: Instant::now(),
                    is_prelaunch,
                }),
                None,
            );
            new_terminal_tab.term_id
        };
        self.common.focus.set(Focus::Panel(PanelKind::Terminal));
        self.terminal.focus_terminal(term_id);
        if !self.panel.is_panel_visible(&PanelKind::Terminal) {
            self.panel.show_panel(&PanelKind::Terminal);
        }
        if from_dap {
            let dap_id = config.dap_id;
            self.terminal.debug.daps.update(|x| {
                if let Some(data) = x.get_mut(&dap_id) {
                    data.term_id = Some(term_id);
                } else {
                    error!("no found dap data {dap_id:?}")
                }
            });
        }
        term_id
    }

    fn restart_run_program_in_terminal(
        &self,
        terminal_id: TerminalTabId,
    ) -> anyhow::Result<()> {
        let terminal = self
            .terminal
            .get_terminal(terminal_id)
            .ok_or(anyhow!("not found tab(terminal_id={terminal_id:?})"))?;
        // let terminal = tab.get_terminal();
        let mut run_debug = terminal
            .data
            .with_untracked(|x| x.run_debug.clone())
            .ok_or(anyhow!("run_debug is none(terminal_id={terminal_id:?})"))?;
        if run_debug.origin_config.config_source.from_palette() {
            match self
                .main_split
                .get_run_config_by_name(&run_debug.config.name)
            {
                Ok(Some(mut new_config)) => {
                    if let Some(workspace) = self.workspace.path() {
                        new_config.update_by_workspace(
                            workspace.to_string_lossy().as_ref(),
                        );
                    }
                    run_debug.origin_config = new_config.clone();
                    run_debug.config = new_config;
                },
                Ok(None) => {},
                Err(err) => {
                    error!("{err}");
                },
            }
        }
        log::debug!("restart_run_program_in_terminal {run_debug:?}");
        if !run_debug.stopped {
            self.terminal.manual_stop_run_debug(terminal_id);
        }
        match run_debug.mode {
            RunDebugMode::Run => {
                run_debug.config = run_debug.origin_config.clone();
                run_debug.stopped = false;
                run_debug.created = Instant::now();
                run_debug.is_prelaunch = true;
                terminal.new_process(Some(run_debug));
            },
            RunDebugMode::Debug => {
                let mut config = run_debug.origin_config.clone();
                let dap_id = config.dap_id;
                let dap_data = DapData::new(
                    self.scope,
                    dap_id,
                    Some(terminal.term_id),
                    self.common.clone(),
                );
                self.terminal.debug.daps.update(|x| {
                    x.insert(dap_id, dap_data);
                });
                if config.prelaunch.is_some() {
                    if !run_debug.is_prelaunch {
                        config
                            .config_source
                            .update_program(&run_debug.config.program);
                    }
                    run_debug.config = config.clone();
                    run_debug.stopped = false;
                    run_debug.created = Instant::now();
                    run_debug.is_prelaunch = true;
                    terminal.new_process(Some(run_debug));
                } else {
                    self.terminal.dap_start(config);
                };
                if !self.panel.is_panel_visible(&PanelKind::Debug) {
                    self.panel.show_panel(&PanelKind::Debug);
                }
            },
        }
        Ok(())
    }

    pub fn open_paths(&self, paths: &[PathObject]) {
        let (folders, files): (Vec<&PathObject>, Vec<&PathObject>) =
            paths.iter().partition(|p| p.is_dir);

        for folder in folders {
            self.common.window_common.window_command.send(
                WindowCommand::NewWorkspaceTab {
                    workspace: Arc::new(LapceWorkspace::new(
                        self.workspace.kind().clone(),
                        Some(folder.path.clone()),
                        0,
                    )),
                    end:       false,
                },
            );
        }

        for file in files {
            let position = file.linecol.map(|pos| {
                EditorPosition::Position(lsp_types::Position {
                    line:      pos.line.saturating_sub(1) as u32,
                    character: pos.column.saturating_sub(1) as u32,
                })
            });

            self.common
                .internal_command
                .send(InternalCommand::GoToLocation {
                    location: EditorLocation {
                        path: file.path.clone(),
                        position,
                        scroll_offset: None,
                        // Create a new editor for the file, so we don't change any
                        // current unconfirmed editor
                        ignore_unconfirmed: true,
                        same_editor_tab: false,
                    },
                });
        }
    }

    pub fn show_alert(&self, title: String, msg: String, buttons: Vec<AlertButton>) {
        self.alert_data.title.set(title);
        self.alert_data.msg.set(msg);
        self.alert_data.buttons.set(buttons);
        self.alert_data.active.set(true);
    }

    fn update_progress(&self, progress: &ProgressParams) {
        let token = progress.token.clone();
        match &progress.value {
            lsp_types::ProgressParamsValue::WorkDone(progress) => match progress {
                lsp_types::WorkDoneProgress::Begin(progress) => {
                    let progress = WorkProgress {
                        token:      token.clone(),
                        title:      progress.title.clone(),
                        message:    progress.message.clone(),
                        percentage: progress.percentage,
                    };
                    self.progresses.update(|p| {
                        p.insert(token, progress);
                    });
                },
                lsp_types::WorkDoneProgress::Report(report) => {
                    self.progresses.update(|p| {
                        if let Some(progress) = p.get_mut(&token) {
                            progress.message.clone_from(&report.message);
                            progress.percentage = report.percentage;
                        }
                    })
                },
                lsp_types::WorkDoneProgress::End(_) => {
                    self.progresses.update(|p| {
                        p.swap_remove(&token);
                    });
                },
            },
        }
    }

    fn show_message(&self, title: &str, message: &ShowMessageParams) {
        self.messages.update(|messages| {
            messages.push((title.to_string(), message.clone()));
        });
    }

    #[allow(dead_code)]
    fn show_error_message(&self, title: String, message: String) {
        self.messages.update(|messages| {
            messages.push((
                title,
                ShowMessageParams {
                    typ: MessageType::ERROR,
                    message,
                },
            ));
        });
    }

    pub fn show_code_lens(
        &self,
        mouse_click: bool,
        plugin_id: PluginId,
        offset: usize,
        lens: im::Vector<(Id, CodeLens)>,
    ) {
        self.common
            .internal_command
            .send(InternalCommand::ShowCodeActions {
                offset,
                mouse_click,
                plugin_id,
                code_actions: lens
                    .into_iter()
                    .filter_map(|lens| {
                        Some(CodeActionOrCommand::Command(lens.1.command?))
                    })
                    .collect(),
            });
    }

    pub fn call_hierarchy_incoming(&self, root_id: ViewId, item_id: ViewId) {
        let Some(item) = self.main_split.hierarchy.tabs.with_untracked(|x| {
            x.iter().find_map(move |item| {
                let refe = item.references.get_untracked();
                if refe.root_id == root_id {
                    Some(refe)
                } else {
                    None
                }
            })
        }) else {
            return;
        };
        let Some(item) = CallHierarchyItemData::find_by_id(item.root, item_id)
        else {
            return;
        };
        let root_item = item;
        let path: PathBuf = item.get_untracked().item.uri.to_file_path().unwrap();
        let scope = self.scope;
        let send = create_ext_action(
            scope,
            move |(_id, _rs): (u64, Result<ProxyResponse, RpcError>)| match _rs {
                Ok(ProxyResponse::CallHierarchyIncomingResponse { items }) => {
                    if let Some(items) = items {
                        let mut item_children = Vec::new();
                        for x in items {
                            let item = Rc::new(x.from);
                            for range in x.from_ranges {
                                item_children.push(scope.create_rw_signal(
                                    CallHierarchyItemData {
                                        root_id,
                                        view_id: floem::ViewId::new(),
                                        item: item.clone(),
                                        from_range: range,
                                        init: false,
                                        open: scope.create_rw_signal(false),
                                        children: scope.create_rw_signal(Vec::new()),
                                    },
                                ))
                            }
                        }
                        root_item.update(|x| {
                            x.init = true;
                            x.children.update(|children| {
                                *children = item_children;
                            })
                        });
                    }
                },
                Err(err) => {
                    log::error!("{:?}", err);
                },
                Ok(_) => {},
            },
        );
        self.common.proxy.proxy_rpc.call_hierarchy_incoming(
            path,
            item.get_untracked().item.as_ref().clone(),
            send,
        );
    }

    pub fn content_info(&self, data: &SplitContent) -> SplitContentInfo {
        match data {
            SplitContent::EditorTab(editor_tab_id) => {
                let editor_tab_data = self
                    .main_split
                    .editor_tabs
                    .get_untracked()
                    .get(editor_tab_id)
                    .cloned()
                    .unwrap();
                SplitContentInfo::EditorTab(
                    editor_tab_data.get_untracked().tab_info(self),
                )
            },
            SplitContent::Split(split_id) => {
                let split_data = self
                    .main_split
                    .splits
                    .get_untracked()
                    .get(split_id)
                    .cloned()
                    .unwrap();
                SplitContentInfo::Split(split_data.get_untracked().split_info(self))
            },
        }
    }
}

/// Open path with the default application without blocking.
fn open_uri(path: &Path) {
    match open::that(path) {
        Ok(_) => {
            debug!("opened active file: {path:?}");
        },
        Err(e) => {
            error!("failed to open active file: {path:?}, error: {e}");
        },
    }
}

pub fn contains_ansi(s: &str) -> bool {
    // ANSI 转义序列一般以 \x1B (ESC) 开头，紧接着是 '[' 等控制符
    let ansi_regex = regex::Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").unwrap();
    ansi_regex.is_match(s)
}

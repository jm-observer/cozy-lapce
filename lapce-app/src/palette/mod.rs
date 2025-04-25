use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use anyhow::Result;
use doc::{
    language::LapceLanguage,
    lines::{
        EditBuffer, buffer::rope_text::RopeText, command::FocusCommand,
        editor_command::CommandExecuted, line_ending::LineEnding, mode::Mode,
        movement::Movement,
    },
    syntax::Syntax,
};
use floem::{
    action::{TimerToken, exec_after},
    ext_event::create_ext_action,
    keyboard::Modifiers,
    reactive::{
        Memo, ReadSignal, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith,
        batch, use_context,
    },
};
use im::Vector;
use itertools::Itertools;
#[cfg(windows)]
use lapce_core::workspace::WslHost;
use lapce_core::{
    debug::{RunDebugConfigs, RunDebugMode},
    doc::DocContent,
    workspace::{LapceWorkspace, LapceWorkspaceType, SshHost},
};
use lapce_rpc::proxy::ProxyResponse;
use lapce_xi_rope::Rope;
use log::{error, info};
use lsp_types::{DocumentSymbol, DocumentSymbolResponse};
use nucleo::Utf32Str;
use strum::{EnumMessage, IntoEnumIterator};

use self::{
    item::{PaletteItem, PaletteItemContent},
    kind::PaletteKind,
};
use crate::{
    command::{
        CommandKind, InternalCommand, LapceCommand, LapceWorkbenchCommand,
        WindowCommand,
    },
    db::LapceDb,
    editor::{
        EditorData,
        location::{EditorLocation, EditorPosition},
    },
    keypress::{KeyPressData, KeyPressFocus, condition::Condition},
    lsp::path_from_url,
    main_split::MainSplitData,
    source_control::SourceControlData,
    window_workspace::{CommonData, Focus},
};

pub mod item;
pub mod kind;

pub const DEFAULT_RUN_TOML: &str = include_str!("../../../defaults/run.toml");

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PaletteStatus {
    Inactive,
    Started,
    Done,
}

#[derive(Clone, Debug)]
pub struct PaletteInput {
    pub input: String,
    pub kind:  PaletteKind,
}

impl PaletteInput {
    /// Update the current input in the palette, and the kind of palette it is
    pub fn update_input(&mut self, input: String, kind: Option<PaletteKind>) {
        if let Some(kind) = kind {
            self.kind = kind.get_palette_kind(&input);
            self.input = self.kind.get_input(&input).to_string();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RunResult {
    pub id:      u64,
    pub rs:      Vector<PaletteItem>,
    pub updated: bool,
}

impl RunResult {
    pub fn update_id(&mut self) -> u64 {
        self.id += 1;
        self.rs.clear();
        self.updated = false;
        self.id
    }

    pub fn update_rs(&mut self, id: u64, rs: Vector<PaletteItem>) -> bool {
        if id == self.id {
            self.rs = rs;
            self.updated = true;
            true
        } else {
            false
        }
    }

    pub fn rs(&self) -> Option<&Vector<PaletteItem>> {
        if self.updated { Some(&self.rs) } else { None }
    }

    pub fn is_empty(&self) -> bool {
        self.updated && self.rs.is_empty()
    }
}

pub type DocumentSymbolInfo =
    RwSignal<Option<(PathBuf, Option<(im::Vector<PaletteItem>, SystemTime)>)>>;
#[derive(Clone)]
pub struct PaletteData {
    pub workspace:             Arc<LapceWorkspace>,
    pub status:                RwSignal<PaletteStatus>,
    pub index:                 RwSignal<usize>,
    pub preselect_index:       RwSignal<Option<usize>>,
    pub items:                 RwSignal<Vector<PaletteItem>>,
    pub filtered_items:        RwSignal<Vector<PaletteItem>>,
    pub input:                 RwSignal<PaletteInput>,
    pub kind:                  RwSignal<Option<PaletteKind>>,
    // pub input_editor:          EditorData,
    pub input_str:             RwSignal<String>,
    pub preview_editor:        EditorData,
    pub has_preview:           RwSignal<bool>,
    pub has_preview_memo:      Memo<bool>,
    pub keypress:              ReadSignal<KeyPressData>,
    /// Listened on for which entry in the palette has been clicked
    pub clicked_index:         RwSignal<Option<usize>>,
    pub executed_commands:     Rc<RefCell<HashMap<String, Instant>>>,
    pub executed_run_configs:  Rc<RefCell<HashMap<(RunDebugMode, String), Instant>>>,
    pub main_split:            MainSplitData,
    pub references:            RwSignal<Vec<EditorLocation>>,
    pub source_control:        SourceControlData,
    pub common:                Rc<CommonData>,
    left_diff_path:            RwSignal<Option<PathBuf>>,
    pub workspace_document_id: RwSignal<Option<u64>>,
    pub document_symbol:       DocumentSymbolInfo,
    pub run_result:            RwSignal<RunResult>,
}

impl std::fmt::Debug for PaletteData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaletteData").finish()
    }
}

impl PaletteData {
    pub fn new(
        cx: Scope,
        workspace: Arc<LapceWorkspace>,
        main_split: MainSplitData,
        keypress: ReadSignal<KeyPressData>,
        source_control: SourceControlData,
        common: Rc<CommonData>,
    ) -> Self {
        let status = cx.create_rw_signal(PaletteStatus::Inactive);
        let items = cx.create_rw_signal(im::Vector::new());
        let preselect_index = cx.create_rw_signal(None);
        let index = cx.create_rw_signal(0);
        let references = cx.create_rw_signal(Vec::new());
        let input = cx.create_rw_signal(PaletteInput {
            input: "".to_string(),
            kind:  PaletteKind::HelpAndFile,
        });
        let kind = cx.create_rw_signal(None);

        let preview_editor = main_split.editors.make_local(cx, common.clone());
        let has_preview = cx.create_rw_signal(false);
        let has_preview_memo = cx.create_memo(move |_| has_preview.get());

        let set_filtered_items = cx.create_rw_signal(Vector::new());

        let clicked_index = cx.create_rw_signal(Option::<usize>::None);
        let left_diff_path = cx.create_rw_signal(None);

        let palette = Self {
            main_split,
            workspace,
            status,
            index,
            preselect_index,
            items,
            filtered_items: set_filtered_items,
            input_str: cx.create_rw_signal(String::new()),
            preview_editor,
            has_preview,
            has_preview_memo,
            input,
            kind,
            keypress,
            clicked_index,
            executed_commands: Rc::new(RefCell::new(HashMap::new())),
            executed_run_configs: Rc::new(RefCell::new(HashMap::new())),
            references,
            source_control,
            common,
            left_diff_path,
            workspace_document_id: cx.create_rw_signal(None),
            document_symbol: cx.create_rw_signal(None),
            run_result: cx.create_rw_signal(RunResult::default()),
        };

        {
            let palette = palette.clone();
            let clicked_index = clicked_index.read_only();
            let index = index.write_only();
            cx.create_effect(move |_| {
                if let Some(clicked_index) = clicked_index.get() {
                    index.set(clicked_index);
                    palette.select();
                }
            });
        }

        {
            let palette = palette.clone();
            let input_str = palette.input_str;
            let status = palette.status.read_only();
            let blink_timer = cx.create_rw_signal(TimerToken::INVALID);
            // Monitors when the palette's input changes, so that it can update the
            // stored input and kind of palette.
            cx.create_effect(move |_| {
                // TODO(minor, perf): this could have perf issues if the user
                // accidentally pasted a huge amount of text into the palette.
                let new_input = input_str.get();
                let status = status.get_untracked();
                // info!("input {new_input} status: {:?}", status);
                if status == PaletteStatus::Inactive {
                    return None;
                }

                let palette = palette.clone();
                let timer_token =
                    exec_after(Duration::from_millis(500), move |timer_token| {
                        if timer_token == blink_timer.get_untracked() {
                            if let Err(err) = palette.run_inner_by_input(new_input) {
                                error!("{err}");
                            }
                        }
                    });
                // warn!("set id={:?} {:?}",
                // floem::prelude::SignalGet::id(&blink_timer),
                // timer_token);
                blink_timer.set(timer_token);
                Some(())
            });
        }

        // {
        //     let palette = palette.clone();
        //     let preset_kind = palette.kind.read_only();
        //     cx.create_effect(move |last_kind| {
        //         let new_kind = preset_kind.get();
        //         if let Some(new_kind) = new_kind {
        //             if !last_kind
        //                 .flatten()
        //                 .map(|x| x == new_kind)
        //                 .unwrap_or_default()
        //             {
        //                 palette.run_inner(new_kind);
        //             }
        //         }
        //         new_kind
        //     });
        // }

        {
            let palette = palette.clone();
            cx.create_effect(move |_| {
                let _ = palette.index.get();
                palette.preview();
            });
        }

        {
            let palette = palette.clone();
            cx.create_effect(move |_| {
                let focus = palette.common.focus.get();
                if focus != Focus::Palette
                    && palette.status.get_untracked() != PaletteStatus::Inactive
                {
                    palette.cancel();
                }
            });
        }

        palette
    }

    /// Start and focus the palette for the given kind.
    pub fn run(&self, kind: PaletteKind) {
        self.run_result.update(|x| {
            x.update_id();
        });
        self.common.focus.set(Focus::Palette);
        self.status.set(PaletteStatus::Started);
        let symbol = kind.symbol();
        self.kind.set(Some(kind));
        self.input_str.set(symbol.to_string());
        // // Refresh the palette input with only the symbol prefix, losing old
        // content. self.input_editor.doc().reload(Rope::from(symbol),
        // true); self.input_editor
        //     .cursor()
        //     .update(|cursor|
        // cursor.set_insert(Selection::caret(symbol.len())));
    }

    /// Get the placeholder text to use in the palette input field.
    pub fn placeholder_text(&self) -> &'static str {
        match self.kind.get() {
            Some(PaletteKind::SshHost) => {
                "Type [user@]host or select a previously connected workspace below"
            },
            Some(PaletteKind::DiffFiles) => {
                if self.left_diff_path.with(Option::is_some) {
                    "Select right file"
                } else {
                    "Seleft left file"
                }
            },
            _ => "",
        }
    }

    fn run_inner_by_input(&self, input: String) -> Result<()> {
        let kind = match self.kind.get_untracked() {
            None => PaletteKind::from_input(&input),
            Some(kind) => {
                if matches!(kind, PaletteKind::HelpAndFile) && !input.is_empty() {
                    PaletteKind::from_input(&input)
                } else {
                    kind
                }
            },
        };
        let kind_input = kind.get_input(&input);
        // 太多kind不需要input了
        // if kind_input.is_empty() && !matches!(kind, PaletteKind::HelpAndFile) {
        //     return;
        // }
        let run_id = self.run_result.try_update(|x| x.update_id()).unwrap();
        log::debug!("run_inner_by_input {} {:?} input={input}", run_id, kind);
        match kind {
            PaletteKind::PaletteHelp => self.get_palette_help(run_id),
            PaletteKind::DiffFiles => {
                self.get_files(run_id);
            },
            PaletteKind::HelpAndFile => self.get_palette_help_and_file(run_id),
            PaletteKind::Line => {
                self.get_lines(run_id, kind_input);
            },
            PaletteKind::Command => {
                self.get_commands(run_id, kind_input);
            },
            PaletteKind::Workspace => {
                self.get_workspaces(run_id, kind_input);
            },
            PaletteKind::Reference => {
                self.get_references(run_id, kind_input);
            },
            PaletteKind::DocumentSymbol => {
                self.get_document_symbols(run_id, kind_input);
            },
            PaletteKind::WorkspaceSymbol => {
                self.get_workspace_symbols(run_id, kind_input);
            },
            PaletteKind::SshHost => {
                self.get_ssh_hosts(run_id);
            },
            #[cfg(windows)]
            PaletteKind::WslHost => {
                self.get_wsl_hosts(run_id);
            },
            PaletteKind::RunAndDebug => {
                self.get_run_configs(run_id, kind_input.to_string())?;
            },
            PaletteKind::ColorTheme => {
                self.get_color_themes(run_id);
            },
            PaletteKind::IconTheme => {
                self.get_icon_themes(run_id);
            },
            PaletteKind::Language => {
                self.get_languages(run_id);
            },
            PaletteKind::LineEnding => {
                self.get_line_endings(run_id);
            },
            PaletteKind::SCMReferences => {
                self.get_scm_references(run_id);
            },
            PaletteKind::TerminalProfile => self.get_terminal_profiles(run_id),
        }
        Ok(())
    }

    fn update_rs(&self, id: u64, rs: Vector<PaletteItem>) {
        batch(|| {
            let is_empty = rs.is_empty();
            if self
                .run_result
                .try_update(|run_result| run_result.update_rs(id, rs))
                .unwrap_or_default()
                && !is_empty
            {
                self.index.set(0);
            }
        });
    }

    // /// Execute the internal behavior of the palette for the given kind. This
    // /// ignores updating and focusing the palette input.
    // fn run_inner(&self, kind: PaletteKind) {
    //     self.has_preview.set(false);
    //
    //     let run_id = self.run_id_counter.fetch_add(1, Ordering::Relaxed) + 1;
    //     log::debug!("run_inner {} {:?}", run_id, kind);
    //     match kind {
    //         PaletteKind::PaletteHelp => (),
    //         PaletteKind::DiffFiles => {},
    //         PaletteKind::HelpAndFile => (),
    //         PaletteKind::Line => {},
    //         PaletteKind::Command => {},
    //         PaletteKind::Workspace => {},
    //         PaletteKind::Reference => {},
    //         PaletteKind::DocumentSymbol => {
    //             // self.get_document_symbols();
    //         },
    //         PaletteKind::WorkspaceSymbol => {
    //             // self.get_workspace_symbols(, );
    //         },
    //         PaletteKind::SshHost => {
    //             self.get_ssh_hosts(run_id);
    //         },
    //         #[cfg(windows)]
    //         PaletteKind::WslHost => {
    //             self.get_wsl_hosts();
    //         },
    //         PaletteKind::RunAndDebug => {},
    //         PaletteKind::ColorTheme => {
    //             self.get_color_themes();
    //         },
    //         PaletteKind::IconTheme => {
    //             self.get_icon_themes();
    //         },
    //         PaletteKind::Language => {
    //             self.get_languages();
    //         },
    //         PaletteKind::LineEnding => {
    //             self.get_line_endings();
    //         },
    //         PaletteKind::SCMReferences => {
    //             self.get_scm_references();
    //         },
    //         PaletteKind::TerminalProfile => self.get_terminal_profiles()
    //     }
    // }

    /// Initialize the palette with a list of the available palette kinds.
    fn get_palette_help(&self, run_id: u64) {
        let items = self.get_palette_help_items(run_id);
        self.update_rs(run_id, items);
    }

    fn get_palette_help_items(&self, run_id: u64) -> Vector<PaletteItem> {
        PaletteKind::iter()
            .filter_map(|kind| {
                // Don't include PaletteHelp as the user is already here.
                (kind != PaletteKind::PaletteHelp)
                    .then(|| {
                        let symbol = kind.symbol();

                        // Only include palette kinds accessible by typing a prefix
                        // into the palette.
                        (!symbol.is_empty()).then_some(kind)
                    })
                    .flatten()
            })
            .filter_map(|kind| kind.command().map(|cmd| (kind, cmd)))
            .map(|(kind, cmd)| {
                let description = kind.symbol().to_string()
                    + " "
                    + cmd.get_message().unwrap_or("");

                PaletteItem {
                    content: PaletteItemContent::PaletteHelp { cmd },
                    filter_text: description,
                    score: 0,
                    indices: vec![],
                    run_id,
                }
            })
            .collect()
    }

    fn get_palette_help_and_file(&self, run_id: u64) {
        let help_items: Vector<PaletteItem> = self.get_palette_help_items(run_id);
        self.get_files_and_prepend(Some(help_items), run_id);
    }

    // get the files in the current workspace
    // and prepend items if prepend is some
    // e.g. help_and_file
    fn get_files_and_prepend(
        &self,
        prepend: Option<im::Vector<PaletteItem>>,
        run_id: u64,
    ) {
        let workspace = self.workspace.clone();
        let data = self.clone();
        let send =
            create_ext_action(self.common.scope, move |items: Vec<PathBuf>| {
                let items = items
                    .into_iter()
                    .map(|full_path| {
                        // Strip the workspace prefix off the path, to avoid clutter
                        let path =
                            if let Some(workspace_path) = workspace.path.as_ref() {
                                full_path
                                    .strip_prefix(workspace_path)
                                    .unwrap_or(&full_path)
                                    .to_path_buf()
                            } else {
                                full_path.clone()
                            };
                        let filter_text = path.to_string_lossy().into_owned();
                        PaletteItem {
                            content: PaletteItemContent::File { path, full_path },
                            filter_text,
                            score: 0,
                            indices: Vec::new(),
                            run_id,
                        }
                    })
                    .collect::<im::Vector<_>>();
                let input_str = data.input_str.get_untracked();
                if let Some(mut prepend) = prepend {
                    prepend.append(items);
                    data.filter_items(
                        run_id,
                        PaletteKind::HelpAndFile.get_input(&input_str),
                        prepend,
                    );
                } else {
                    data.filter_items(
                        run_id,
                        PaletteKind::HelpAndFile.get_input(&input_str),
                        items,
                    );
                }
            });
        self.common.proxy.proxy_rpc.get_files(move |(_, result)| {
            if let Ok(ProxyResponse::GetFilesResponse { items }) = result {
                send(items);
            }
        });
    }

    /// Initialize the palette with the files in the current workspace.
    fn get_files(&self, run_id: u64) {
        self.get_files_and_prepend(None, run_id);
    }

    /// Initialize the palette with the lines in the current document.
    fn get_lines(&self, run_id: u64, input_str: &str) {
        let editor = self.main_split.active_editor.get_untracked();
        let doc = match editor {
            Some(editor) => editor.doc(),
            None => {
                info!("get_lines none");
                return;
            },
        };

        let buffer = doc
            .lines
            .with_untracked(|x| x.signal_buffer())
            .get_untracked();
        let last_line_number = buffer.last_line() + 1;
        let last_line_number_len = last_line_number.to_string().len();
        let items = buffer
            .text()
            .lines(0..buffer.len())
            .enumerate()
            .map(|(i, l)| {
                let line_number = i + 1;
                let text = format!(
                    "{}{} {}",
                    line_number,
                    vec![" "; last_line_number_len - line_number.to_string().len()]
                        .join(""),
                    l
                );
                PaletteItem {
                    content: PaletteItemContent::Line {
                        line:    i,
                        content: text.clone(),
                    },
                    filter_text: text,
                    score: 0,
                    indices: vec![],
                    run_id,
                }
            })
            .collect();

        self.filter_items(run_id, input_str, items);
    }

    fn get_commands(&self, run_id: u64, input_str: &str) {
        const EXCLUDED_ITEMS: &[&str] = &["palette.command"];

        let items = self.keypress.with_untracked(|keypress| {
            // Get all the commands we've executed, and sort them by how recently
            // they were executed. Ignore commands without descriptions.
            let mut items: im::Vector<PaletteItem> = self
                .executed_commands
                .borrow()
                .iter()
                .sorted_by_key(|(_, i)| *i)
                .rev()
                .filter_map(|(key, _)| {
                    keypress.commands.get(key).and_then(|c| {
                        c.kind.desc().as_ref().map(|m| PaletteItem {
                            content: PaletteItemContent::Command { cmd: c.clone() },
                            filter_text: m.to_string(),
                            score: 0,
                            indices: vec![],
                            run_id,
                        })
                    })
                })
                .collect();
            // Add all the rest of the commands, ignoring palette commands (because
            // we're in it) and commands that are sorted earlier due to
            // being executed.
            items.extend(keypress.commands.iter().filter_map(|(_, c)| {
                if EXCLUDED_ITEMS.contains(&c.kind.str()) {
                    return None;
                }

                if self.executed_commands.borrow().contains_key(c.kind.str()) {
                    return None;
                }

                c.kind.desc().as_ref().map(|m| PaletteItem {
                    content: PaletteItemContent::Command { cmd: c.clone() },
                    filter_text: m.to_string(),
                    score: 0,
                    indices: vec![],
                    run_id,
                })
            }));

            items
        });

        self.filter_items(run_id, input_str, items);
    }

    /// Initialize the palette with all the available workspaces, local and
    /// remote.
    fn get_workspaces(&self, run_id: u64, input_str: &str) {
        let db: Arc<LapceDb> = use_context().unwrap();
        let workspaces = db.recent_workspaces().unwrap_or_default();

        let items = workspaces
            .into_iter()
            .filter_map(|w| {
                let text = w.path.as_ref()?.to_str()?.to_string();
                let filter_text = match &w.kind {
                    LapceWorkspaceType::Local => text,
                    LapceWorkspaceType::RemoteSSH(remote) => {
                        format!("[{remote}] {text}")
                    },
                    #[cfg(windows)]
                    LapceWorkspaceType::RemoteWSL(remote) => {
                        format!("[{remote}] {text}")
                    },
                };
                Some(PaletteItem {
                    content: PaletteItemContent::Workspace {
                        workspace: Arc::new(w),
                    },
                    filter_text,
                    score: 0,
                    indices: vec![],
                    run_id,
                })
            })
            .collect();
        self.filter_items(run_id, input_str, items);
    }

    /// Initialize the list of references in the file, from the current editor
    /// location.
    fn get_references(&self, run_id: u64, input_str: &str) {
        let items = self
            .references
            .get_untracked()
            .into_iter()
            .map(|l| {
                let full_path = l.path.clone();
                let mut path = l.path.clone();
                if let Some(workspace_path) = self.workspace.path.as_ref() {
                    path = path
                        .strip_prefix(workspace_path)
                        .unwrap_or(&full_path)
                        .to_path_buf();
                }
                let filter_text = path.to_str().unwrap_or("").to_string();
                PaletteItem {
                    content: PaletteItemContent::Reference { path, location: l },
                    filter_text,
                    score: 0,
                    indices: vec![],
                    run_id,
                }
            })
            .collect();
        self.filter_items(run_id, input_str, items);
    }

    fn get_document_symbols(&self, run_id: u64, input_str: &str) {
        // if input_str.is_empty() {
        //     info!("get_document_symbols is_empty");
        //     return;
        // }
        let editor = self.main_split.active_editor.get_untracked();
        let doc = match editor {
            Some(editor) => editor.doc(),
            None => {
                self.items.update(|items| items.clear());
                return;
            },
        };
        let path = doc
            .content
            .with_untracked(|content| content.path().cloned());
        let path = match path {
            Some(path) => path,
            None => {
                self.filtered_items.update(|items| items.clear());
                return;
            },
        };

        if let Some((old, items)) = self.document_symbol.get_untracked() {
            if old == path {
                if let Some((items, time)) = &items {
                    if time.elapsed().map(|x| x.as_secs() < 60).unwrap_or_default() {
                        info!("old data");
                        self.filter_items(run_id, input_str, items.clone());
                    } else {
                        self.document_symbol.set(Some((old, None)));
                    }
                } else {
                    return;
                }
            } else {
                self.document_symbol.set(Some((path.clone(), None)));
            }
        }

        let doc_path = path.clone();
        let input = self.input_str;
        let document_symbol = self.document_symbol;
        let data = self.clone();
        let send = create_ext_action(self.common.scope, move |result| {
            if let Ok(ProxyResponse::GetDocumentSymbols { resp }) = result {
                let items = Self::format_document_symbol_resp(resp, run_id);
                document_symbol
                    .set(Some((doc_path, Some((items.clone(), SystemTime::now())))));
                let input_str = input.get_untracked();
                data.filter_items(
                    run_id,
                    PaletteKind::DocumentSymbol.get_input(&input_str),
                    items,
                );
            }
        });

        self.common.proxy.proxy_rpc.get_document_symbols(
            path,
            move |(_, result)| {
                send(result);
            },
        );
    }

    fn format_document_symbol_resp(
        resp: DocumentSymbolResponse,
        run_id: u64,
    ) -> im::Vector<PaletteItem> {
        match resp {
            DocumentSymbolResponse::Flat(symbols) => symbols
                .iter()
                .map(|s| {
                    let mut filter_text = s.name.clone();
                    if let Some(container_name) = s.container_name.as_ref() {
                        filter_text += container_name;
                    }
                    PaletteItem {
                        content: PaletteItemContent::DocumentSymbol {
                            kind:           s.kind,
                            name:           s.name.replace('\n', "↵"),
                            range:          s.location.range,
                            container_name: s.container_name.clone(),
                        },
                        filter_text,
                        score: 0,
                        indices: Vec::new(),
                        run_id,
                    }
                })
                .collect(),
            DocumentSymbolResponse::Nested(symbols) => {
                let mut items = im::Vector::new();
                for s in symbols {
                    Self::format_document_symbol(&mut items, None, s, run_id)
                }
                items
            },
        }
    }

    fn format_document_symbol(
        items: &mut im::Vector<PaletteItem>,
        parent: Option<String>,
        s: DocumentSymbol,
        run_id: u64,
    ) {
        items.push_back(PaletteItem {
            content: PaletteItemContent::DocumentSymbol {
                kind:           s.kind,
                name:           s.name.replace('\n', "↵"),
                range:          s.range,
                container_name: parent,
            },
            filter_text: s.name.clone(),
            score: 0,
            indices: Vec::new(),
            run_id,
        });
        if let Some(children) = s.children {
            let parent = Some(s.name.replace('\n', "↵"));
            for child in children {
                Self::format_document_symbol(items, parent.clone(), child, run_id);
            }
        }
    }

    fn get_workspace_symbols(&self, run_id: u64, input: &str) {
        let data = self.clone();
        let send = create_ext_action(self.common.scope, move |(old_id, result)| {
            if data.reset_workspace_id(old_id) {
                if let Ok(ProxyResponse::GetWorkspaceSymbols { symbols }) = result {
                    let items: im::Vector<PaletteItem> = symbols
                        .iter()
                        .map(|s| {
                            // TODO: Should we be using filter text?
                            let mut filter_text = s.name.clone();
                            if let Some(container_name) = s.container_name.as_ref() {
                                filter_text += container_name;
                            }
                            PaletteItem {
                                content: PaletteItemContent::WorkspaceSymbol {
                                    kind:           s.kind,
                                    name:           s.name.clone(),
                                    location:       EditorLocation {
                                        path:               path_from_url(
                                            &s.location.uri,
                                        ),
                                        position:           Some(
                                            EditorPosition::Position(
                                                s.location.range.start,
                                            ),
                                        ),
                                        scroll_offset:      None,
                                        ignore_unconfirmed: false,
                                        same_editor_tab:    false,
                                    },
                                    container_name: s.container_name.clone(),
                                },
                                filter_text,
                                score: 0,
                                indices: Vec::new(),
                                run_id,
                            }
                        })
                        .collect();
                    data.update_rs(run_id, items);
                } else {
                    data.update_rs(run_id, Vector::new());
                }
            }
        });

        let id = self.common.proxy.proxy_rpc.get_workspace_symbols(
            input.to_string(),
            move |(id, result)| {
                send((id, result));
            },
        );
        self.update_workspace_id(id);
    }

    fn update_workspace_id(&self, id: u64) {
        if let Some(_old_id) = self.workspace_document_id.get_untracked() {
            // todo
            self.common.proxy.proxy_rpc.lsp_cancel(_old_id);
        }
        self.workspace_document_id.set(Some(id));
    }

    fn reset_workspace_id(&self, id: u64) -> bool {
        if let Some(_old_id) = self.workspace_document_id.get_untracked() {
            if _old_id == id {
                self.workspace_document_id.set(None);
                return true;
            }
        }
        false
    }

    fn get_ssh_hosts(&self, run_id: u64) {
        let db: Arc<LapceDb> = use_context().unwrap();
        let workspaces = db.recent_workspaces().unwrap_or_default();
        let mut hosts = HashSet::new();
        for workspace in workspaces.iter() {
            if let LapceWorkspaceType::RemoteSSH(host) = &workspace.kind {
                hosts.insert(host.clone());
            }
        }

        let items = hosts
            .iter()
            .map(|host| PaletteItem {
                content: PaletteItemContent::SshHost { host: host.clone() },
                filter_text: host.to_string(),
                score: 0,
                indices: vec![],
                run_id,
            })
            .collect();
        self.items.set(items);
    }

    #[cfg(windows)]
    fn get_wsl_hosts(&self, run_id: u64) {
        use std::{os::windows::process::CommandExt, process};
        let cmd = process::Command::new("wsl")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .arg("-l")
            .arg("-v")
            .stdout(process::Stdio::piped())
            .output();

        let distros = if let Ok(proc) = cmd {
            let distros = String::from_utf16(bytemuck::cast_slice(&proc.stdout))
                .unwrap_or_default()
                .lines()
                .skip(1)
                .filter_map(|line| {
                    let line = line.trim_start();
                    // let default = line.starts_with('*');
                    let name = line
                        .trim_start_matches('*')
                        .trim_start()
                        .split(' ')
                        .next()?;
                    Some(name.to_string())
                })
                .collect();

            distros
        } else {
            vec![]
        };

        let db: Arc<LapceDb> = use_context().unwrap();
        let workspaces = db.recent_workspaces().unwrap_or_default();
        let mut hosts = HashSet::new();
        for distro in distros {
            hosts.insert(distro);
        }

        for workspace in workspaces.iter() {
            if let LapceWorkspaceType::RemoteWSL(host) = &workspace.kind {
                hosts.insert(host.host.clone());
            }
        }

        let items = hosts
            .iter()
            .map(|host| PaletteItem {
                content: PaletteItemContent::WslHost {
                    host: WslHost { host: host.clone() },
                },
                filter_text: host.to_string(),
                score: 0,
                indices: vec![],
                run_id,
            })
            .collect();
        self.items.set(items);
    }

    fn set_run_configs(
        &self,
        content: String,
        run_id: u64,
        input: &str,
    ) -> Result<()> {
        let configs: Option<RunDebugConfigs> = toml::from_str(&content).ok();
        if configs.is_none() {
            if let Some(path) = self.workspace.run_and_debug_path_with_create()? {
                self.common
                    .internal_command
                    .send(InternalCommand::OpenFile { path });
            }
        }

        let executed_run_configs = self.executed_run_configs.borrow();
        let mut items = Vec::new();
        if let Some(configs) = configs.as_ref() {
            for config in &configs.configs {
                items.push((
                    executed_run_configs
                        .get(&(RunDebugMode::Run, config.name.clone())),
                    PaletteItem {
                        content: PaletteItemContent::RunAndDebug {
                            mode:   RunDebugMode::Run,
                            config: config.clone(),
                        },
                        filter_text: format!(
                            "Run {} {} {}",
                            config.name,
                            config.program,
                            config.args.clone().unwrap_or_default().join(" ")
                        ),
                        score: 0,
                        indices: vec![],
                        run_id,
                    },
                ));
                if config.ty.is_some() {
                    items.push((
                        executed_run_configs
                            .get(&(RunDebugMode::Debug, config.name.clone())),
                        PaletteItem {
                            content: PaletteItemContent::RunAndDebug {
                                mode:   RunDebugMode::Debug,
                                config: config.clone(),
                            },
                            filter_text: format!(
                                "Debug {} {} {}",
                                config.name,
                                config.program,
                                config.args.clone().unwrap_or_default().join(" ")
                            ),
                            score: 0,
                            indices: vec![],
                            run_id,
                        },
                    ));
                }
            }
        }

        items.sort_by_key(|(executed, _item)| std::cmp::Reverse(executed.copied()));

        self.filter_items(
            run_id,
            input,
            items.into_iter().map(|(_, item)| item).collect(),
        );
        Ok(())
    }

    fn get_run_configs(&self, run_id: u64, input_str: String) -> Result<()> {
        if let Some(run_toml) =
            self.common.workspace.run_and_debug_path_with_create()?
        {
            let (doc, _new_doc) = self.main_split.get_doc_with_force(
                run_toml.clone(),
                None,
                false,
                DocContent::File {
                    path:      run_toml.clone(),
                    read_only: false,
                },
                true,
            );
            let loaded = doc.loaded;
            let palette = self.clone();
            self.common.scope.create_effect(move |prev_loaded| {
                if prev_loaded == Some(true) {
                    return true;
                }
                let loaded = loaded.get();
                if loaded {
                    let content =
                        doc.lines.with_untracked(|x| x.buffer().to_string());
                    if content.is_empty() {
                        doc.reload(Rope::from(DEFAULT_RUN_TOML), false);
                    }
                    if let Err(err) =
                        palette.set_run_configs(content, run_id, &input_str)
                    {
                        error!("{err}");
                    }
                }
                loaded
            });
        }
        Ok(())
    }

    fn get_color_themes(&self, run_id: u64) {
        let (items, name) = self.common.config.with_untracked(|config| {
            (
                config
                    .color_theme_list()
                    .iter()
                    .map(|name| PaletteItem {
                        content: PaletteItemContent::ColorTheme {
                            name: name.clone(),
                        },
                        filter_text: name.clone(),
                        score: 0,
                        indices: Vec::new(),
                        run_id,
                    })
                    .collect(),
                config.color_theme.name.clone(),
            )
        });

        self.preselect_matching(&items, &name);
        self.items.set(items);
    }

    fn get_icon_themes(&self, run_id: u64) {
        let (items, name) = self.common.config.with_untracked(|config| {
            (
                config
                    .icon_theme_list()
                    .iter()
                    .map(|name| PaletteItem {
                        content: PaletteItemContent::IconTheme {
                            name: name.clone(),
                        },
                        filter_text: name.clone(),
                        score: 0,
                        indices: Vec::new(),
                        run_id,
                    })
                    .collect(),
                config.icon_theme.name.clone(),
            )
        });
        self.preselect_matching(&items, &name);
        self.items.set(items);
    }

    fn get_languages(&self, run_id: u64) {
        let langs = LapceLanguage::languages();
        let items = langs
            .iter()
            .map(|lang| PaletteItem {
                content: PaletteItemContent::Language {
                    name: lang.to_string(),
                },
                filter_text: lang.to_string(),
                score: 0,
                indices: Vec::new(),
                run_id,
            })
            .collect();
        if let Some(editor) = self.main_split.active_editor.get_untracked() {
            let doc = editor.doc();
            let language = doc.lines.with_untracked(|x| x.syntax.language.name());
            self.preselect_matching(&items, language);
        }
        self.items.set(items);
    }

    fn get_line_endings(&self, run_id: u64) {
        let items = [LineEnding::Lf, LineEnding::CrLf]
            .iter()
            .map(|l| PaletteItem {
                content: PaletteItemContent::LineEnding { kind: *l },
                filter_text: l.as_str().to_string(),
                score: 0,
                indices: Vec::new(),
                run_id,
            })
            .collect();
        if let Some(editor) = self.main_split.active_editor.get_untracked() {
            let doc = editor.doc();
            let line_ending = doc.line_ending();
            self.preselect_matching(&items, line_ending.as_str());
        }
        self.items.set(items);
    }

    fn get_scm_references(&self, run_id: u64) {
        let branches = self.source_control.branches.get_untracked();
        let tags = self.source_control.tags.get_untracked();
        let mut items: im::Vector<PaletteItem> = im::Vector::new();
        for refs in branches.into_iter() {
            items.push_back(PaletteItem {
                content: PaletteItemContent::SCMReference {
                    name: refs.to_owned(),
                },
                filter_text: refs.to_owned(),
                score: 0,
                indices: Vec::new(),
                run_id,
            });
        }
        for refs in tags.into_iter() {
            items.push_back(PaletteItem {
                content: PaletteItemContent::SCMReference {
                    name: refs.to_owned(),
                },
                filter_text: refs.to_owned(),
                score: 0,
                indices: Vec::new(),
                run_id,
            });
        }
        self.items.set(items);
    }

    fn get_terminal_profiles(&self, run_id: u64) {
        let profiles = self
            .common
            .config
            .with_untracked(|x| x.terminal.profiles.clone());
        let mut items: im::Vector<PaletteItem> = im::Vector::new();

        for (name, profile) in profiles.into_iter() {
            let uri = match lsp_types::Url::parse(&format!(
                "file://{}",
                profile.workdir.unwrap_or_default().display()
            )) {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("Failed to parse uri: {e}");
                    None
                },
            };

            items.push_back(PaletteItem {
                content: PaletteItemContent::TerminalProfile {
                    name:    name.to_owned(),
                    profile: lapce_rpc::terminal::TerminalProfile {
                        name:        name.to_owned(),
                        command:     profile.command,
                        arguments:   profile.arguments,
                        workdir:     uri,
                        environment: profile.environment,
                    },
                },
                filter_text: name.to_owned(),
                score: 0,
                indices: Vec::new(),
                run_id,
            });
        }

        self.items.set(items);
    }

    fn preselect_matching(&self, items: &im::Vector<PaletteItem>, matching: &str) {
        let Some((idx, _)) = items
            .iter()
            .find_position(|item| item.filter_text == matching)
        else {
            return;
        };

        self.preselect_index.set(Some(idx));
    }

    fn select(&self) {
        let index = self.index.get_untracked();
        let items = self.run_result.get_untracked().rs;
        self.close();
        if let Some(item) = items.get(index) {
            match &item.content {
                PaletteItemContent::PaletteHelp { cmd } => {
                    let cmd = LapceCommand {
                        kind: CommandKind::Workbench(cmd.clone()),
                        data: None,
                    };

                    self.common.lapce_command.send(cmd);
                },
                PaletteItemContent::File { full_path, .. } => {
                    if self.kind.get_untracked() == Some(PaletteKind::DiffFiles) {
                        if let Some(left_path) =
                            self.left_diff_path.try_update(Option::take).flatten()
                        {
                            self.common.internal_command.send(
                                InternalCommand::OpenDiffFiles {
                                    left_path,
                                    right_path: full_path.clone(),
                                },
                            );
                        } else {
                            self.left_diff_path.set(Some(full_path.clone()));
                            self.run(PaletteKind::DiffFiles);
                        }
                    } else {
                        self.common.internal_command.send(
                            InternalCommand::OpenFile {
                                path: full_path.clone(),
                            },
                        );
                    }
                },
                PaletteItemContent::Line { line, .. } => {
                    let editor = self.main_split.active_editor.get_untracked();
                    let doc = match editor {
                        Some(editor) => editor.doc(),
                        None => {
                            return;
                        },
                    };
                    let path = doc
                        .content
                        .with_untracked(|content| content.path().cloned());
                    let path = match path {
                        Some(path) => path,
                        None => return,
                    };
                    self.common.internal_command.send(
                        InternalCommand::JumpToLocation {
                            location: EditorLocation {
                                path,
                                position: Some(EditorPosition::Line(*line)),
                                scroll_offset: None,
                                ignore_unconfirmed: false,
                                same_editor_tab: false,
                            },
                        },
                    );
                },
                PaletteItemContent::Command { cmd } => {
                    self.common.lapce_command.send(cmd.clone());
                },
                PaletteItemContent::Workspace { workspace } => {
                    self.common.window_common.window_command.send(
                        WindowCommand::SetWorkspace {
                            workspace: workspace.clone(),
                        },
                    );
                },
                PaletteItemContent::Reference { location, .. } => {
                    self.common.internal_command.send(
                        InternalCommand::JumpToLocation {
                            location: location.clone(),
                        },
                    );
                },
                PaletteItemContent::SshHost { host } => {
                    self.common.window_common.window_command.send(
                        WindowCommand::SetWorkspace {
                            workspace: Arc::new(LapceWorkspace {
                                kind:      LapceWorkspaceType::RemoteSSH(
                                    host.clone(),
                                ),
                                path:      None,
                                last_open: 0,
                            }),
                        },
                    );
                },
                #[cfg(windows)]
                PaletteItemContent::WslHost { host } => {
                    self.common.window_common.window_command.send(
                        WindowCommand::SetWorkspace {
                            workspace: Arc::new(LapceWorkspace {
                                kind:      LapceWorkspaceType::RemoteWSL(
                                    host.clone(),
                                ),
                                path:      None,
                                last_open: 0,
                            }),
                        },
                    );
                },
                PaletteItemContent::DocumentSymbol { range, .. } => {
                    let editor = self.main_split.active_editor.get_untracked();
                    let doc = match editor {
                        Some(editor) => editor.doc(),
                        None => {
                            return;
                        },
                    };
                    let path = doc
                        .content
                        .with_untracked(|content| content.path().cloned());
                    let path = match path {
                        Some(path) => path,
                        None => return,
                    };
                    self.common.internal_command.send(
                        InternalCommand::JumpToLocation {
                            location: EditorLocation {
                                path,
                                position: Some(EditorPosition::Position(
                                    range.start,
                                )),
                                scroll_offset: None,
                                ignore_unconfirmed: false,
                                same_editor_tab: false,
                            },
                        },
                    );
                },
                PaletteItemContent::WorkspaceSymbol { location, .. } => {
                    self.common.internal_command.send(
                        InternalCommand::JumpToLocation {
                            location: location.clone(),
                        },
                    );
                },
                PaletteItemContent::RunAndDebug { mode, config } => {
                    self.common.internal_command.send(
                        InternalCommand::RunAndDebug {
                            mode:   *mode,
                            config: config.clone(),
                        },
                    );
                },
                PaletteItemContent::ColorTheme { name } => self
                    .common
                    .internal_command
                    .send(InternalCommand::SetColorTheme {
                        name: name.clone(),
                        save: true,
                    }),
                PaletteItemContent::IconTheme { name } => self
                    .common
                    .internal_command
                    .send(InternalCommand::SetIconTheme {
                        name: name.clone(),
                        save: true,
                    }),
                PaletteItemContent::Language { name } => {
                    let editor = self.main_split.active_editor.get_untracked();
                    let doc = match editor {
                        Some(editor) => editor.doc(),
                        None => {
                            return;
                        },
                    };

                    let queries_directory = &self.common.directory.queries_directory;
                    let grammars_directory =
                        &self.common.directory.grammars_directory;
                    if name.is_empty() || name.to_lowercase().eq("plain text") {
                        doc.set_syntax(Syntax::plaintext(
                            grammars_directory,
                            queries_directory,
                        ))
                    } else {
                        let lang = match LapceLanguage::from_name(name) {
                            Some(v) => v,
                            None => return,
                        };
                        doc.set_language(lang);
                    }
                    doc.trigger_syntax_change(None);
                },
                PaletteItemContent::LineEnding { kind } => {
                    let Some(editor) = self.main_split.active_editor.get_untracked()
                    else {
                        return;
                    };
                    let doc = editor.doc();
                    doc.buffer_edit(EditBuffer::SetLineEnding(*kind));
                    // doc.lines.update(|lines| {
                    //     // todo maybe should upate content
                    //     lines.set_line_ending(*kind);
                    // });
                },
                PaletteItemContent::SCMReference { name } => {
                    self.common
                        .lapce_command
                        .send(crate::command::LapceCommand {
                        kind: CommandKind::Workbench(
                            crate::command::LapceWorkbenchCommand::CheckoutReference,
                        ),
                        data: Some(serde_json::json!(name.to_owned())),
                    });
                },
                PaletteItemContent::TerminalProfile { name: _, profile } => self
                    .common
                    .internal_command
                    .send(InternalCommand::NewTerminal {
                        profile: Some(profile.to_owned()),
                    }),
            }
        } else if self.kind.get_untracked() == Some(PaletteKind::SshHost) {
            let input = self.input.with_untracked(|input| input.input.clone());
            let ssh = SshHost::from_string(&input);
            self.common.window_common.window_command.send(
                WindowCommand::SetWorkspace {
                    workspace: Arc::new(LapceWorkspace {
                        kind:      LapceWorkspaceType::RemoteSSH(ssh),
                        path:      None,
                        last_open: 0,
                    }),
                },
            );
        }
    }

    /// Update the preview for the currently active palette item, if it has one.
    fn preview(&self) {
        if self.status.get_untracked() == PaletteStatus::Inactive {
            return;
        }

        let index = self.index.get_untracked();
        let item = self.run_result.with_untracked(|x| x.rs.get(index).cloned());
        // info!("preview index={index} {item:?}");
        let mut has_preview = false;
        if let Some(item) = item {
            match &item.content {
                PaletteItemContent::PaletteHelp { .. } => {},
                PaletteItemContent::File { .. } => {},
                PaletteItemContent::Line { line, .. } => {
                    has_preview = true;
                    let editor = self.main_split.active_editor.get_untracked();
                    let doc = match editor {
                        Some(editor) => editor.doc(),
                        None => {
                            return;
                        },
                    };
                    let path = doc
                        .content
                        .with_untracked(|content| content.path().cloned());
                    let path = match path {
                        Some(path) => path,
                        None => return,
                    };
                    self.preview_editor.update_doc(doc);
                    self.preview_editor.go_to_location(
                        EditorLocation {
                            path,
                            position: Some(EditorPosition::Line(*line)),
                            scroll_offset: None,
                            ignore_unconfirmed: false,
                            same_editor_tab: false,
                        },
                        false,
                        None,
                        None,
                    );
                },
                PaletteItemContent::Command { .. } => {},
                PaletteItemContent::Workspace { .. } => {},
                PaletteItemContent::RunAndDebug { .. } => {},
                PaletteItemContent::SshHost { .. } => {},
                #[cfg(windows)]
                PaletteItemContent::WslHost { .. } => {},
                PaletteItemContent::Language { .. } => {},
                PaletteItemContent::LineEnding { .. } => {},
                PaletteItemContent::Reference { location, .. } => {
                    has_preview = true;
                    let (doc, new_doc) = self.main_split.get_doc(
                        location.path.clone(),
                        None,
                        false,
                        DocContent::File {
                            path:      location.path.clone(),
                            read_only: true,
                        },
                    );
                    self.preview_editor.update_doc(doc);
                    self.preview_editor.go_to_location(
                        location.clone(),
                        new_doc,
                        None,
                        None,
                    );
                },
                PaletteItemContent::DocumentSymbol { range, .. } => {
                    has_preview = true;
                    let editor = self.main_split.active_editor.get_untracked();
                    let doc = match editor {
                        Some(editor) => editor.doc(),
                        None => {
                            return;
                        },
                    };
                    let path = doc
                        .content
                        .with_untracked(|content| content.path().cloned());
                    let path = match path {
                        Some(path) => path,
                        None => return,
                    };
                    self.preview_editor.update_doc(doc);
                    self.preview_editor.go_to_location(
                        EditorLocation {
                            path,
                            position: Some(EditorPosition::Position(range.start)),
                            scroll_offset: None,
                            ignore_unconfirmed: false,
                            same_editor_tab: false,
                        },
                        false,
                        None,
                        None,
                    );
                },
                PaletteItemContent::WorkspaceSymbol { location, .. } => {
                    has_preview = true;
                    let (doc, new_doc) = self.main_split.get_doc(
                        location.path.clone(),
                        None,
                        false,
                        DocContent::File {
                            path:      location.path.clone(),
                            read_only: true,
                        },
                    );
                    self.preview_editor.update_doc(doc);
                    self.preview_editor.go_to_location(
                        location.clone(),
                        new_doc,
                        None,
                        None,
                    );
                },
                PaletteItemContent::ColorTheme { name } => self
                    .common
                    .internal_command
                    .send(InternalCommand::SetColorTheme {
                        name: name.clone(),
                        save: false,
                    }),
                PaletteItemContent::IconTheme { name } => self
                    .common
                    .internal_command
                    .send(InternalCommand::SetIconTheme {
                        name: name.clone(),
                        save: false,
                    }),
                PaletteItemContent::SCMReference { .. } => {},
                PaletteItemContent::TerminalProfile { .. } => {},
            }
            self.has_preview.set(has_preview);
        }
    }

    /// Cancel the palette, doing cleanup specific to the palette kind.
    fn cancel(&self) {
        if let Some(PaletteKind::ColorTheme | PaletteKind::IconTheme) =
            self.kind.get_untracked()
        {
            // TODO(minor): We don't really need to reload the *entire config* here!
            self.common
                .internal_command
                .send(InternalCommand::ReloadConfig);
        }

        self.left_diff_path.set(None);
        self.close();
    }

    /// Close the palette, reverting focus back to the workbench.
    fn close(&self) {
        self.status.set(PaletteStatus::Inactive);
        if self.common.focus.get_untracked() == Focus::Palette {
            self.common.focus.set(Focus::Workbench);
        }
        self.has_preview.set(false);
        self.items.update(|items| items.clear());
        self.input_str.set(String::new());
        self.kind.set(None);
        // self.input_editor.doc().reload(Rope::from(""), true);
        // self.input_editor
        //     .cursor()
        //     .update(|cursor| cursor.set_insert(Selection::caret(0)));
    }

    /// Move to the next entry in the palette list, wrapping around if needed.
    fn next(&self) {
        let index = self.index.get_untracked();
        let len = self.run_result.with_untracked(|i| i.rs.len());
        let new_index = Movement::Down.update_index(index, len, 1, true);
        self.index.set(new_index);
    }

    /// Move to the previous entry in the palette list, wrapping around if
    /// needed.
    fn previous(&self) {
        let index = self.index.get_untracked();
        let len = self.run_result.with_untracked(|i| i.rs.len());
        let new_index = Movement::Up.update_index(index, len, 1, true);
        self.index.set(new_index);
    }

    fn next_page(&self) {
        // TODO: implement
    }

    fn previous_page(&self) {
        // TODO: implement
    }

    fn run_focus_command(&self, cmd: &FocusCommand) -> CommandExecuted {
        match cmd {
            FocusCommand::ModalClose => {
                self.cancel();
            },
            FocusCommand::ListNext => {
                self.next();
            },
            FocusCommand::ListNextPage => {
                self.next_page();
            },
            FocusCommand::ListPrevious => {
                self.previous();
            },
            FocusCommand::ListPreviousPage => {
                self.previous_page();
            },
            FocusCommand::ListSelect => {
                self.select();
            },
            _ => return CommandExecuted::No,
        }
        CommandExecuted::Yes
    }

    fn filter_items(&self, id: u64, input: &str, items: im::Vector<PaletteItem>) {
        let equal_id = self.run_result.get_untracked().id == id;
        // info!(
        //     "filter_items {id} input={input} items={} equal_id={equal_id}",
        //     items.len()
        // );
        if !equal_id {
            return;
        }

        if input.is_empty() {
            self.update_rs(id, items);
            return;
        }
        let mut matcher =
            nucleo::Matcher::new(nucleo::Config::DEFAULT.match_paths());

        let pattern = nucleo::pattern::Pattern::parse(
            input,
            nucleo::pattern::CaseMatching::Ignore,
            nucleo::pattern::Normalization::Smart,
        );

        // NOTE: We collect into a Vec to sort as we are hitting a worst-case
        // behavior in `im::Vector` that can lead to a stack overflow!
        let mut filtered_items = Vec::new();
        let mut indices = Vec::new();
        let mut filter_text_buf = Vec::new();
        for i in &items {
            // If the run id has ever changed, then we'll just bail out of this
            // filtering to avoid wasting effort. This would happen, for
            // example, on the user continuing to type.
            indices.clear();
            filter_text_buf.clear();
            let filter_text = Utf32Str::new(&i.filter_text, &mut filter_text_buf);
            if let Some(score) =
                pattern.indices(filter_text, &mut matcher, &mut indices)
            {
                let mut item = i.clone();
                item.score = score;
                item.indices = indices.iter().map(|i| *i as usize).collect();
                filtered_items.push(item);
            }
        }

        filtered_items.sort_by(|a, b| {
            let order = b.score.cmp(&a.score);
            match order {
                std::cmp::Ordering::Equal => a.filter_text.cmp(&b.filter_text),
                _ => order,
            }
        });
        self.update_rs(id, filtered_items.into());
    }
}

impl KeyPressFocus for PaletteData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(&self, condition: Condition) -> bool {
        matches!(
            condition,
            Condition::ListFocus | Condition::PaletteFocus | Condition::ModalFocus
        )
    }

    fn run_command(
        &self,
        command: &LapceCommand,
        _count: Option<usize>,
        _mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(_cmd) => {
                if matches!(_cmd, LapceWorkbenchCommand::OpenUIInspector) {
                    self.common.view_id.get_untracked().inspect();
                }
            },
            CommandKind::Scroll(_) => {},
            CommandKind::Focus(cmd) => {
                self.run_focus_command(cmd);
            },
            CommandKind::Edit(_)
            | CommandKind::Move(_)
            | CommandKind::MultiSelection(_) => {
                error!("todo run_command {command:?}");
                // self.input_editor.run_command(command, count, mods);
            },
            CommandKind::MotionMode(_) => {},
        }
        CommandExecuted::Yes
    }

    fn receive_char(&self, _c: &str) {
        error!("todo receive_char");
    }
}

use std::{
    path::{Path, PathBuf},
    rc::Rc
};

use floem::{
    peniko::{
        Color,
        kurbo::{Point, Rect}
    },
    reactive::{
        Memo, ReadSignal, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith,
        WriteSignal, create_memo
    }
};
use lapce_core::{
    doc::DocContent,
    editor_tab::{EditorTabChildInfo, EditorTabInfo},
    icon::LapceIcons,
    id::{
        DiffEditorId, EditorId, EditorTabManageId, KeymapId, SettingsId, SplitId,
        ThemeColorSettingsId, VoltViewId
    }
};
use lapce_rpc::plugin::VoltID;

use crate::{
    config::{WithLapceConfig, color::LapceColor},
    doc::Doc,
    editor::{EditorData, diff::DiffEditorData, location::EditorLocation},
    main_split::Editors,
    plugin::PluginData,
    window_workspace::WindowWorkspaceData
};

pub enum EditorTabChildSource {
    Editor { path: PathBuf, doc: Rc<Doc> },
    DiffEditor { left: Rc<Doc>, right: Rc<Doc> },
    NewFileEditor,
    Settings,
    ThemeColorSettings,
    Keymap,
    Volt(VoltID)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorTabChildId {
    Editor(EditorId),
    DiffEditor(DiffEditorId),
    Settings(SettingsId),
    ThemeColorSettings(ThemeColorSettingsId),
    Keymap(KeymapId),
    Volt(VoltViewId, VoltID)
}

#[derive(PartialEq)]
pub struct EditorTabChildViewInfo {
    pub icon:        String,
    pub color:       Option<Color>,
    pub name:        String,
    pub path:        Option<PathBuf>,
    pub confirmed:   RwSignal<bool>,
    pub is_pristine: bool
}

impl EditorTabChildId {
    pub fn id(&self) -> u64 {
        match self {
            EditorTabChildId::Editor(id) => id.to_raw(),
            EditorTabChildId::DiffEditor(id) => id.to_raw(),
            EditorTabChildId::Settings(id) => id.to_raw(),
            EditorTabChildId::ThemeColorSettings(id) => id.to_raw(),
            EditorTabChildId::Keymap(id) => id.to_raw(),
            EditorTabChildId::Volt(id, _) => id.to_raw()
        }
    }

    pub fn is_settings(&self) -> bool {
        matches!(self, EditorTabChildId::Settings(_))
    }

    pub fn child_info(&self, data: &WindowWorkspaceData) -> EditorTabChildInfo {
        match &self {
            EditorTabChildId::Editor(editor_id) => {
                let editor_data = data
                    .main_split
                    .editors
                    .editor_untracked(*editor_id)
                    .unwrap();
                EditorTabChildInfo::Editor(editor_data.editor_info(data))
            },
            EditorTabChildId::DiffEditor(diff_editor_id) => {
                let diff_editor_data = data
                    .main_split
                    .diff_editors
                    .get_untracked()
                    .get(diff_editor_id)
                    .cloned()
                    .unwrap();
                EditorTabChildInfo::DiffEditor(diff_editor_data.diff_editor_info())
            },
            EditorTabChildId::Settings(_) => EditorTabChildInfo::Settings,
            EditorTabChildId::ThemeColorSettings(_) => {
                EditorTabChildInfo::ThemeColorSettings
            },
            EditorTabChildId::Keymap(_) => EditorTabChildInfo::Keymap,
            EditorTabChildId::Volt(_, id) => EditorTabChildInfo::Volt(id.to_owned())
        }
    }

    pub fn view_info(
        &self,
        editors: Editors,
        diff_editors: RwSignal<im::HashMap<DiffEditorId, DiffEditorData>>,
        plugin: PluginData,
        config: WithLapceConfig,
        confirmed: RwSignal<bool>
    ) -> Memo<EditorTabChildViewInfo> {
        match self.clone() {
            EditorTabChildId::Editor(editor_id) => create_memo(move |_| {
                let editor_data = editors.editor(editor_id);
                let path = if let Some(editor_data) = editor_data {
                    let doc = editor_data.doc_signal().get();
                    let is_pristine =
                        doc.lines.with_untracked(|x| x.signal_pristine()).get();
                    if !is_pristine {
                        confirmed.set(true);
                    }
                    let content = doc.content.get();
                    match content {
                        DocContent::File { path, .. } => Some((path, is_pristine)),
                        DocContent::Local => None,
                        DocContent::History(_) => None,
                        DocContent::Scratch { name, .. } => {
                            Some((PathBuf::from(name), is_pristine))
                        },
                    }
                } else {
                    None
                };

                let (icon, color, name, is_pristine) = match path {
                    Some((ref path, is_pritine)) => {
                        let icon_theme =
                            config.signal(|config| config.icon_theme.signal());
                        let (svg, color) = icon_theme.with(|x| x.file_svg(&path));

                        // let (svg, color) =
                        //     config.with(|config| config.file_svg(path));
                        (
                            svg,
                            color,
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            is_pritine
                        )
                    },
                    None => {
                        let (svg, color) = config.signal(|config| {
                            (
                                config.ui_svg(LapceIcons::FILE),
                                config.color(LapceColor::LAPCE_ICON_ACTIVE)
                            )
                        });
                        (svg.get(), Some(color.get()), "local".to_string(), true)
                    }
                };
                EditorTabChildViewInfo {
                    icon,
                    color,
                    name,
                    path: path.map(|opt| opt.0),
                    confirmed,
                    is_pristine
                }
            }),
            EditorTabChildId::DiffEditor(diff_editor_id) => create_memo(move |_| {
                let diff_editor_data = diff_editors
                    .with(|diff_editors| diff_editors.get(&diff_editor_id).cloned());
                // let confirmed = diff_editor_data.as_ref().map(|d| d.confirmed);

                let info = diff_editor_data
                    .map(|diff_editor_data| {
                        [diff_editor_data.left, diff_editor_data.right].map(|data| {
                            let (content, is_pristine) =
                                data.doc_signal().with(|doc| {
                                    let buffer = doc
                                        .lines
                                        .with_untracked(|x| x.signal_buffer());
                                    (
                                        doc.content.get(),
                                        buffer.with(|b| b.is_pristine())
                                    )
                                });
                            match content {
                                DocContent::File { path, .. } => {
                                    Some((path, is_pristine))
                                },
                                DocContent::Local => None,
                                DocContent::History(_) => None,
                                DocContent::Scratch { name, .. } => {
                                    Some((PathBuf::from(name), is_pristine))
                                },
                            }
                        })
                    })
                    .unwrap_or([None, None]);

                let (icon, color, path, is_pristine) = match info {
                    [Some((path, is_pristine)), None]
                    | [None, Some((path, is_pristine))] => {
                        let file_name = format!(
                            "{} (Diff)",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        // let (svg, color) =
                        //     config.with(|config| config.file_svg(&path));
                        let icon_theme =
                            config.signal(|config| config.icon_theme.signal());
                        let (svg, color) = icon_theme.with(|x| x.file_svg(&path));

                        (svg, color, file_name, is_pristine)
                    },
                    [
                        Some((left_path, left_is_pristine)),
                        Some((right_path, right_is_pristine))
                    ] => {
                        let icon_theme =
                            config.signal(|config| config.icon_theme.signal());
                        let (svg, color) = icon_theme
                            .with(|x| x.files_svg(&[&left_path, &right_path]));

                        let [left_file_name, right_file_name] =
                            [&left_path, &right_path].map(|path| {
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            });
                        (
                            svg,
                            color,
                            format!("{left_file_name} - {right_file_name} (Diff)"),
                            left_is_pristine && right_is_pristine
                        )
                    },
                    [None, None] => {
                        let (svg, color) = config.signal(|config| {
                            (
                                config.ui_svg(LapceIcons::FILE),
                                config.color(LapceColor::LAPCE_ICON_ACTIVE)
                            )
                        });
                        (svg.get(), Some(color.get()), "local".to_string(), true)
                    }
                };
                EditorTabChildViewInfo {
                    icon,
                    color,
                    name: path,
                    path: None,
                    confirmed,
                    is_pristine
                }
            }),
            EditorTabChildId::Settings(_) => create_memo(move |_| {
                let (caret_color, ui_svg) = config.signal(|config| {
                    (
                        config.color(LapceColor::LAPCE_ICON_ACTIVE),
                        config.ui_svg(LapceIcons::SETTINGS)
                    )
                });
                EditorTabChildViewInfo {
                    icon: ui_svg.get(),
                    color: Some(caret_color.get()),
                    name: "Settings".to_string(),
                    path: None,
                    confirmed,
                    is_pristine: true
                }
            }),
            EditorTabChildId::ThemeColorSettings(_) => create_memo(move |_| {
                let (caret_color, ui_svg) = config.signal(|config| {
                    (
                        config.color(LapceColor::LAPCE_ICON_ACTIVE),
                        config.ui_svg(LapceIcons::SYMBOL_COLOR)
                    )
                });
                EditorTabChildViewInfo {
                    icon: ui_svg.get(),
                    color: Some(caret_color.get()),
                    name: "Theme Colors".to_string(),
                    path: None,
                    confirmed,
                    is_pristine: true
                }
            }),
            EditorTabChildId::Keymap(_) => create_memo(move |_| {
                let (caret_color, ui_svg) = config.signal(|config| {
                    (
                        config.color(LapceColor::LAPCE_ICON_ACTIVE),
                        config.ui_svg(LapceIcons::KEYBOARD)
                    )
                });
                EditorTabChildViewInfo {
                    icon: ui_svg.get(),
                    color: Some(caret_color.get()),
                    name: "Keyboard Shortcuts".to_string(),
                    path: None,
                    confirmed,
                    is_pristine: true
                }
            }),
            EditorTabChildId::Volt(_, id) => create_memo(move |_| {
                let display_name = plugin
                    .installed
                    .with(|volts| volts.get(&id).cloned())
                    .map(|volt| volt.meta.with(|m| m.display_name.clone()))
                    .or_else(|| {
                        plugin.available.volts.with(|volts| {
                            let volt = volts.get(&id);
                            volt.map(|volt| {
                                volt.info.with(|m| m.display_name.clone())
                            })
                        })
                    })
                    .unwrap_or_else(|| id.name.clone());
                let (caret_color, ui_svg) = config.signal(|config| {
                    (
                        config.color(LapceColor::LAPCE_ICON_ACTIVE),
                        config.ui_svg(LapceIcons::EXTENSIONS)
                    )
                });
                EditorTabChildViewInfo {
                    icon: ui_svg.get(),
                    color: Some(caret_color.get()),
                    name: display_name,
                    path: None,
                    confirmed,
                    is_pristine: true
                }
            })
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditorTabChildSimple {
    index:     RwSignal<usize>,
    position:  RwSignal<Rect>,
    confirmed: RwSignal<bool>,
    id:        EditorTabChildId
}

impl EditorTabChildSimple {
    pub fn new(
        index: RwSignal<usize>,
        position: RwSignal<Rect>,
        id: EditorTabChildId,
        confirmed: RwSignal<bool>
    ) -> Self {
        match &id {
            EditorTabChildId::Editor(_) => {},
            EditorTabChildId::DiffEditor(_) => {},
            EditorTabChildId::Settings(_) => confirmed.set(true),
            EditorTabChildId::ThemeColorSettings(_) => confirmed.set(true),
            EditorTabChildId::Keymap(_) => confirmed.set(true),
            EditorTabChildId::Volt(_, _) => confirmed.set(true)
        }
        Self {
            index,
            position,
            id,
            confirmed
        }
    }

    pub fn update_index_with_judgment(&self, index: usize) {
        if self.index.get_untracked() != index {
            self.index.set(index);
        }
    }

    pub fn layout_untracted(&self) -> Rect {
        self.position.get_untracked()
    }

    pub fn layout_tracing(&self) -> Rect {
        self.position.get()
    }

    pub fn write_layout(&self) -> WriteSignal<Rect> {
        self.position.write_only()
    }

    pub fn read_index(&self) -> ReadSignal<usize> {
        self.index.read_only()
    }

    pub fn confirmed_mut(&self) -> RwSignal<bool> {
        self.confirmed
    }

    pub fn id(&self) -> &EditorTabChildId {
        &self.id
    }
}

#[derive(Clone)]
pub struct EditorTabDraging {
    editor_tab_child_index: ReadSignal<usize>,
    editor_tab_manage_id:   EditorTabManageId
}

impl EditorTabDraging {
    pub fn new(
        editor_tab_child_index: ReadSignal<usize>,
        editor_tab_manage_id: EditorTabManageId
    ) -> Self {
        Self {
            editor_tab_child_index,
            editor_tab_manage_id
        }
    }

    pub fn data(&self) -> (usize, EditorTabManageId) {
        (
            self.editor_tab_child_index.get_untracked(),
            self.editor_tab_manage_id
        )
    }
}

#[derive(Clone)]
pub struct EditorTabManageData {
    pub scope:                Scope,
    pub split:                SplitId,
    pub editor_tab_manage_id: EditorTabManageId,
    pub active:               usize,
    pub children:             Vec<EditorTabChildSimple>,
    pub window_origin:        RwSignal<Point>,
    pub layout_rect:          RwSignal<Rect>,
    pub locations:            RwSignal<im::Vector<EditorLocation>>,
    pub current_location:     RwSignal<usize>
}

impl EditorTabManageData {
    pub fn active_child(&self) -> &EditorTabChildSimple {
        &self.children[self.active]
    }

    pub fn get_editor(
        &self,
        editors: Editors,
        path: &Path
    ) -> Option<(usize, EditorData)> {
        for (i, child) in self.children.iter().enumerate() {
            if let EditorTabChildId::Editor(editor_id) = child.id() {
                if let Some(editor) = editors.editor_untracked(*editor_id) {
                    let is_path = editor.doc().content.with_untracked(|content| {
                        if let DocContent::File { path: p, .. } = content {
                            p == path
                        } else {
                            false
                        }
                    });
                    if is_path {
                        return Some((i, editor));
                    }
                }
            }
        }
        None
    }

    pub fn get_unconfirmed_editor_tab_child(
        &self /* editors: Editors,
               * diff_editors: &im::HashMap<EditorId, DiffEditorData>, */
    ) -> Option<(usize, EditorTabChildId)> {
        for (i, child) in self.children.iter().enumerate() {
            let confirmed = child.confirmed.get_untracked();
            if !confirmed {
                return Some((i, child.id().clone()));
            }
            // match child.id() {
            //     EditorTabChildId::Editor(editor_id) => {
            //         if let Some(editor) =
            // editors.editor_untracked(*editor_id) {
            // let confirmed = editor.confirmed.get_untracked();
            //             if !confirmed {
            //                 return Some((i, child.id().clone()));
            //             }
            //         }
            //     },
            //     EditorTabChildId::DiffEditor(diff_editor_id) => {
            //         if let Some(diff_editor) =
            // diff_editors.get(diff_editor_id) {             let
            // confirmed = diff_editor.confirmed.get_untracked();
            //             if !confirmed {
            //                 return Some((i, child.id().clone()));
            //             }
            //         }
            //     },
            //     _ => (),
            // }
        }
        None
    }

    pub fn tab_info(&self, data: &WindowWorkspaceData) -> EditorTabInfo {
        let info = EditorTabInfo {
            active:   self.active,
            is_focus: data.main_split.active_editor_tab.get_untracked()
                == Some(self.editor_tab_manage_id),
            children: self
                .children
                .iter()
                .map(|child| child.id().child_info(data))
                .collect()
        };
        info
    }
}

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use floem::{
    peniko::{
        kurbo::{Point, Rect},
        Color,
    },
    reactive::{
        create_memo, create_rw_signal, Memo, ReadSignal, RwSignal, Scope, SignalGet,
        SignalUpdate, SignalWith,
    },
    views::editor::id::EditorId,
};
use floem::reactive::WriteSignal;
use lapce_rpc::plugin::VoltID;
use serde::{Deserialize, Serialize};

use crate::{
    config::{color::LapceColor, icon::LapceIcons, LapceConfig},
    doc::{Doc, DocContent},
    editor::{
        diff::{DiffEditorData, DiffEditorInfo},
        location::EditorLocation,
        EditorData, EditorInfo,
    },
    id::{
        DiffEditorId, EditorTabManageId, KeymapId, SettingsId, SplitId,
        ThemeColorSettingsId, VoltViewId,
    },
    main_split::{Editors, MainSplitData},
    plugin::PluginData,
    window_workspace::WindowWorkspaceData,
};

#[derive(Clone, Serialize, Deserialize)]
pub enum EditorTabChildInfo {
    Editor(EditorInfo),
    DiffEditor(DiffEditorInfo),
    Settings,
    ThemeColorSettings,
    Keymap,
    Volt(VoltID),
}

impl EditorTabChildInfo {
    pub fn to_data(
        &self,
        data: MainSplitData,
        editor_tab_id: EditorTabManageId,
    ) -> EditorTabChildId {
        match &self {
            EditorTabChildInfo::Editor(editor_info) => {
                let editor_id = editor_info.to_data(data, editor_tab_id);
                EditorTabChildId::Editor(editor_id)
            },
            EditorTabChildInfo::DiffEditor(diff_editor_info) => {
                let diff_editor_data = diff_editor_info.to_data(data, editor_tab_id);
                EditorTabChildId::DiffEditor(diff_editor_data.id)
            },
            EditorTabChildInfo::Settings => {
                EditorTabChildId::Settings(SettingsId::next())
            },
            EditorTabChildInfo::ThemeColorSettings => {
                EditorTabChildId::ThemeColorSettings(ThemeColorSettingsId::next())
            },
            EditorTabChildInfo::Keymap => EditorTabChildId::Keymap(KeymapId::next()),
            EditorTabChildInfo::Volt(id) => {
                EditorTabChildId::Volt(VoltViewId::next(), id.to_owned())
            },
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorTabInfo {
    pub active: usize,
    pub is_focus: bool,
    pub children: Vec<EditorTabChildInfo>,
}

impl EditorTabInfo {
    pub fn to_data(
        &self,
        data: MainSplitData,
        split: SplitId,
    ) -> RwSignal<EditorTabManageData> {
        let editor_tab_id = EditorTabManageId::next();
        let editor_tab_data = {
            let cx = data.scope.create_child();
            let editor_tab_data = EditorTabManageData {
                scope: cx,
                editor_tab_manage_id: editor_tab_id,
                split,
                active: self.active,
                children: self
                    .children
                    .iter()
                    .map(|child| {
                        EditorTabChildSimple::new(
                            cx.create_rw_signal(0),
                            cx.create_rw_signal(Rect::ZERO),
                            child.to_data(data.clone(), editor_tab_id),
                        )
                    })
                    .collect(),
                layout_rect: Rect::ZERO,
                window_origin: Point::ZERO,
                locations: cx.create_rw_signal(im::Vector::new()),
                current_location: cx.create_rw_signal(0),
            };
            cx.create_rw_signal(editor_tab_data)
        };
        if self.is_focus {
            data.active_editor_tab.set(Some(editor_tab_id));
        }
        data.editor_tabs.update(|editor_tabs| {
            editor_tabs.insert(editor_tab_id, editor_tab_data);
        });
        editor_tab_data
    }
}

pub enum EditorTabChildSource {
    Editor { path: PathBuf, doc: Rc<Doc> },
    DiffEditor { left: Rc<Doc>, right: Rc<Doc> },
    NewFileEditor,
    Settings,
    ThemeColorSettings,
    Keymap,
    Volt(VoltID),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorTabChildId {
    Editor(EditorId),
    DiffEditor(DiffEditorId),
    Settings(SettingsId),
    ThemeColorSettings(ThemeColorSettingsId),
    Keymap(KeymapId),
    Volt(VoltViewId, VoltID),
}

#[derive(PartialEq)]
pub struct EditorTabChildViewInfo {
    pub icon: String,
    pub color: Option<Color>,
    pub name: String,
    pub path: Option<PathBuf>,
    pub confirmed: Option<RwSignal<bool>>,
    pub is_pristine: bool,
}

impl EditorTabChildId {
    pub fn id(&self) -> u64 {
        match self {
            EditorTabChildId::Editor(id) => id.to_raw(),
            EditorTabChildId::DiffEditor(id) => id.to_raw(),
            EditorTabChildId::Settings(id) => id.to_raw(),
            EditorTabChildId::ThemeColorSettings(id) => id.to_raw(),
            EditorTabChildId::Keymap(id) => id.to_raw(),
            EditorTabChildId::Volt(id, _) => id.to_raw(),
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
            EditorTabChildId::Volt(_, id) => EditorTabChildInfo::Volt(id.to_owned()),
        }
    }

    pub fn view_info(
        &self,
        editors: Editors,
        diff_editors: RwSignal<im::HashMap<DiffEditorId, DiffEditorData>>,
        plugin: PluginData,
        config: ReadSignal<Arc<LapceConfig>>,
    ) -> Memo<EditorTabChildViewInfo> {
        match self.clone() {
            EditorTabChildId::Editor(editor_id) => create_memo(move |_| {
                let config = config.get();
                let editor_data = editors.editor(editor_id);
                let path = if let Some(editor_data) = editor_data {
                    let doc = editor_data.doc_signal().get();
                    let is_pristine =
                        doc.lines.with_untracked(|x| x.signal_pristine()).get();
                    let (content, confirmed) =
                        (doc.content.get(), editor_data.confirmed);
                    match content {
                        DocContent::File { path, .. } => {
                            Some((path, confirmed, is_pristine))
                        },
                        DocContent::Local => None,
                        DocContent::History(_) => None,
                        DocContent::Scratch { name, .. } => {
                            Some((PathBuf::from(name), confirmed, is_pristine))
                        },
                    }
                } else {
                    None
                };
                let (icon, color, name, confirmed, is_pristine) = match path {
                    Some((ref path, confirmed, is_pritine)) => {
                        let (svg, color) = config.file_svg(path);
                        (
                            svg,
                            color,
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            confirmed,
                            is_pritine,
                        )
                    },
                    None => (
                        config.ui_svg(LapceIcons::FILE),
                        Some(config.color(LapceColor::LAPCE_ICON_ACTIVE)),
                        "local".to_string(),
                        create_rw_signal(true),
                        true,
                    ),
                };
                EditorTabChildViewInfo {
                    icon,
                    color,
                    name,
                    path: path.map(|opt| opt.0),
                    confirmed: Some(confirmed),
                    is_pristine,
                }
            }),
            EditorTabChildId::DiffEditor(diff_editor_id) => create_memo(move |_| {
                let config = config.get();
                let diff_editor_data = diff_editors
                    .with(|diff_editors| diff_editors.get(&diff_editor_id).cloned());
                let confirmed = diff_editor_data.as_ref().map(|d| d.confirmed);

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
                                        buffer.with(|b| b.is_pristine()),
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
                        let (svg, color) = config.file_svg(&path);
                        (
                            svg,
                            color,
                            format!(
                                "{} (Diff)",
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                            ),
                            is_pristine,
                        )
                    },
                    [Some((left_path, left_is_pristine)), Some((right_path, right_is_pristine))] =>
                    {
                        let (svg, color) =
                            config.files_svg(&[&left_path, &right_path]);
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
                            left_is_pristine && right_is_pristine,
                        )
                    },
                    [None, None] => (
                        config.ui_svg(LapceIcons::FILE),
                        Some(config.color(LapceColor::LAPCE_ICON_ACTIVE)),
                        "local".to_string(),
                        true,
                    ),
                };
                EditorTabChildViewInfo {
                    icon,
                    color,
                    name: path,
                    path: None,
                    confirmed,
                    is_pristine,
                }
            }),
            EditorTabChildId::Settings(_) => create_memo(move |_| {
                let config = config.get();
                EditorTabChildViewInfo {
                    icon: config.ui_svg(LapceIcons::SETTINGS),
                    color: Some(config.color(LapceColor::LAPCE_ICON_ACTIVE)),
                    name: "Settings".to_string(),
                    path: None,
                    confirmed: None,
                    is_pristine: true,
                }
            }),
            EditorTabChildId::ThemeColorSettings(_) => create_memo(move |_| {
                let config = config.get();
                EditorTabChildViewInfo {
                    icon: config.ui_svg(LapceIcons::SYMBOL_COLOR),
                    color: Some(config.color(LapceColor::LAPCE_ICON_ACTIVE)),
                    name: "Theme Colors".to_string(),
                    path: None,
                    confirmed: None,
                    is_pristine: true,
                }
            }),
            EditorTabChildId::Keymap(_) => create_memo(move |_| {
                let config = config.get();
                EditorTabChildViewInfo {
                    icon: config.ui_svg(LapceIcons::KEYBOARD),
                    color: Some(config.color(LapceColor::LAPCE_ICON_ACTIVE)),
                    name: "Keyboard Shortcuts".to_string(),
                    path: None,
                    confirmed: None,
                    is_pristine: true,
                }
            }),
            EditorTabChildId::Volt(_, id) => create_memo(move |_| {
                let config = config.get();
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
                EditorTabChildViewInfo {
                    icon: config.ui_svg(LapceIcons::EXTENSIONS),
                    color: Some(config.color(LapceColor::LAPCE_ICON_ACTIVE)),
                    name: display_name,
                    path: None,
                    confirmed: None,
                    is_pristine: true,
                }
            }),
        }
    }
}
#[derive(Clone, Debug,)]
pub struct EditorTabChildSimple {
    index: RwSignal<usize>,
    position: RwSignal<Rect>,
    id: EditorTabChildId
}

impl EditorTabChildSimple {
    pub fn new(index: RwSignal<usize>,
               position: RwSignal<Rect>,
               id: EditorTabChildId) -> Self {
        Self {
            index, position, id
        }
    }
    pub fn update_index_with_judgment(&self, index: usize, ) {
        if self.index.get_untracked() != index {
            self.index.set(index);
        }
    }

    pub fn layout_untracted(&self, ) -> Rect {
        self.position.get_untracked()
    }

    pub fn layout_tracing(&self, ) -> Rect {
        self.position.get()
    }

    pub fn write_layout(&self) -> WriteSignal<Rect> {
        self.position.write_only()
    }

    pub fn read_index(&self, ) -> ReadSignal<usize> {
        self.index.read_only()
    }
    pub fn id(&self) -> &EditorTabChildId {
        &self.id
    }
}

#[derive(Clone)]
pub struct EditorTabDraging {
    editor_tab_child_index: ReadSignal<usize>,
    editor_tab_manage_id: EditorTabManageId,
}

impl EditorTabDraging {
    pub fn new(editor_tab_child_index: ReadSignal<usize>,
               editor_tab_manage_id: EditorTabManageId) -> Self {
        Self {
            editor_tab_child_index, editor_tab_manage_id
        }
    }
    pub fn data(&self) -> (usize, EditorTabManageId) {
        (self.editor_tab_child_index.get_untracked(), self.editor_tab_manage_id)
    }
}

#[derive(Clone)]
pub struct EditorTabManageData {
    pub scope: Scope,
    pub split: SplitId,
    pub editor_tab_manage_id: EditorTabManageId,
    pub active: usize,
    pub children: Vec<EditorTabChildSimple>,
    pub window_origin: Point,
    pub layout_rect: Rect,
    pub locations: RwSignal<im::Vector<EditorLocation>>,
    pub current_location: RwSignal<usize>,
}

impl EditorTabManageData {
    pub fn get_editor(
        &self,
        editors: Editors,
        path: &Path,
    ) -> Option<(usize, EditorData)> {
        for (i, child) in self.children.iter().enumerate() {
            if let  EditorTabChildId::Editor(editor_id) = child.id() {
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
        &self,
        editors: Editors,
        diff_editors: &im::HashMap<EditorId, DiffEditorData>,
    ) -> Option<(usize, EditorTabChildId)> {
        for (i, child) in self.children.iter().enumerate() {
            match child.id() {
                EditorTabChildId::Editor(editor_id) => {
                    if let Some(editor) = editors.editor_untracked(*editor_id) {
                        let confirmed = editor.confirmed.get_untracked();
                        if !confirmed {
                            return Some((i, child.id().clone()));
                        }
                    }
                },
                EditorTabChildId::DiffEditor(diff_editor_id) => {
                    if let Some(diff_editor) = diff_editors.get(diff_editor_id) {
                        let confirmed = diff_editor.confirmed.get_untracked();
                        if !confirmed {
                            return Some((i, child.id().clone()));
                        }
                    }
                },
                _ => (),
            }
        }
        None
    }

    pub fn tab_info(&self, data: &WindowWorkspaceData) -> EditorTabInfo {
        let info = EditorTabInfo {
            active: self.active,
            is_focus: data.main_split.active_editor_tab.get_untracked()
                == Some(self.editor_tab_manage_id),
            children: self
                .children
                .iter()
                .map(|child| child.id().child_info(data))
                .collect(),
        };
        info
    }
}

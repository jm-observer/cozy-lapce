use serde::{Deserialize, Serialize};
use lapce_rpc::plugin::VoltID;
use crate::doc::DocContent;

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorTabInfo {
    pub active: usize,
    pub is_focus: bool,
    pub children: Vec<EditorTabChildInfo>,
}

impl EditorTabInfo {

}

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
}



#[derive(Clone, Serialize, Deserialize)]
pub struct EditorInfo {
    pub content: DocContent,
    pub unsaved: Option<String>,
    pub offset: usize,
    pub scroll_offset: (f64, f64),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DiffEditorInfo {
    pub left_content: DocContent,
    pub right_content: DocContent,
}
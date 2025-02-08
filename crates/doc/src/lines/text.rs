use floem::{prelude::RwSignal, reactive::Scope};
use serde::{Deserialize, Serialize};

use crate::lines::register::Clipboard;

#[derive(Clone)]
pub struct Preedit {
    pub text:   String,
    pub cursor: Option<(usize, usize)>,
    pub offset: usize
}

/// IME Preedit
/// This is used for IME input, and must be owned by the `Document`.
#[derive(Debug, Clone)]
pub struct PreeditData {
    pub preedit: RwSignal<Option<Preedit>>
}
impl PreeditData {
    pub fn new(cx: Scope) -> PreeditData {
        PreeditData {
            preedit: cx.create_rw_signal(None)
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum WrapMethod {
    None,
    #[default]
    EditorWidth,
    WrapColumn {
        col: usize
    },
    WrapWidth {
        width: f32
    }
}
impl std::fmt::Display for WrapMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapMethod::None => f.write_str("None"),
            WrapMethod::EditorWidth => f.write_str("Editor Width"),
            WrapMethod::WrapColumn { col } => {
                f.write_fmt(format_args!("Wrap at Column {col}"))
            },
            WrapMethod::WrapWidth { width } => {
                f.write_fmt(format_args!("Wrap Width {width}"))
            },
        }
    }
}
impl WrapMethod {
    pub fn is_none(&self) -> bool {
        matches!(self, WrapMethod::None)
    }

    pub fn is_constant(&self) -> bool {
        matches!(
            self,
            WrapMethod::None
                | WrapMethod::WrapColumn { .. }
                | WrapMethod::WrapWidth { .. }
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderWhitespace {
    #[default]
    None,
    All,
    Boundary,
    Trailing
}
impl std::fmt::Display for RenderWhitespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{self:?}"))
    }
}

// TODO(minor): Should we get rid of this now that this is in floem?
pub struct SystemClipboard;

impl Default for SystemClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemClipboard {
    pub fn new() -> Self {
        Self
    }

    #[cfg(windows)]
    pub fn get_file_list() -> Option<Vec<std::path::PathBuf>> {
        todo!()
        // floem::Clipboard::get_file_list().ok()
    }
}

impl Clipboard for SystemClipboard {
    fn get_string(&mut self) -> Option<String> {
        floem::Clipboard::get_contents().ok()
    }

    fn put_string(&mut self, s: impl AsRef<str>) {
        let _ = floem::Clipboard::set_contents(s.as_ref().to_string());
    }
}

use im::{Vector, vector};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::icon::LapceIcons;

pub type PanelOrder = im::HashMap<PanelContainerPosition, Vector<PanelKind>>;

pub fn default_panel_order() -> PanelOrder {
    let mut order = PanelOrder::new();
    order.insert(
        PanelContainerPosition::Left,
        vector![
            PanelKind::FileExplorer,
            PanelKind::Plugin,
            PanelKind::SourceControl,
            PanelKind::Debug,
        ],
    );
    order.insert(
        PanelContainerPosition::Bottom,
        vector![
            PanelKind::Terminal,
            PanelKind::Search,
            PanelKind::Problem,
            PanelKind::CallHierarchy,
            PanelKind::References,
            PanelKind::Implementation
        ],
    );
    order.insert(
        PanelContainerPosition::Right,
        vector![PanelKind::DocumentSymbol,],
    );

    order
}
#[derive(Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum PanelSection {
    OpenEditor,
    FileExplorer,
    Error,
    Warn,
    Changes,
    Installed,
    Available,
    Process,
    Variable,
    StackFrame,
    Breakpoint,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PanelSize {
    pub left:   f64,
    pub bottom: f64,
    pub right:  f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PanelInfo {
    pub panels:   PanelOrder,
    pub styles:   im::HashMap<PanelContainerPosition, PanelStyle>,
    pub size:     PanelSize,
    pub sections: im::HashMap<PanelSection, bool>,
}

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PanelContainerPosition {
    Left,
    Bottom,
    Right,
}

impl PanelContainerPosition {
    pub fn is_bottom(&self) -> bool {
        matches!(self, PanelContainerPosition::Bottom)
    }

    pub fn is_right(&self) -> bool {
        matches!(self, PanelContainerPosition::Right)
    }

    pub fn is_left(&self) -> bool {
        matches!(self, PanelContainerPosition::Left)
    }

    // pub fn first(&self) -> PanelPosition {
    //     match self {
    //         PanelContainerPosition::Left => PanelPosition::LeftTop,
    //         PanelContainerPosition::Bottom => PanelPosition::BottomLeft,
    //         PanelContainerPosition::Right => PanelPosition::RightTop,
    //     }
    // }
    //
    // pub fn second(&self) -> PanelPosition {
    //     match self {
    //         PanelContainerPosition::Left => PanelPosition::LeftBottom,
    //         PanelContainerPosition::Bottom => PanelPosition::BottomRight,
    //         PanelContainerPosition::Right => PanelPosition::RightBottom,
    //     }
    // }

    pub fn debug_name(&self) -> &'static str {
        match self {
            PanelContainerPosition::Left => "Left Pannel Container View",
            PanelContainerPosition::Bottom => "Bottom Pannel Container View",
            PanelContainerPosition::Right => "Right Pannel Container View",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PanelStyle {
    pub active:    usize,
    pub shown:     bool,
    pub maximized: bool,
}

#[derive(
    Clone, Copy, PartialEq, Serialize, Deserialize, Hash, Eq, Debug, EnumIter,
)]
pub enum PanelKind {
    Terminal,
    FileExplorer,
    SourceControl,
    Plugin,
    Search,
    Problem,
    Debug,
    CallHierarchy,
    DocumentSymbol,
    References,
    Implementation,
    Build,
}

impl PanelKind {
    pub fn svg_name(&self) -> &'static str {
        match &self {
            PanelKind::Terminal => LapceIcons::TERMINAL,
            PanelKind::FileExplorer => LapceIcons::FILE_EXPLORER,
            PanelKind::SourceControl => LapceIcons::SCM,
            PanelKind::Plugin => LapceIcons::EXTENSIONS,
            PanelKind::Search => LapceIcons::SEARCH,
            PanelKind::Problem => LapceIcons::PROBLEM,
            PanelKind::Debug => LapceIcons::DEBUG,
            PanelKind::CallHierarchy => LapceIcons::TYPE_HIERARCHY,
            PanelKind::DocumentSymbol => LapceIcons::DOCUMENT_SYMBOL,
            PanelKind::References => LapceIcons::REFERENCES,
            PanelKind::Implementation => LapceIcons::IMPLEMENTATION,
            PanelKind::Build => LapceIcons::DEBUG,
        }
    }

    pub fn position(
        &self,
        order: &PanelOrder,
    ) -> Option<(usize, PanelContainerPosition)> {
        for (pos, panels) in order.iter() {
            let index = panels.iter().position(|k| k == self);
            if let Some(index) = index {
                return Some((index, *pos));
            }
        }
        None
    }

    pub fn default_position(&self) -> PanelContainerPosition {
        match self {
            PanelKind::Terminal => PanelContainerPosition::Bottom,
            PanelKind::FileExplorer => PanelContainerPosition::Left,
            PanelKind::SourceControl => PanelContainerPosition::Left,
            PanelKind::Plugin => PanelContainerPosition::Left,
            PanelKind::Search => PanelContainerPosition::Bottom,
            PanelKind::Problem => PanelContainerPosition::Bottom,
            PanelKind::Debug => PanelContainerPosition::Left,
            PanelKind::CallHierarchy => PanelContainerPosition::Bottom,
            PanelKind::DocumentSymbol => PanelContainerPosition::Right,
            PanelKind::References => PanelContainerPosition::Bottom,
            PanelKind::Implementation => PanelContainerPosition::Bottom,
            PanelKind::Build => PanelContainerPosition::Bottom,
        }
    }

    pub fn tooltip(&self) -> &'static str {
        match self {
            PanelKind::Terminal => "Terminal",
            PanelKind::FileExplorer => "File Explorer",
            PanelKind::SourceControl => "Source Control",
            PanelKind::Plugin => "Plugins",
            PanelKind::Search => "Search",
            PanelKind::Problem => "Problems",
            PanelKind::Debug => "Debug",
            PanelKind::CallHierarchy => "Call Hierarchy",
            PanelKind::DocumentSymbol => "Document Symbol",
            PanelKind::References => "References",
            PanelKind::Implementation => "Implementation",
            PanelKind::Build => "Build",
        }
    }
}

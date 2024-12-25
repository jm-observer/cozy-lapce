use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use super::data::PanelOrder;
use crate::config::icon::LapceIcons;
use crate::panel::position::PanelContainerPosition;

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
        }
    }
}

use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

pub type LineStyles = HashMap<usize, std::sync::Arc<Vec<LineStyle>>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LineStyle {
    pub start: usize,
    pub end:   usize,
    pub text:  Option<String>,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    pub fg_color: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SemanticStyles {
    pub rev:    u64,
    pub path:   PathBuf,
    pub len:    usize,
    pub styles: Vec<LineStyle>,
}

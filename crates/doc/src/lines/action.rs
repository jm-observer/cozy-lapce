use lsp_types::Position;
use serde::{Deserialize, Serialize};

use crate::lines::fold::{FoldingDisplayItem, FoldingRange};

#[derive(Debug, Serialize, Deserialize)]
pub enum UpdateFolding {
    UpdateByItem(FoldingDisplayItem),
    UpdateByPhantom(Position),
    New(Vec<FoldingRange>),
    FoldCode(usize)
}

impl From<FoldingDisplayItem> for UpdateFolding {
    fn from(value: FoldingDisplayItem) -> Self {
        Self::UpdateByItem(value)
    }
}

impl From<Position> for UpdateFolding {
    fn from(value: Position) -> Self {
        Self::UpdateByPhantom(value)
    }
}

impl From<Vec<FoldingRange>> for UpdateFolding {
    fn from(value: Vec<FoldingRange>) -> Self {
        Self::New(value)
    }
}

impl From<usize> for UpdateFolding {
    fn from(offset: usize) -> Self {
        Self::FoldCode(offset)
    }
}

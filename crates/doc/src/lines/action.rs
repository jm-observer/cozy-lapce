use serde::{Deserialize, Serialize};

use crate::lines::fold::FoldingDisplayItem;

#[derive(Debug, Serialize, Deserialize)]
pub enum UpdateFolding {
    UpdateByItem(FoldingDisplayItem),
    UpdateByPhantom(usize),
    New(Vec<lsp_types::FoldingRange>),
    FoldCode(usize),
    UnFoldCodeByGoTo(usize),
}

impl From<FoldingDisplayItem> for UpdateFolding {
    fn from(value: FoldingDisplayItem) -> Self {
        Self::UpdateByItem(value)
    }
}

// impl From<Position> for UpdateFolding {
//     fn from(value: Position) -> Self {
//         Self::UpdateByPhantom(value)
//     }
// }

impl From<Vec<lsp_types::FoldingRange>> for UpdateFolding {
    fn from(value: Vec<lsp_types::FoldingRange>) -> Self {
        Self::New(value)
    }
}

impl From<usize> for UpdateFolding {
    fn from(offset: usize) -> Self {
        Self::FoldCode(offset)
    }
}

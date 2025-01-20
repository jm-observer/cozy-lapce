// use serde::{Deserialize, Serialize};

// #[derive(Eq, PartialEq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
// pub enum PanelPosition {
//     LeftTop,
//     // LeftBottom,
//     BottomLeft,
//     // BottomRight,
//     RightTop,
//     // RightBottom,
// }
//
// impl PanelPosition {
//     pub fn is_bottom(&self) -> bool {
//         matches!(self, PanelPosition::BottomLeft |
// PanelPosition::BottomRight)     }
//
//     pub fn is_right(&self) -> bool {
//         matches!(self, PanelPosition::RightTop | PanelPosition::RightBottom)
//     }
//
//     pub fn is_left(&self) -> bool {
//         matches!(self, PanelPosition::LeftTop | PanelPosition::LeftBottom)
//     }
//
//     pub fn is_first(&self) -> bool {
//         matches!(
//             self,
//             PanelPosition::LeftTop
//                 | PanelPosition::BottomLeft
//                 | PanelPosition::RightTop
//         )
//     }
//
//     pub fn peer(&self) -> PanelPosition {
//         match &self {
//             PanelPosition::LeftTop => PanelPosition::LeftBottom,
//             PanelPosition::LeftBottom => PanelPosition::LeftTop,
//             PanelPosition::BottomLeft => PanelPosition::BottomRight,
//             PanelPosition::BottomRight => PanelPosition::BottomLeft,
//             PanelPosition::RightTop => PanelPosition::RightBottom,
//             PanelPosition::RightBottom => PanelPosition::RightTop,
//         }
//     }
// }




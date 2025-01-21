use serde::{Deserialize, Serialize};

use crate::editor_tab::EditorTabInfo;
use crate::id::{EditorTabManageId, SplitId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug)]
pub enum SplitMoveDirection {
    Up,
    Down,
    Right,
    Left,
}

impl SplitMoveDirection {
    pub fn direction(&self) -> SplitDirection {
        match self {
            SplitMoveDirection::Up | SplitMoveDirection::Down => {
                SplitDirection::Horizontal
            }
            SplitMoveDirection::Left | SplitMoveDirection::Right => {
                SplitDirection::Vertical
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitContent {
    EditorTab(EditorTabManageId),
    Split(SplitId),
}

impl SplitContent {
    pub fn id(&self) -> u64 {
        match self {
            SplitContent::EditorTab(id) => id.to_raw(),
            SplitContent::Split(id) => id.to_raw(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SplitInfo {
    pub children: Vec<SplitContentInfo>,
    pub direction: SplitDirection,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SplitContentInfo {
    EditorTab(EditorTabInfo),
    Split(SplitInfo),
}

// fn workspace_edits(edit: &WorkspaceEdit) -> Option<HashMap<Url, Vec<TextEdit>>> {
//     if let Some(changes) = edit.changes.as_ref() {
//         return Some(changes.clone());
//     }
//
//     let changes = edit.document_changes.as_ref()?;
//     let edits = match changes {
//         DocumentChanges::Edits(edits) => edits
//             .iter()
//             .map(|e| {
//                 (
//                     e.text_document.uri.clone(),
//                     e.edits
//                         .iter()
//                         .map(|e| match e {
//                             OneOf::Left(e) => e.clone(),
//                             OneOf::Right(e) => e.text_edit.clone(),
//                         })
//                         .collect(),
//                 )
//             })
//             .collect::<HashMap<Url, Vec<TextEdit>>>(),
//         DocumentChanges::Operations(ops) => ops
//             .iter()
//             .filter_map(|o| match o {
//                 DocumentChangeOperation::Op(_op) => None,
//                 DocumentChangeOperation::Edit(e) => Some((
//                     e.text_document.uri.clone(),
//                     e.edits
//                         .iter()
//                         .map(|e| match e {
//                             OneOf::Left(e) => e.clone(),
//                             OneOf::Right(e) => e.text_edit.clone(),
//                         })
//                         .collect(),
//                 )),
//             })
//             .collect::<HashMap<Url, Vec<TextEdit>>>(),
//     };
//     Some(edits)
// }

// fn next_in_file_errors_offset(
//     active_path: Option<(PathBuf, usize, Position)>,
//     file_diagnostics: &[(PathBuf, Vec<EditorDiagnostic>)],
// ) -> (PathBuf, EditorPosition) {
//     if let Some((active_path, offset, position)) = active_path {
//         for (current_path, diagnostics) in file_diagnostics {
//             if &active_path == current_path {
//                 for diagnostic in diagnostics {
//                     if let Some((start, _)) = diagnostic.range {
//                         if start > offset {
//                             return (
//                                 (*current_path).clone(),
//                                 EditorPosition::Offset(start),
//                             );
//                         }
//                     }
//
//                     if diagnostic.diagnostic.range.start.line > position.line
//                         || (diagnostic.diagnostic.range.start.line == position.line
//                             && diagnostic.diagnostic.range.start.character
//                                 > position.character)
//                     {
//                         return (
//                             (*current_path).clone(),
//                             EditorPosition::Position(
//                                 diagnostic.diagnostic.range.start,
//                             ),
//                         );
//                     }
//                 }
//             }
//             if current_path > &active_path {
//                 if let Some((start, _)) = diagnostics[0].range {
//                     return ((*current_path).clone(), EditorPosition::Offset(start));
//                 }
//                 return (
//                     (*current_path).clone(),
//                     if let Some((start, _)) = diagnostics[0].range {
//                         EditorPosition::Offset(start)
//                     } else {
//                         EditorPosition::Position(
//                             diagnostics[0].diagnostic.range.start,
//                         )
//                     },
//                 );
//             }
//         }
//     }
//
//     (
//         file_diagnostics[0].0.clone(),
//         if let Some((start, _)) = file_diagnostics[0].1[0].range {
//             EditorPosition::Offset(start)
//         } else {
//             EditorPosition::Position(file_diagnostics[0].1[0].diagnostic.range.start)
//         },
//     )
// }

#[derive(Clone, Copy, Debug)]
pub enum TabCloseKind {
    CloseOther,
    CloseToLeft,
    CloseToRight,
}

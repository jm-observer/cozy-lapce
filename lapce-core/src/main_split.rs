use std::collections::HashMap;

use lsp_types::{
    DocumentChangeOperation,
    DocumentChanges, OneOf, TextEdit, Url, WorkspaceEdit,
};
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

    // pub fn content_info(&self, data: &WindowWorkspaceData) -> SplitContentInfo {
    //     match &self {
    //         SplitContent::EditorTab(editor_tab_id) => {
    //             let editor_tab_data = data
    //                 .main_split
    //                 .editor_tabs
    //                 .get_untracked()
    //                 .get(editor_tab_id)
    //                 .cloned()
    //                 .unwrap();
    //             SplitContentInfo::EditorTab(
    //                 editor_tab_data.get_untracked().tab_info(data),
    //             )
    //         },
    //         SplitContent::Split(split_id) => {
    //             let split_data = data
    //                 .main_split
    //                 .splits
    //                 .get_untracked()
    //                 .get(split_id)
    //                 .cloned()
    //                 .unwrap();
    //             SplitContentInfo::Split(split_data.get_untracked().split_info(data))
    //         },
    //     }
    // }
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

impl SplitContentInfo {
    // pub fn to_data(
    //     &self,
    //     data: MainSplitData,
    //     parent_split: SplitId,
    // ) -> SplitContent {
    //     match &self {
    //         SplitContentInfo::EditorTab(tab_info) => {
    //             let tab_data = tab_info.to_data(data, parent_split);
    //             SplitContent::EditorTab(
    //                 tab_data.with_untracked(|tab_data| tab_data.editor_tab_manage_id),
    //             )
    //         },
    //         SplitContentInfo::Split(split_info) => {
    //             let split_id = SplitId::next();
    //             split_info.to_data(data, Some(parent_split), split_id);
    //             SplitContent::Split(split_id)
    //         },
    //     }
    // }
}

fn workspace_edits(edit: &WorkspaceEdit) -> Option<HashMap<Url, Vec<TextEdit>>> {
    if let Some(changes) = edit.changes.as_ref() {
        return Some(changes.clone());
    }

    let changes = edit.document_changes.as_ref()?;
    let edits = match changes {
        DocumentChanges::Edits(edits) => edits
            .iter()
            .map(|e| {
                (
                    e.text_document.uri.clone(),
                    e.edits
                        .iter()
                        .map(|e| match e {
                            OneOf::Left(e) => e.clone(),
                            OneOf::Right(e) => e.text_edit.clone(),
                        })
                        .collect(),
                )
            })
            .collect::<HashMap<Url, Vec<TextEdit>>>(),
        DocumentChanges::Operations(ops) => ops
            .iter()
            .filter_map(|o| match o {
                DocumentChangeOperation::Op(_op) => None,
                DocumentChangeOperation::Edit(e) => Some((
                    e.text_document.uri.clone(),
                    e.edits
                        .iter()
                        .map(|e| match e {
                            OneOf::Left(e) => e.clone(),
                            OneOf::Right(e) => e.text_edit.clone(),
                        })
                        .collect(),
                )),
            })
            .collect::<HashMap<Url, Vec<TextEdit>>>(),
    };
    Some(edits)
}

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

pub mod view;

use std::hash::Hash;

use doc::lines::{buffer::rope_text::RopeText, screen_lines::VisualLineInfo};
use floem::prelude::{RwSignal, SignalGet, SignalWith};

use crate::{editor::EditorData, window_workspace::WindowWorkspaceData};

pub fn gutter_data(
    window_tab_data: WindowWorkspaceData,
    e_data: RwSignal<EditorData>
) -> Vec<GutterData> {
    let breakpoints = window_tab_data.terminal.debug.breakpoints;
    let e_data = e_data.get();
    let doc = e_data.doc_signal().get();
    let content = doc.content.get();
    let breakpoints = if let Some(path) = content.path() {
        breakpoints
            .with(|b| b.get(path).cloned())
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let code_lens = doc.code_lens.get();
    let offset = e_data.editor.cursor.get().offset();
    let (current_line, screen_lines) = doc.lines.with_untracked(|x| {
        (x.buffer().line_of_offset(offset), x.signal_screen_lines())
    });
    let screen_lines = screen_lines.get();

    screen_lines
        .visual_lines
        .into_iter()
        .map(|vl_info| {
            if vl_info.visual_line.origin_folded_line_sub_index == 0 {
                let is_current_line =
                    vl_info.visual_line.origin_line == current_line;
                if code_lens.contains_key(&vl_info.visual_line.origin_line) {
                    GutterData {
                        vl_info,
                        marker: GutterMarker::CodeLen,
                        is_current_line
                    }
                } else if breakpoints.contains_key(&vl_info.visual_line.origin_line)
                {
                    GutterData {
                        vl_info,
                        marker: GutterMarker::Breakpoint,
                        is_current_line
                    }
                } else {
                    GutterData {
                        vl_info,
                        marker: GutterMarker::None,
                        is_current_line
                    }
                }
            } else {
                GutterData {
                    vl_info,
                    marker: GutterMarker::None,
                    is_current_line: false
                }
            }
        })
        .collect()
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct GutterData {
    vl_info:         VisualLineInfo,
    marker:          GutterMarker,
    is_current_line: bool
}

impl GutterData {
    pub fn display_line_num(&self) -> String {
        if self.vl_info.visual_line.origin_folded_line_sub_index == 0 {
            (self.vl_info.visual_line.origin_line + 1).to_string()
        } else {
            "".to_string()
        }
    }
}

// impl PartialEq for GutterData {
//     fn eq(&self, other: &Self) -> bool {
//         self.position_y.to_bits() == other.position_y.to_bits()
//             && self.display_line_num == other.display_line_num
//             && self.marker == other.marker
//     }
// }
//
// impl Eq for GutterData {}

// impl Hash for GutterData {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         self.display_line_num.hash(state);
//         self.marker.hash(state);
//         self.position_y.to_bits().hash(state);
//     }
// }
#[derive(Debug, Clone, Hash, Copy, Eq, PartialEq)]
pub enum GutterMarker {
    None,
    CodeLen,
    Breakpoint // CodeLenAndBreakPoint,
}

#[derive(Debug, Clone, Hash, Copy, Eq, PartialEq)]
pub enum GutterFolding {
    None,
    Start,
    End,
    Folded
}

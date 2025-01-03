pub mod view;

use std::hash::{Hash};
use std::rc::Rc;
use doc::lines::screen_lines::VisualLineInfo;
use floem::prelude::{RwSignal, SignalGet, SignalWith};
use crate::editor::EditorData;
use crate::window_tab::WindowTabData;

pub fn gutter_data(window_tab_data: Rc<WindowTabData>,
                   e_data: RwSignal<EditorData>, ) -> Vec<GutterData> {
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
    let screen_lines = doc.lines.with_untracked(|x| x.signal_screen_lines()).get();

    screen_lines.visual_lines.into_iter().map(|vl_info| {
        if vl_info.visual_line.origin_folded_line_sub_index == 0 {
            if code_lens.get(&vl_info.visual_line.origin_line).is_some() {
                GutterData {
                    vl_info,
                    marker: GutterMarker::CodeLen,
                }
            } else if breakpoints.get(&vl_info.visual_line.origin_line).is_some() {
                GutterData {
                    vl_info,
                    marker: GutterMarker::Breakpoint,
                }
            } else {
                GutterData {
                    vl_info,
                    marker: GutterMarker::None,
                }
            }
        } else {
            GutterData {
                vl_info,
                marker: GutterMarker::None,
            }
        }
    }).collect()
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct GutterData {
    vl_info: VisualLineInfo,
    marker: GutterMarker,
}

impl GutterData {
    pub fn display_line_num(&self) -> String {
        if self.vl_info.visual_line.origin_folded_line_sub_index == 0 {
            self.vl_info.visual_line.origin_line.to_string()
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
    Breakpoint,
    // CodeLenAndBreakPoint,
}

#[derive(Debug, Clone, Hash, Copy, Eq, PartialEq)]
pub enum GutterFolding {
    None,
    Start,
    End,
    Folded,
}

pub mod view;

use std::hash::{Hash, Hasher};
use floem::kurbo::Point;
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
    let path = content.path().cloned();
    screen_lines.with(|screen_lines| {
        screen_lines
            .visual_lines
            .iter()
            .map(|vl_info| {
                let is_current_line =
                    vl_info.visual_line.origin_line_start == current_line;
                if code_lens.contains_key(&vl_info.visual_line.origin_line_start) {
                    GutterData {
                        origin_line_start: vl_info.visual_line.origin_line_end,
                        paint_point_y: vl_info.folded_line_y,
                        marker: GutterMarker::CodeLen,
                        is_current_line
                    }
                } else if breakpoints.contains_key(&vl_info.visual_line.origin_line_start)
                {
                    GutterData {
                        origin_line_start: vl_info.visual_line.origin_line_end,
                        paint_point_y: vl_info.folded_line_y,
                        marker: GutterMarker::Breakpoint,
                        is_current_line
                    }
                } else {
                    GutterData {
                        origin_line_start: vl_info.visual_line.origin_line_end,
                        paint_point_y: vl_info.folded_line_y,
                        marker: GutterMarker::None,
                        is_current_line,
                    }
                }
            })
            .collect()
    })
}

#[derive(Clone)]
pub struct GutterData {
    origin_line_start: usize,
    paint_point_y: f64,
    marker:          GutterMarker,
    is_current_line: bool
}

impl GutterData {
    pub fn display_line_num(&self) -> String {
            (self.origin_line_start + 1).to_string()
    }
}

impl PartialEq for GutterData {
    fn eq(&self, other: &Self) -> bool {
        self.paint_point_y.to_bits() == other.paint_point_y.to_bits()
            && self.origin_line_start == other.origin_line_start
            && self.marker == other.marker
    }
}

impl Eq for GutterData {}

impl Hash for GutterData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.origin_line_start.hash(state);
        self.marker.hash(state);
        self.paint_point_y.to_bits().hash(state);
    }
}
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

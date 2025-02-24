pub mod view;

use std::hash::{Hash, Hasher};
use floem::peniko::Color;
use doc::lines::{buffer::rope_text::RopeText, };
use floem::prelude::{RwSignal, SignalGet, SignalWith};

use crate::{editor::EditorData, window_workspace::WindowWorkspaceData};
use crate::config::color::LapceColor;

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
    let (current_line, width) = doc.lines.with_untracked(|x| {
        (x.buffer().line_of_offset(offset), x.last_line_width())
    });
    let screen_lines = e_data.editor.screen_lines.read_only();

    let (fg, dim, style_font_size, font_family) = window_tab_data.common.config.with(|config| {
        (
            config.color(LapceColor::EDITOR_FOREGROUND),
            config.color(LapceColor::EDITOR_DIM),
            config.editor.font_size(),
            config.editor.font_family.clone()
        )
    });

    screen_lines.with(|screen_lines| {
        screen_lines
            .visual_lines
            .iter()
            .map(|vl_info| {
                let style_color = if vl_info.visual_line.origin_line_start == current_line { fg } else { dim };
                if code_lens.contains_key(&vl_info.visual_line.origin_line_start) {
                    GutterData {
                        origin_line_start: vl_info.visual_line.origin_line_end,
                        paint_point_y: vl_info.folded_line_y,
                        marker: GutterMarker::CodeLen,
                        style_color,
                        style_width: width,style_font_size, style_font_family: font_family.clone()
                    }
                } else if breakpoints.contains_key(&vl_info.visual_line.origin_line_start)
                {
                    GutterData {
                        origin_line_start: vl_info.visual_line.origin_line_end,
                        paint_point_y: vl_info.folded_line_y,
                        marker: GutterMarker::Breakpoint,
                        style_color,
                        style_width: width,style_font_size, style_font_family: font_family.clone()
                    }
                } else {
                    GutterData {
                        origin_line_start: vl_info.visual_line.origin_line_end,
                        paint_point_y: vl_info.folded_line_y,
                        marker: GutterMarker::None,
                        style_color,
                        style_width: width,style_font_size, style_font_family: font_family.clone()
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
    style_width: f64,
    style_color: Color,
    style_font_size: usize,
    style_font_family: String,
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

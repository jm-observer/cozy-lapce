pub mod view;

use std::hash::{Hash, Hasher};

use doc::lines::{buffer::rope_text::RopeText, screen_lines::VisualLineInfo};
use floem::{
    peniko::Color,
    prelude::{SignalGet, SignalWith},
};

use crate::{
    config::color::LapceColor, editor::EditorData,
    window_workspace::WindowWorkspaceData,
};

pub fn gutter_data(
    window_tab_data: WindowWorkspaceData,
    e_data: &EditorData,
) -> Vec<GutterData> {
    let breakpoints = window_tab_data.terminal.common.breakpoints;
    let doc = e_data.doc_signal().get();
    let content = doc.content.get();
    let break_line = window_tab_data.terminal.breakline.get();
    // log::error!("break_line {break_line:?}");
    let (breakpoints, current_debug_line) = if let Some(path) = content.path() {
        let current_debug_line = break_line
            .map(|x| if *path == x.1 { x.0 } else { usize::MAX })
            .unwrap_or(usize::MAX);
        (breakpoints.get_by_path_tracked(path), current_debug_line)
    } else {
        (Default::default(), usize::MAX)
    };
    let code_lens = doc.code_lens.get();
    let offset = e_data.cursor.get().offset();
    let (current_line, signal_last_line) = doc.lines.with_untracked(|x| {
        (x.buffer().line_of_offset(offset), x.signal_last_line())
    });
    let width = signal_last_line.get().1 + 8.0;
    let screen_lines = e_data.screen_lines.read_only();

    let (fg, dim, style_font_size, font_family) =
        window_tab_data.common.config.signal(|config| {
            (
                config.color(LapceColor::EDITOR_FOREGROUND),
                config.color(LapceColor::EDITOR_DIM),
                config.editor.font_size.signal(),
                config.editor.font_family.signal(),
            )
        });
    let (fg, dim, style_font_size, font_family) = (
        fg.get(),
        dim.get(),
        style_font_size.get(),
        font_family.get(),
    );

    screen_lines.with(|screen_lines| {
        screen_lines
            .visual_lines
            .iter()
            .map(|vl_info| {
                match vl_info {
                    VisualLineInfo::OriginText { text } => {
                        let is_current_line =
                            text.folded_line.origin_line_start == current_line;
                        let style_color = if is_current_line { fg } else { dim };
                        if current_debug_line == text.folded_line.origin_line_start {
                            GutterData {
                                origin_line_start: Some(
                                    text.folded_line.origin_line_start,
                                ),
                                paint_point_y: text.folded_line_y,
                                marker: GutterMarker::CurrentDebugLine,
                                style_color,
                                style_width: width,
                                style_font_size,
                                style_font_family: font_family.1.clone(),
                                is_current_line,
                            }
                        } else if code_lens
                            .contains_key(&text.folded_line.origin_line_start)
                        {
                            GutterData {
                                origin_line_start: Some(
                                    text.folded_line.origin_line_start,
                                ),
                                paint_point_y: text.folded_line_y,
                                marker: GutterMarker::CodeLen,
                                style_color,
                                style_width: width,
                                style_font_size,
                                style_font_family: font_family.1.clone(),
                                is_current_line,
                            }
                        } else if let Some(breakpoint) =
                            breakpoints.get(&text.folded_line.origin_line_start)
                        {
                            if breakpoint.verified {
                                GutterData {
                                    origin_line_start: Some(
                                        text.folded_line.origin_line_start,
                                    ),
                                    paint_point_y: text.folded_line_y,
                                    marker: GutterMarker::BreakpointVerified,
                                    style_color,
                                    style_width: width,
                                    style_font_size,
                                    style_font_family: font_family.1.clone(),
                                    is_current_line,
                                }
                            } else if breakpoint.active {
                                GutterData {
                                    origin_line_start: Some(
                                        text.folded_line.origin_line_start,
                                    ),
                                    paint_point_y: text.folded_line_y,
                                    marker: GutterMarker::Breakpoint,
                                    style_color,
                                    style_width: width,
                                    style_font_size,
                                    style_font_family: font_family.1.clone(),
                                    is_current_line,
                                }
                            } else {
                                GutterData {
                                    origin_line_start: Some(
                                        text.folded_line.origin_line_start,
                                    ),
                                    paint_point_y: text.folded_line_y,
                                    marker: GutterMarker::BreakpointInactive,
                                    style_color,
                                    style_width: width,
                                    style_font_size,
                                    style_font_family: font_family.1.clone(),
                                    is_current_line,
                                }
                            }
                        } else {
                            GutterData {
                                origin_line_start: Some(
                                    text.folded_line.origin_line_start,
                                ),
                                paint_point_y: text.folded_line_y,
                                marker: GutterMarker::None,
                                style_color,
                                style_width: width,
                                style_font_size,
                                style_font_family: font_family.1.clone(),
                                is_current_line,
                            }
                        }
                    },
                    VisualLineInfo::DiffDelete { folded_line_y } => {
                        // todo origin_line_start
                        GutterData {
                            origin_line_start: None,
                            paint_point_y: *folded_line_y,
                            marker: GutterMarker::None,
                            style_color: dim,
                            style_width: width,
                            style_font_size,
                            style_font_family: font_family.1.clone(),
                            is_current_line: false,
                        }
                    },
                }
            })
            .collect()
    })
}

#[derive(Clone, Debug)]
pub struct GutterData {
    origin_line_start: Option<usize>,
    paint_point_y:     f64,
    marker:            GutterMarker,
    style_width:       f64,
    is_current_line:   bool,
    style_color:       Color,
    style_font_size:   usize,
    style_font_family: String,
}

impl GutterData {
    pub fn display_line_num(&self) -> String {
        self.origin_line_start
            .map(|x| (x + 1).to_string())
            .unwrap_or_default()
    }
}

impl PartialEq for GutterData {
    fn eq(&self, other: &Self) -> bool {
        self.paint_point_y.to_bits() == other.paint_point_y.to_bits()
            && self.origin_line_start == other.origin_line_start
            && self.marker == other.marker
            && self.is_current_line == other.is_current_line
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
    CurrentDebugLine,
    Breakpoint,
    BreakpointInactive,
    BreakpointVerified, // CodeLenAndBreakPoint,
}

#[derive(Debug, Clone, Hash, Copy, Eq, PartialEq)]
pub enum GutterFolding {
    None,
    Start,
    End,
    Folded,
}

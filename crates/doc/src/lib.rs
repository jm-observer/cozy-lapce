use cosmic_text::LayoutGlyph;
use floem::{kurbo::Point, reactive::RwSignal, text::HitPosition};
use lapce_xi_rope::spans::Spans;
use lsp_types::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::lines::{diff::DiffInfo, layout::TextLayout};

pub mod language;
pub mod lens;
pub mod lines;
pub mod syntax;
// mod meta;
pub mod config;

#[derive(Clone, Debug)]
pub struct DiagnosticData {
    pub expanded:         RwSignal<bool>,
    pub diagnostics:      RwSignal<im::Vector<Diagnostic>>,
    pub diagnostics_span: RwSignal<Spans<Diagnostic>>
}

#[derive(Clone)]
pub enum EditorViewKind {
    Normal,
    Diff(DiffInfo)
}

impl EditorViewKind {
    pub fn is_normal(&self) -> bool {
        matches!(self, EditorViewKind::Normal)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LineStyle {
    pub start:    usize,
    pub end:      usize,
    pub text:     Option<String>,
    pub fg_color: Option<String>
}

/// Hit position but decides whether it should go to the next line
/// based on the `before` bool.
///
/// (Hit position should be equivalent to `before=false`).
/// This is needed when we have an idx at the end of, for example, a
/// wrapped line which could be on the first or second line.
pub fn hit_position_aff(this: &TextLayout, idx: usize, before: bool) -> HitPosition {
    let mut last_line = 0;
    let mut last_end: usize = 0;
    let mut offset = 0;
    let mut last_glyph: Option<(&LayoutGlyph, usize)> = None;
    let mut last_line_width = 0.0;
    let mut last_glyph_width = 0.0;
    let mut last_position = HitPosition {
        line:          0,
        point:         Point::ZERO,
        glyph_ascent:  0.0,
        glyph_descent: 0.0
    };
    for (line, run) in this.layout_runs().enumerate() {
        if run.line_i > last_line {
            last_line = run.line_i;
            offset += last_end;
        }

        // Handles wrapped lines, like:
        // ```rust
        // let config_path = |
        // dirs::config_dir();
        // ```
        // The glyphs won't contain the space at the end of the first
        // part, and the position right after the space is the
        // same column as at `|dirs`, which is what before is letting
        // us distinguish.
        // So essentially, if the next run has a glyph that is at the
        // same idx as the end of the previous run, *and* it
        // is at `idx` itself, then we know to position it on the
        // previous.
        if let Some((last_glyph, last_offset)) = last_glyph {
            if let Some(first_glyph) = run.glyphs.first() {
                let end = last_glyph.end + last_offset;
                if before && idx == first_glyph.start + offset {
                    last_position.point.x = if end == idx {
                        // if last glyph end index == idx == first
                        // glyph start index,
                        // it means the wrap wasn't from a whitespace
                        last_line_width as f64
                    } else {
                        // the wrap was a whitespace so we need to add
                        // the whitespace's width
                        // to the line width
                        (last_line_width + last_glyph.w) as f64
                    };
                    return last_position;
                }
            }
        }

        for glyph in run.glyphs {
            if glyph.start + offset > idx {
                last_position.point.x += last_glyph_width as f64;
                return last_position;
            }
            last_end = glyph.end;
            last_glyph_width = glyph.w;
            last_position = HitPosition {
                line,
                point: Point::new(glyph.x as f64, run.line_y as f64),
                glyph_ascent: run.max_ascent as f64,
                glyph_descent: run.max_descent as f64
            };
            if (glyph.start + offset..glyph.end + offset).contains(&idx) {
                return last_position;
            }
        }

        last_glyph = run.glyphs.last().map(|g| (g, offset));
        last_line_width = run.line_w;
    }

    if idx > 0 {
        last_position.point.x += last_glyph_width as f64;
        return last_position;
    }

    HitPosition {
        line:          0,
        point:         Point::ZERO,
        glyph_ascent:  0.0,
        glyph_descent: 0.0
    }
}

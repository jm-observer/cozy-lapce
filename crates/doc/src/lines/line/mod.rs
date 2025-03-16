pub mod update_lines;

use std::{
    cell::RefMut,
    fmt::{Debug, Formatter}
};

use floem::{
    kurbo::{Point, Rect, Size, Vec2},
    text::{HitPoint, HitPosition}
};
use floem::peniko::Color;
use lapce_xi_rope::Interval;
use lsp_types::DocumentHighlight;
use serde::{Deserialize, Serialize};

use super::layout::{LineExtraStyle, TextLayout, TextLayoutLine};
use crate::{
    hit_position_aff,
    lines::{
        cursor::CursorAffinity,
        delta_compute::Offset,
        phantom_text::{PhantomTextLine, Text},
        style::NewLineStyle
    }
};
//
// #[derive(Clone, Debug)]
// pub struct OriginLine {
//     pub line_index: usize,
//     pub start_offset: usize,
//     pub phantom: PhantomTextLine,
//     pub fg_styles: Vec<(usize, usize, Color)>
// }

#[derive(Clone, Serialize, Deserialize)]
pub struct OriginLine {
    pub line_index:        usize,
    /// [start_offset...end_offset)
    pub start_offset:      usize,
    pub len:               usize,
    pub phantom:           PhantomTextLine,
    pub semantic_styles:   Vec<NewLineStyle>,
    pub diagnostic_styles: Vec<NewLineStyle>
}

impl Debug for OriginLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OriginLine line_index={} start_offset={} phantom_text={:?} ",
            self.line_index, self.start_offset, self.phantom,
        )
    }
}

impl OriginLine {
    pub fn semantic_styles(&self, delta: usize) -> Vec<NewLineStyle> {
        self.semantic_styles
            .iter()
            .map(|x| {
                let mut x = x.clone();
                x.origin_line_offset_start += delta;
                x
            })
            .collect()
    }

    pub fn diagnostic_styles(&self, delta: usize) -> Vec<NewLineStyle> {
        self.diagnostic_styles
            .iter()
            .map(|x| {
                let mut x = x.clone();
                x.origin_line_offset_start += delta;
                x
            })
            .collect()
    }

    pub fn adjust(&self, offset: Offset, line_offset: Offset) -> Self {
        let mut obj = self.clone();
        line_offset.adjust(&mut obj.line_index);
        offset.adjust(&mut obj.start_offset);
        offset.adjust(&mut obj.phantom.offset_of_line);
        line_offset.adjust(&mut obj.phantom.line);
        obj.semantic_styles
            .iter_mut()
            .for_each(|x| x.adjust(offset, line_offset));
        obj.diagnostic_styles
            .iter_mut()
            .for_each(|x| x.adjust(offset, line_offset));
        obj
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OriginFoldedLine {
    pub line_index:        usize,
    /// origin_line_start..=origin_line_end
    pub origin_line_start: usize,
    /// origin_line_start..=origin_line_end
    pub origin_line_end:   usize,
    pub origin_interval:   Interval,
    pub last_line:         bool,
    text_layout:           TextLayoutLine
}

impl OriginFoldedLine {
    pub fn adjust(
        &self,
        offset: Offset,
        line_offset: Offset,
        line_index: usize
    ) -> Self {
        let mut obj = self.clone();
        offset.adjust(&mut obj.origin_interval.start);
        offset.adjust(&mut obj.origin_interval.end);
        line_offset.adjust(&mut obj.origin_line_start);
        line_offset.adjust(&mut obj.origin_line_end);
        obj.line_index = line_index;
        obj.text_layout.adjust(line_offset, offset);

        obj
    }

    // fn final_offset_of_visual_line(
    //     &self,
    //     sub_line_index: usize,
    //     line_offset: usize
    // ) -> usize {
    //     let final_offset =
    //         self.text_layout.text.line_layout().iter().enumerate().fold(
    //             line_offset,
    //             |mut offset, (index, layout)| {
    //                 if sub_line_index < index {
    //                     offset += layout.glyphs.len();
    //                 }
    //                 offset
    //             }
    //         );
    //     let (_orgin_line, _offset_of_line, offset_of_buffer, _) = self
    //         .text_layout
    //         .phantom_text
    //         .cursor_position_of_final_col(final_offset);
    //     offset_of_buffer
    // }

    // /// 求原始的行的偏移，最终出现在第几个视觉行，
    // /// 以及在视觉行的偏移位置，以及合并行的偏移位置
    // pub(crate) fn final_offset_of_line_and_offset(
    //     &self,
    //     origin_line: usize,
    //     offset: usize,
    //     _affinity: CursorAffinity
    // ) -> usize {
    //     self.text_layout.phantom_text.final_col_of_col(
    //         origin_line,
    //         offset,
    //         true
    //     )
    // }

    pub(crate) fn visual_offset_of_cursor_offset(
        &self,
        origin_line: usize,
        offset: usize,
        _affinity: CursorAffinity
    ) -> Option<usize> {
        let final_offset = self
            .text_layout
            .phantom_text
            .visual_offset_of_cursor_offset(origin_line, offset, _affinity)?;
        // let (sub_line, offset_of_visual) =
        //     self.visual_line_of_final_offset(final_offset);
        Some(final_offset)
    }

    // /// 求最终的行偏移出现在第几个视觉行，以及在视觉行的偏移位置
    // fn visual_line_of_final_offset(&self, final_offset: usize) -> usize {
    //     // 空行时，会出现==的情况
    //     if final_offset > self.len() {
    //         panic!("final_offset={final_offset} >= {}", self.len())
    //     }
    //     let folded_line_layout = self.text_layout.text.line_layout();
    //     if folded_line_layout.len() == 1 {
    //         return (0, final_offset);
    //     }
    //     let mut sub_line_index = folded_line_layout.len() - 1;
    //     let mut final_offset_line = final_offset;
    //     // let mut last_char = false;
    //
    //     for (index, sub_line) in folded_line_layout.iter().enumerate() {
    //         if final_offset_line <= sub_line.glyphs.len() {
    //             sub_line_index = index;
    //             // last_char = final_offset == sub_line.glyphs.len() -
    //             // self.text_layout.text.;
    //             break;
    //         } else {
    //             final_offset_line -= sub_line.glyphs.len();
    //         }
    //     }
    //     (sub_line_index, final_offset_line)
    // }

    pub fn is_last_char(&self, final_offset: usize) -> bool {
        // struct A|;
        final_offset >= self.text_layout.text.borrow().text_len_without_rn
    }

    /// 单一视觉行的间隔point
    pub fn line_scope(
        &self,
        start_col: usize,
        end_col: usize,
        line_height: f64,
        y: f64,
        base: Vec2
    ) -> Rect {
        let mut hit0 = self.text_layout.text.borrow_mut().hit_position(start_col);
        let hit1 = self.text_layout.text.borrow_mut().hit_position(end_col);
        let width = hit1.point.x - hit0.point.x;
        hit0.point.y = y + base.y;
        Rect::from_origin_size(hit0.point, Size::new(width, line_height))
    }

    // 行号
    pub fn line_number(
        &self,
        show_relative: bool,
        current_number: Option<usize>
    ) -> Option<usize> {
        let line_number = self.origin_line_start + 1;
        Some(if show_relative {
            if let Some(current_number) = current_number {
                if line_number == current_number {
                    line_number
                } else {
                    line_number.abs_diff(current_number)
                }
            } else {
                line_number
            }
        } else {
            line_number
        })
    }

    pub fn size_width(&self) -> Size {
        self.text_layout.text.borrow_mut().size()
    }

    pub fn hit_position_aff(
        &self,
        col: usize,
        affinity: CursorAffinity
    ) -> HitPosition {
        hit_position_aff(
            &mut self.text_layout.text.borrow_mut(),
            col,
            affinity == CursorAffinity::Backward
        )
    }

    pub fn hit_point(&self, point: Point) -> HitPoint {
        self.text_layout.text.borrow_mut().hit_point(point)
    }

    pub fn text_of_final_col(&self, final_col: usize) -> &Text {
        self.text_layout
            .phantom_text
            .text_of_final_col_even_overflow(final_col)
    }

    pub fn text_of_origin_merge_col(
        &self,
        final_col: usize
    ) -> anyhow::Result<&Text> {
        self.text_layout
            .phantom_text
            .text_of_origin_merge_col(final_col)
    }

    pub fn cursor_position_of_final_col(
        &self,
        final_col: usize
    ) -> (usize, CursorAffinity) {
        log::info!("{:?} {}", self, final_col);
        match self
            .text_layout
            .phantom_text
            .text_of_final_col_even_overflow(final_col)
        {
            Text::Phantom { text } => {
                // 在虚拟文本的后半部分，则光标置于虚拟文本之后
                if final_col > text.final_col + text.text.len() / 2 {
                    (
                        text.origin_merge_col + self.offset_of_line(),
                        CursorAffinity::Forward
                    )
                } else {
                    (
                        text.origin_merge_col + self.offset_of_line(),
                        CursorAffinity::Backward
                    )
                }
            },
            Text::OriginText { text } => {
                let max_origin_merge_col = self.origin_interval.size()
                    - (self.len() - self.len_without_rn());
                let merge_col = (final_col - text.final_col.start
                    + text.origin_merge_col_start())
                .min(max_origin_merge_col);
                (
                    // text.line,
                    // text.origin_col_of_final_col(visual_char_offset),
                    // visual_char_offset,
                    self.offset_of_line() + merge_col,
                    CursorAffinity::Backward
                )
            },
            Text::EmptyLine { text } => {
                (text.offset_of_line, CursorAffinity::Backward)
            },
        }
    }

    pub fn buffer_offset_of_start_line(&self) -> usize {
        self.text_layout.phantom_text.offset_of_line
    }

    pub fn text(&self) -> &[Text] {
        &self.text_layout.phantom_text.text
    }

    pub fn cursor_final_col_of_merge_col(
        &self,
        merge_col: usize,
        cursor_affinity: CursorAffinity
    ) -> anyhow::Result<usize> {
        self.text_layout
            .phantom_text
            .cursor_final_col_of_origin_merge_col(merge_col, cursor_affinity)
    }

    pub fn final_col_of_origin_merge_col(
        &self,
        merge_col: usize
    ) -> anyhow::Result<Option<usize>> {
        self.text_layout
            .phantom_text
            .final_col_of_origin_merge_col(merge_col)
    }

    pub fn offset_of_line(&self) -> usize {
        self.text_layout.phantom_text.offset_of_line
    }

    pub fn len(&self) -> usize {
        self.text_layout.text.borrow().text_len
    }

    /// note:
    /// len_without_rn of final content
    pub fn len_without_rn(&self) -> usize {
        self.text_layout.text.borrow().text_len_without_rn
    }

    pub fn first_no_whitespace(&self) -> Option<usize> {
        self.text_layout
            .text
            .borrow()
            .text()
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(idx, _)| idx)
    }

    pub fn borrow_text(&self) -> RefMut<TextLayout> {
        self.text_layout.text.borrow_mut()
    }

    // pub fn init_layout(&mut self) {
    //     if !self.text_layout.text.init_line {
    //         let mut font_system = FONT_SYSTEM.lock();
    //         self.text_layout.text.shape_until_scroll(&mut font_system, false);
    //         self.text_layout.text.init_line = true;
    //     }
    // }

    pub fn extra_style(&mut self) -> &[LineExtraStyle] {
        &self.text_layout.extra_style()
    }

    pub fn document_highlight_style(&mut self) -> &[LineExtraStyle] {
        &self.text_layout.document_highlight_style()
    }

    pub fn whitespaces(&self) -> &Option<Vec<(char, (f64, f64))>> {
        &self.text_layout.whitespaces
    }

    #[inline]
    pub fn contain_buffer_offset(&self, buffer_offset: usize) -> bool {
        if self.last_line {
            self.origin_interval.start <= buffer_offset
        } else {
            self.origin_interval.contains(buffer_offset)
        }
    }

    pub fn init_layout(&self) {
        self.text_layout.text.borrow_mut().init_line();
    }

    pub fn init_document_highlight(&mut self, highlight: Vec<DocumentHighlight>, fg_color: Color) {
        self.text_layout.init_document_highlight(highlight, fg_color);
    }
}

impl Debug for OriginFoldedLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OriginFoldedLine line_index={} origin_line_start={} \
             origin_line_end={} origin_interval={} {:?} text_len={} \
             text_len_without_rn={} text_layout_line={} text_layout={} \
             phantom_text={:?} ",
            self.line_index,
            self.origin_line_start,
            self.origin_line_end,
            self.origin_interval,
            self.text_layout.text.borrow().text(),
            self.len(),
            self.len_without_rn(),
            self.text_layout.init(),
            self.text_layout.text.borrow().init(),
            self.text_layout.phantom_text,
        )
    }
}

// #[derive(Clone)]
// pub struct VisualLine {
//     pub line_index:                   usize,
//     pub origin_interval:              Interval,
//     /// 合并后的视觉范围
//     pub visual_interval:              Interval,
//     pub origin_line:                  usize,
//     pub origin_folded_line:           usize,
//     pub origin_folded_line_sub_index: usize,
//     pub text_layout:                  TextLayoutLine
// }
//
// impl Debug for VisualLine {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("VisualLine")
//             .field("line_index", &self.line_index)
//             .field("origin_interval", &self.origin_interval)
//             .field("visual_interval", &self.visual_interval)
//             .field("origin_line", &self.origin_line)
//             .field("origin_folded_line", &self.origin_folded_line)
//             .field(
//                 "origin_folded_line_sub_index",
//                 &self.origin_folded_line_sub_index,
//             )
//             // .field("text_layout layout len=",
// &self.text_layout.text.line().layout_opt().map(|x| x.len()))             //
// .field("phantom_text", &self.text_layout.phantom_text)             .finish()
//     }
// }

// impl VisualLine {
//     // pub fn cmp_y(&self, other: &Self) -> Ordering {
//     //     let rs = self.origin_folded_line.cmp(&other.origin_folded_line);
//     //     match rs {
//     //         Ordering::Equal => self
//     //             .origin_folded_line_sub_index
//     //             .cmp(&other.origin_folded_line_sub_index),
//     //         Ordering::Less | Ordering::Greater => rs
//     //     }
//     // }
//
//     // pub fn rvline(&self) -> RVLine {
//     //     RVLine {
//     //         line: self.origin_folded_line,
//     //         line_index: self.origin_folded_line_sub_index,
//     //     }
//     // }
//     //
//     // pub fn vline(&self) -> VLine {
//     //     VLine(self.line_index)
//     // }
//
//     // pub fn vline_info(&self) -> VLineInfo {
//     //     let rvline = self.rvline();
//     //     let vline = self.vline();
//     //     let interval = self.origin_interval;
//     //     // todo?
//     //     let origin_line = self.origin_folded_line;
//     //     VLineInfo {
//     //         interval,
//     //         rvline,
//     //         origin_line,
//     //         vline,
//     //     }
//     // }
//
//     // 行号
//     pub fn line_number(
//         &self,
//         show_relative: bool,
//         current_number: Option<usize>
//     ) -> Option<usize> {
//         if self.origin_folded_line_sub_index == 0 {
//             let line_number = self.origin_line + 1;
//             Some(if show_relative {
//                 if let Some(current_number) = current_number {
//                     if line_number == current_number {
//                         line_number
//                     } else {
//                         line_number.abs_diff(current_number)
//                     }
//                 } else {
//                     line_number
//                 }
//             } else {
//                 line_number
//             })
//         } else {
//             None
//         }
//     }
// }

// impl From<&VisualLine> for RVLine {
//     fn from(value: &VisualLine) -> Self {
//         value.rvline()
//     }
// }
//
// impl From<&VisualLine> for VLine {
//     fn from(value: &VisualLine) -> Self {
//         value.vline()
//     }
// }

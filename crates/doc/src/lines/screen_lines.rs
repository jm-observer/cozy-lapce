use std::{cmp::Ordering, ops::AddAssign, rc::Rc};

use anyhow::{Result, bail};
use floem::kurbo::{Point, Rect};

use crate::lines::{cursor::CursorAffinity, line::OriginFoldedLine};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiffSectionKind {
    NoCode,
    Added,
    Removed
}

#[derive(Clone, PartialEq)]
pub struct DiffSection {
    /// The y index that the diff section is at.
    /// This is multiplied by the line height to get the y position.
    /// So this can roughly be considered as the `VLine of the start of this
    /// diff section, but it isn't necessarily convertible to a `VLine` due
    /// to jumping over empty code sections.
    pub y_idx:  usize,
    pub height: usize,
    pub kind:   DiffSectionKind
}

// TODO(minor): We have diff sections in screen lines because Lapce
// uses them, but we don't really have support for diffs in
// floem-editor! Is there a better design for this? Possibly we should
// just move that out to a separate field on Lapce's editor.
// 不允许滚到到窗口没有文本！！！因此lines等不会为空
#[derive(Clone, Default)]
pub struct ScreenLines {
    pub visual_lines:  Vec<VisualLineInfo>,
    /// Guaranteed to have an entry for each `VLine` in `lines`
    /// You should likely use accessor functions rather than this
    /// directly.
    pub diff_sections: Option<Rc<Vec<DiffSection>>>,
    // The base y position that all the y positions inside `info` are
    // relative to. This exists so that if a text layout is
    // created outside of the view, we don't have to completely
    // recompute the screen lines (or do somewhat intricate things to
    // update them) we simply have to update the `base_y`.
    /// 滚动窗口
    pub base:          Rect,
    pub line_height:   f64,
    pub buffer_len:    usize
}

#[derive(Clone)]
pub struct VisualLineInfo {
    /// 该视觉行所属折叠行（原始行）在窗口的y偏移（不是整个文档的y偏移）。
    /// 若该折叠行（原始行）只有1行视觉行，则y=vline_y。行顶的y值！！！
    pub folded_line_y: f64,
    // 在不支持编辑器折叠下，该行已无意义。
    // 视觉行在窗口的y偏移（不是整个文档的y偏移）。行顶的y值！！！
    // pub visual_line_y: f64,
    pub base:          Rect,
    pub visual_line:   OriginFoldedLine
}

// impl Hash for VisualLineInfo {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         self.folded_line_y.to_bits().hash(state);
//         self.visual_line_y.to_bits().hash(state);
//         self.visual_line.hash(state);
//     }
// }

impl VisualLineInfo {
    pub fn paint_point(&self) -> Point {
        Point::new(self.base.x0, self.folded_line_y + self.base.y0)
    }
}

impl ScreenLines {
    // pub fn new(_cx: Scope, viewport: Rect, line_height: f64) -> ScreenLines {
    //     ScreenLines {
    //         visual_lines: Default::default(),
    //         diff_sections: Default::default(),
    //         base: viewport,
    //         line_height
    //     }
    // }

    pub fn is_empty(&self) -> bool {
        self.visual_lines.is_empty()
    }

    pub fn clear(&mut self, viewport: Rect) {
        self.base = viewport;
    }

    // /// Get the line info for the given rvline.
    // pub fn info(&self, rvline: RVLine) -> Option<LineInfo> {
    //     let info = self.info.get(&rvline)?;
    //     // let base = self.base.get();
    //
    //     Some(info.clone().with_base(self.base))
    // }

    pub fn visual_line_of_y(&self, y: f64) -> &VisualLineInfo {
        let y = y - self.base.y0;
        for vli in &self.visual_lines {
            if vli.folded_line_y <= y && y < vli.folded_line_y + self.line_height {
                return vli;
            }
        }
        self.visual_lines.last().unwrap()
    }

    // pub fn log(&self) {
    //     info!("{:?}", self.visual_lines);
    // }
}

impl ScreenLines {
    pub fn line_interval(&self) -> Result<(usize, usize)> {
        match (self.visual_lines.first(), self.visual_lines.last()) {
            (Some(first), Some(last)) => Ok((
                first.visual_line.origin_line_start,
                last.visual_line.origin_line_start
            )),
            _ => bail!("ScreenLines is empty?")
        }
    }

    pub fn offset_interval(&self) -> Result<(usize, usize)> {
        match (self.visual_lines.first(), self.visual_lines.last()) {
            (Some(first), Some(last)) => Ok((
                first.visual_line.origin_interval.start,
                last.visual_line.origin_interval.end
            )),
            _ => bail!("ScreenLines is empty?")
        }
    }

    /// 获取原始行的视觉行信息。为none则说明被折叠，或者没有在窗口范围
    pub fn visual_line_info_for_origin_line(
        &self,
        origin_line: usize
    ) -> Option<&VisualLineInfo> {
        for visual_line in &self.visual_lines {
            match origin_line.cmp(&visual_line.visual_line.origin_line_start) {
                Ordering::Less => {
                    return None;
                },
                Ordering::Equal => {
                    return Some(visual_line);
                },
                _ => {}
            }
        }
        None
    }

    /// 获取折叠原始行的视觉行信息。为none则说明被折叠，或者没有在窗口范围
    pub fn visual_line_info_for_origin_folded_line(
        &self,
        line_index: usize
    ) -> Option<&VisualLineInfo> {
        for visual_line in &self.visual_lines {
            if line_index == visual_line.visual_line.line_index {
                return Some(visual_line);
            }
        }
        None
    }

    // equal to visual_line_info_for_origin_line?
    // /// 获取原始行的视觉行信息。为none则说明被折叠，或者没有在窗口范围
    // pub fn visual_line_info_of_origin_line(
    //     &self,
    //     origin_line: usize
    // ) -> Option<&VisualLineInfo> {
    //     for visual_line in &self.visual_lines {
    //         if visual_line.visual_line.origin_line == origin_line
    //             && visual_line.visual_line.origin_folded_line_sub_index == 0
    //         {
    //             return Some(visual_line);
    //         } else if (visual_line.visual_line.origin_line == origin_line
    //             && visual_line.visual_line.origin_folded_line_sub_index > 0)
    //             || visual_line.visual_line.origin_line > origin_line
    //         {
    //             break;
    //         }
    //     }
    //     None
    // }

    // /// 获取原始行的视觉行信息。为none则说明被折叠，或者没有在窗口范围
    // pub fn visual_line_info_of_visual_line(
    //     &self,
    //     visual_line: &VisualLine
    // ) -> Option<&VisualLineInfo> {
    //     for visual_line_info in &self.visual_lines {
    //         if visual_line_info.visual_line == *visual_line {
    //             return Some(visual_line_info);
    //         } else if (visual_line_info.visual_line.origin_folded_line
    //             == visual_line.origin_folded_line
    //             && visual_line_info.visual_line.origin_folded_line_sub_index
    //                 > visual_line.origin_folded_line_sub_index)
    //             || visual_line_info.visual_line.origin_folded_line
    //                 > visual_line.origin_folded_line
    //         {
    //             break;
    //         }
    //     }
    //     None
    // }

    /// 求视窗与参数行的交集
    /// 用于选择鼠标选择区域
    pub fn intersection_with_lines(
        &self,
        start_line_index: usize,
        end_line_index: usize
    ) -> Option<(&VisualLineInfo, &VisualLineInfo)> {
        let first_visual_line = self.visual_lines.first()?;
        let last_visual_line = self.visual_lines.last()?;
        let start_line_index = first_visual_line
            .visual_line
            .line_index
            .max(start_line_index);
        let end_line_index =
            last_visual_line.visual_line.line_index.min(end_line_index);
        if start_line_index > end_line_index {
            return None;
        }
        Some((
            self.visual_line_info_for_origin_folded_line(start_line_index)?,
            self.visual_line_info_for_origin_folded_line(end_line_index)?
        ))
    }

    pub fn visual_line_for_buffer_offset(
        &self,
        buffer_offset: usize
    ) -> Option<&VisualLineInfo> {
        for visual_line in &self.visual_lines {
            if visual_line
                .visual_line
                .origin_interval
                .contains(buffer_offset)
            {
                return Some(visual_line);
            } else if visual_line.visual_line.origin_interval.start == buffer_offset
            {
                // last line and line is empty
                // origin_interval == [buffer_offset, buffer_offset)
                return Some(visual_line);
            } else if visual_line.visual_line.origin_interval.start > buffer_offset {
                return None;
            }
        }
        None
    }

    pub fn visual_line_info_of_buffer_offset(
        &self,
        buffer_offset: usize
    ) -> Result<Option<(&VisualLineInfo, usize)>> {
        let Some(vl) = self.visual_line_for_buffer_offset(buffer_offset) else {
            return Ok(None);
        };
        let merge_col = buffer_offset - vl.visual_line.origin_interval.start;
        let Some(final_offset) =
            vl.visual_line.final_col_of_origin_merge_col(merge_col)?
        else {
            return Ok(None);
        };
        Ok(Some((vl, final_offset)))
    }

    pub fn visual_position_of_buffer_offset(
        &self,
        buffer_offset: usize
    ) -> Result<Option<Point>> {
        let Some((vl, final_offset)) =
            self.visual_line_info_of_buffer_offset(buffer_offset)?
        else {
            return Ok(None);
        };
        let mut viewpport_point = vl
            .visual_line
            .hit_position_aff(final_offset, CursorAffinity::Backward)
            .point;

        viewpport_point.y = vl.folded_line_y;
        viewpport_point.add_assign(self.base.origin().to_vec2());

        Ok(Some(viewpport_point))
    }

    /// considering phantom text
    /// None: not in viewport
    pub fn cursor_position_of_buffer_offset(
        &self,
        buffer_offset: usize,
        affinity: CursorAffinity
    ) -> Result<Option<Point>> {
        let Some((vl, final_offset)) =
            self.cursor_info_of_buffer_offset(buffer_offset, affinity)?
        else {
            return Ok(None);
        };
        let mut viewpport_point = vl
            .visual_line
            .hit_position_aff(final_offset, CursorAffinity::Backward)
            .point;
        viewpport_point.y = vl.folded_line_y;
        viewpport_point.add_assign(self.base.origin().to_vec2());

        Ok(Some(viewpport_point))
    }

    pub fn cursor_info_of_buffer_offset(
        &self,
        buffer_offset: usize,
        cursor_affinity: CursorAffinity
    ) -> Result<Option<(&VisualLineInfo, usize)>> {
        let Some(vl) = self.visual_line_for_buffer_offset(buffer_offset) else {
            return Ok(None);
        };
        let merge_col = buffer_offset - vl.visual_line.origin_interval.start;
        let final_offset = vl
            .visual_line
            .cursor_final_col_of_merge_col(merge_col, cursor_affinity)?;
        Ok(Some((vl, final_offset)))
    }

    pub fn char_rect_in_viewport(&self, offset: usize) -> Result<Option<Rect>> {
        let Some((vl_start, col_start)) =
            self.visual_line_info_of_buffer_offset(offset)?
        else {
            return Ok(None);
        };
        let folded_line_start = &vl_start.visual_line;
        let base = self.base.origin().to_vec2();

        Ok(Some(folded_line_start.line_scope(
            col_start,
            col_start + 1,
            self.line_height,
            vl_start.folded_line_y,
            base
        )))
    }

    pub fn normal_selection(
        &self,
        start_offset: usize,
        end_offset: usize,
        start_affinity: Option<CursorAffinity>,
        end_affinity: Option<CursorAffinity>
    ) -> Result<Vec<Rect>> {
        let (start_offset, end_offset) = if start_offset > end_offset {
            (end_offset, start_offset)
        } else {
            (start_offset, end_offset)
        };
        let Some((vl_start, col_start)) = self.cursor_info_of_buffer_offset(
            start_offset,
            start_affinity.unwrap_or_default()
        )?
        else {
            return Ok(vec![]);
        };
        let folded_line_start = &vl_start.visual_line;
        let Some((vl_end, col_end)) = self.cursor_info_of_buffer_offset(
            end_offset,
            end_affinity.unwrap_or_default()
        )?
        else {
            return Ok(vec![]);
        };
        let folded_line_end = &vl_end.visual_line;

        let Some((rs_start, rs_end)) = self.intersection_with_lines(
            folded_line_start.line_index,
            folded_line_end.line_index
        ) else {
            return Ok(vec![]);
        };
        let base = self.base.origin().to_vec2();
        if folded_line_start.line_index == folded_line_end.line_index {
            let rs = folded_line_start.line_scope(
                col_start,
                col_end,
                self.line_height,
                rs_start.folded_line_y,
                base
            );
            Ok(vec![rs])
        } else {
            let mut first = Vec::with_capacity(
                folded_line_end.line_index + 1 - folded_line_start.line_index
            );
            first.push(folded_line_start.line_scope(
                col_start,
                folded_line_start.len_without_rn(),
                self.line_height,
                rs_start.folded_line_y,
                base
            ));

            for vl in &self.visual_lines {
                if vl.visual_line.line_index >= folded_line_end.line_index {
                    break;
                } else if vl.visual_line.line_index <= folded_line_start.line_index {
                    continue;
                } else {
                    let selection = vl.visual_line.line_scope(
                        0,
                        vl.visual_line.len_without_rn(),
                        self.line_height,
                        vl.folded_line_y,
                        base
                    );
                    first.push(selection)
                }
            }
            let last = folded_line_end.line_scope(
                0,
                col_end,
                self.line_height,
                rs_end.folded_line_y,
                base
            );
            first.push(last);
            Ok(first)
        }
    }
}

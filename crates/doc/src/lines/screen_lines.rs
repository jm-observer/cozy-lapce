use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    rc::Rc
};

use anyhow::{Result, bail};
use floem::{
    kurbo::{Point, Rect},
    reactive::Scope
};
use log::{error, info};

use crate::lines::line::VisualLine;

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
#[derive(Clone)]
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
    pub line_height:   f64
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisualLineInfo {
    /// 该视觉行所属折叠行（原始行）在窗口的y偏移（不是整个文档的y偏移）。
    /// 若该折叠行（原始行）只有1行视觉行，则y=vline_y。行顶的y值！！！
    pub folded_line_y: f64,
    /// 视觉行在窗口的y偏移（不是整个文档的y偏移）。行顶的y值！！！
    pub visual_line_y: f64,
    pub base:          Rect,
    pub visual_line:   VisualLine
}

impl Hash for VisualLineInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.folded_line_y.to_bits().hash(state);
        self.visual_line_y.to_bits().hash(state);
        self.visual_line.hash(state);
    }
}
impl Eq for VisualLineInfo {}

impl VisualLineInfo {
    pub fn paint_point(&self) -> Point {
        Point::new(self.base.x0, self.visual_line_y + self.base.y0)
    }
}

impl ScreenLines {
    pub fn new(_cx: Scope, viewport: Rect, line_height: f64) -> ScreenLines {
        ScreenLines {
            visual_lines: Default::default(),
            diff_sections: Default::default(),
            base: viewport,
            line_height
        }
    }

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

    // pub fn vline_info(&self, rvline: RVLine) ->
    // Option<VLineInfo<VLine>> {     self.info.get(&rvline).
    // map(|info| info.vline_info) }

    // pub fn rvline_range(&self) -> Option<(RVLine, RVLine)> {
    //     self.lines.first().copied().zip(self.lines.last().copied())
    // }

    // /// Iterate over the line info, copying them with the full y
    // positions. pub fn iter_line_info(&self) -> impl
    // Iterator<Item = LineInfo> + '_ {     self.lines.iter().
    // map(|rvline| self.info(*rvline).unwrap()) }

    // /// Iterate over the line info within the range, copying them
    // with the full y positions. /// If the values are out of
    // range, it is clamped to the valid lines within.
    // pub fn iter_line_info_r(
    //     &self,
    //     r: RangeInclusive<RVLine>,
    // ) -> impl Iterator<Item = LineInfo> + '_ {
    //     // We search for the start/end indices due to not having a
    // good way to iterate over     // successive rvlines without
    // the view.     // This should be good enough due to lines
    // being small.     let start_idx =
    // self.lines.binary_search(r.start()).ok().or_else(|| {
    //         if self.lines.first().map(|l| r.start() <
    // l).unwrap_or(false) {             Some(0)
    //         } else {
    //             // The start is past the start of our lines
    //             None
    //         }
    //     });
    //
    //     let end_idx =
    // self.lines.binary_search(r.end()).ok().or_else(|| {
    //         if self.lines.last().map(|l| r.end() >
    // l).unwrap_or(false) {             Some(self.lines.len() -
    // 1) } else { // The end is before the end of our lines but not
    // available             None
    //         }
    //     });
    //
    //     if let (Some(start_idx), Some(end_idx)) = (start_idx,
    // end_idx) {         self.lines.get(start_idx..=end_idx)
    //     } else {
    //         // Hacky method to get an empty iterator of the same
    // type         self.lines.get(0..0)
    //     }
    //     .into_iter()
    //     .flatten()
    //     .copied()
    //     .map(|rvline| self.info(rvline).unwrap())
    // }

    // pub fn iter_vline_info(&self) -> impl Iterator<Item =
    // VLineInfo<()>> + '_ {     self.lines
    //         .iter()
    //         .map(|vline| &self.info[vline].vline_info)
    //         .copied()
    // }

    // pub fn iter_vline_info_r(
    //     &self,
    //     r: RangeInclusive<RVLine>,
    // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
    //     // TODO(minor): this should probably skip tracking?
    //     self.iter_line_info_r(r).map(|x| x.vline_info)
    // }

    // /// Iter the real lines underlying the visual lines on the
    // screen pub fn iter_lines(&self) -> impl Iterator<Item =
    // usize> + '_ {     // We can just assume that the lines
    // stored are contiguous and thus just get the first     // buffer
    // line and then the last buffer line.     let start_vline =
    // self.lines.first().copied().unwrap_or_default();
    //     let end_vline =
    // self.lines.last().copied().unwrap_or_default();
    //
    //     let start_line =
    // self.info(start_vline).unwrap().vline_info.rvline.line;
    //     let end_line =
    // self.info(end_vline).unwrap().vline_info.rvline.line;
    //
    //     start_line..=end_line
    // }

    /// 视觉行
    // pub fn iter_visual_lines_y(
    //     &self,
    //     show_relative: bool,
    //     current_line: usize,
    // ) -> impl Iterator<Item = (String, f64)> + '_ {
    //     self.visual_lines.iter().map(move |vline| {
    //         let text = vline.visual_line.line_number(show_relative,
    // current_line);         // let info =
    // self.info(*vline).unwrap();         // let line =
    // info.vline_info.origin_line;         // if last_line ==
    // Some(line) {         //     // We've already considered
    // this line.         //     return None;
    //         // }
    //         // last_line = Some(line);
    //         (text, vline.y)
    //     })
    // }

    // /// Iterate over the real lines underlying the visual lines on
    // the screen with the y position /// of their layout.
    // /// (line, y)
    // /// 应该为视觉行
    // pub fn iter_lines_y(&self) -> impl Iterator<Item = (usize,
    // f64)> + '_ {     let mut last_line = None;
    //     self.lines.iter().filter_map(move |vline| {
    //         let info = self.info(*vline).unwrap();
    //
    //         let line = info.vline_info.origin_line;
    //
    //         if last_line == Some(line) {
    //             // We've already considered this line.
    //             return None;
    //         }
    //
    //         last_line = Some(line);
    //
    //         Some((line, info.y))
    //     })
    // }
    //
    // pub fn iter_line_info_y(&self) -> impl Iterator<Item =
    // LineInfo> + '_ {     self.lines
    //         .iter()
    //         .map(move |vline| self.info(*vline).unwrap())
    // }

    // /// Get the earliest line info for a given line.
    // pub fn info_for_line(&self, line: usize) ->
    // Option<VisualLineInfo> {     self.info(self.
    // first_rvline_for_line(line)?) }

    // /// Get the earliest rvline for the given line
    // pub fn first_rvline_for_line(&self, line: usize) ->
    // Option<RVLine> {     self.lines
    //         .iter()
    //         .find(|rvline| rvline.line == line)
    //         .copied()
    // }

    // /// Get the latest rvline for the given line
    // pub fn last_rvline_for_line(&self, line: usize) ->
    // Option<RVLine> {     self.lines
    //         .iter()
    //         .rfind(|rvline| rvline.line == line)
    //         .copied()
    // }

    pub fn log(&self) {
        info!("{:?}", self.visual_lines);
    }
}

impl ScreenLines {
    pub fn line_interval(&self) -> Result<(usize, usize)> {
        match (self.visual_lines.first(), self.visual_lines.last()) {
            (Some(first), Some(last)) => {
                Ok((first.visual_line.origin_line, last.visual_line.origin_line))
            },
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
    ) -> Option<VisualLineInfo> {
        for visual_line in &self.visual_lines {
            match origin_line.cmp(&visual_line.visual_line.origin_line) {
                Ordering::Less => {
                    return None;
                },
                Ordering::Equal => {
                    return Some(visual_line.clone());
                },
                _ => {}
            }
        }
        None
    }

    /// 获取原始行的视觉行信息。为none则说明被折叠，或者没有在窗口范围
    pub fn visual_line_info_of_origin_line(
        &self,
        origin_line: usize
    ) -> Option<&VisualLineInfo> {
        for visual_line in &self.visual_lines {
            if visual_line.visual_line.origin_line == origin_line
                && visual_line.visual_line.origin_folded_line_sub_index == 0
            {
                return Some(visual_line);
            } else if (visual_line.visual_line.origin_line == origin_line
                && visual_line.visual_line.origin_folded_line_sub_index > 0)
                || visual_line.visual_line.origin_line > origin_line
            {
                break;
            }
        }
        None
    }

    /// 获取原始行的视觉行信息。为none则说明被折叠，或者没有在窗口范围
    pub fn visual_line_info_of_visual_line(
        &self,
        visual_line: &VisualLine
    ) -> Option<&VisualLineInfo> {
        for visual_line_info in &self.visual_lines {
            if visual_line_info.visual_line == *visual_line {
                return Some(visual_line_info);
            } else if (visual_line_info.visual_line.origin_folded_line
                == visual_line.origin_folded_line
                && visual_line_info.visual_line.origin_folded_line_sub_index
                    > visual_line.origin_folded_line_sub_index)
                || visual_line_info.visual_line.origin_folded_line
                    > visual_line.origin_folded_line
            {
                break;
            }
        }
        None
    }

    /// 视窗的最上一行
    pub fn most_up_visual_line_info_of_visual_line(
        &self,
        visual_line: &VisualLine
    ) -> Option<&VisualLineInfo> {
        if let Some(last) = self.visual_lines.last() {
            if let Ordering::Less = last.visual_line.cmp_y(visual_line) {
                return None;
            }
        }
        if let Some(first) = self.visual_lines.first() {
            match first.visual_line.cmp_y(visual_line) {
                Ordering::Less | Ordering::Equal => {
                    let rs = self.visual_line_info_of_visual_line(visual_line);
                    if rs.is_none() {
                        error!("should not be reached");
                    }
                    return rs;
                },
                Ordering::Greater => return Some(first)
            }
        }
        error!("should not be reached");
        None
    }

    /// 视窗的最上一行
    pub fn most_down_visual_line_info_of_visual_line(
        &self,
        visual_line: &VisualLine
    ) -> Option<&VisualLineInfo> {
        if let Some(first) = self.visual_lines.first() {
            if let Ordering::Greater = first.visual_line.cmp_y(visual_line) {
                return None;
            }
        }
        if let Some(last) = self.visual_lines.last() {
            match last.visual_line.cmp_y(visual_line) {
                Ordering::Greater | Ordering::Equal => {
                    let rs = self.visual_line_info_of_visual_line(visual_line);
                    if rs.is_none() {
                        error!("should not be reached");
                    }
                    return rs;
                },
                Ordering::Less => return Some(last)
            }
        }
        error!("should not be reached");
        None
    }
}

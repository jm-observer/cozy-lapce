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

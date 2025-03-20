use std::cell::RefCell;

use cosmic_text::{
    Affinity, BufferLine, Cursor, FontSystem, LayoutLine, LineEnding, Metrics,
    Scroll, ShapeBuffer, Shaping, Wrap,
};
use floem::{
    kurbo::{Point, Size},
    peniko::Color,
    text::{Attrs, AttrsList, FONT_SYSTEM, HitPoint, HitPosition, LayoutRun},
};
use lsp_types::DocumentHighlight;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::lines::{
    delta_compute::Offset, phantom_text::PhantomTextMultiLine, style::NewLineStyle,
    util, util::extra_styles_for_range,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineExtraStyle {
    pub x:          f64,
    pub y:          f64,
    pub width:      Option<f64>,
    pub height:     f64,
    pub bg_color:   Option<Color>,
    pub under_line: Option<Color>,
    pub wave_line:  Option<Color>,
}

/// --以原始文本行为单位，的相关--
///
/// 应该是视觉行（包含了折叠行）的信息
#[derive(Clone, Serialize, Deserialize)]
pub struct TextLayoutLine {
    /// Extra styling that should be applied to the text
    /// (x0, x1 or line display end, style)
    /// todo?暂时没有数据，下划线等？
    extra_style: Vec<LineExtraStyle>,

    document_highlight_style: Vec<LineExtraStyle>,

    #[serde(skip)]
    // 文本：包含折叠行的文本、幽灵文本，及其所有的样式（背景色等）
    pub text: RefCell<TextLayout>,
    // ?
    pub whitespaces:       Option<Vec<(char, (f64, f64))>>,
    // 缩进?
    pub indent:            f64,
    // 幽灵文本相关信息
    pub phantom_text:      PhantomTextMultiLine,
    // 不易于更新迭代？
    pub semantic_styles:   Vec<NewLineStyle>,
    pub diagnostic_styles: Vec<NewLineStyle>,
    init:                  bool,
}

impl TextLayoutLine {
    pub fn new(
        text: RefCell<TextLayout>,
        // ?
        whitespaces: Option<Vec<(char, (f64, f64))>>,
        // 缩进?
        indent: f64,
        // 幽灵文本相关信息
        phantom_text: PhantomTextMultiLine,
        // 不易于更新迭代？
        semantic_styles: Vec<NewLineStyle>,
        diagnostic_styles: Vec<NewLineStyle>,
    ) -> Self {
        Self {
            extra_style: vec![],
            document_highlight_style: vec![],
            text,
            whitespaces,
            indent,
            phantom_text,
            semantic_styles,
            diagnostic_styles,
            init: false,
        }
    }

    // // /// The number of line breaks in the text layout. Always at
    // // least `1`.
    // pub fn line_count(&self) -> usize {
    //     self.text.line_layout().len()
    // }

    //
    // /// Iterate over all the layouts that are nonempty.
    // /// Note that this may be empty if the line is completely
    // empty, like the last line pub fn relevant_layouts(&self) ->
    // impl Iterator<Item = &'_ LayoutLine> + '_ {     // Even
    // though we only have one hard line (and thus only one `lines`
    // entry) typically, for     // normal buffer lines, we can
    // have more than one due to multiline phantom text. So we have
    //     // to sum over all of the entries line counts.
    //     self.text
    //         .lines().layout_opt().into_iter()
    //         .flat_map(|ls| ls.iter())
    //         .filter(|l| !l.glyphs.is_empty())
    // }

    // /// Iterator over the (start, end) columns of the relevant
    // layouts. pub fn layout_cols<'a>(
    //     &'a self,
    //     _text_prov: &'a Editor,
    //     _line: usize,
    // ) -> impl Iterator<Item = (usize, usize)> + 'a {
    //     let mut prefix = None;
    //     // Include an entry if there is nothing
    //         let line_start = self.text.lines_range.start;
    //         if let Some(layouts) = self.text.line().layout_opt() {
    //             // Do we need to require !layouts.is_empty()?
    //             if !layouts.is_empty() && layouts.iter().all(|l|
    // l.glyphs.is_empty()) {                 // We assume the
    // implicit glyph start is zero                 prefix =
    // Some((line_start, line_start));             }
    //         }
    //
    //     // let line_v = line;
    //     let iter = self
    //         .text
    //         .line().layout_opt().into_iter().map(|x| (self
    //
    // .text
    // .line(), self.text.lines_range(), x))
    //         .flat_map(|(line, line_range, ls)| ls.iter().map(move
    // |l| (line, line_range, l)))         .filter(|(_, _, l)|
    // !l.glyphs.is_empty())         .map(move |(tl_line,
    // line_range, l)| {             let line_start =
    // line_range.start;             tl_line.align();
    //
    //             // todo????
    //             let start = line_start + l.glyphs[0].start;
    //             let end = line_start +
    // l.glyphs.last().unwrap().end;
    //
    //             // let text = text_prov.rope_text();
    //             // // We can't just use the original end, because
    // the *true* last glyph on the line             // // may be
    // a space, but it isn't included in the layout! Though this only
    // happens             // // for single spaces, for some
    // reason.             // let pre_end =
    // text_prov.before_phantom_col(line_v, end);             //
    //             //
    //             // // TODO(minor): We don't really need the entire
    // line, just the two characters after             // let
    // line_end = text.line_end_col(line, true);             //
    //             // let end = if pre_end <= line_end {
    //             //     let line_offset = text.offset_of_line(line);
    //             //     let after = text.slice_to_cow(line_offset +
    // pre_end..line_offset + line_end);             //     if
    // after.starts_with(' ') && !after.starts_with("  ") {
    //             //         end + 1
    //             //     } else {
    //             //         end
    //             //      }
    //             // } else {
    //             //     end
    //             // };
    //
    //             (start, end)
    //         });
    //
    //     prefix.into_iter().chain(iter)
    // }
    //
    // /// Iterator over the start columns of the relevant layouts
    // pub fn start_layout_cols<'a>(
    //     &'a self,
    //     text_prov: &'a Editor,
    //     line: usize,
    // ) -> impl Iterator<Item = usize> + 'a {
    //     self.layout_cols(text_prov, line).map(|(start, _)| start)
    // }

    // /// Get the top y position of the given line index
    // pub fn get_layout_y(&self, nth: usize) -> Option<f32> {
    //     self.text.layout_runs().nth(nth).map(|run| run.line_y)
    // }

    // /// Get the (start x, end x) positions of the given line index
    // pub fn get_layout_x(&self, nth: usize) -> Option<(f32, f32)> {
    //     self.text.layout_runs().nth(nth).map(|run| {
    //         (
    //             run.glyphs.first().map(|g| g.x).unwrap_or(0.0),
    //             run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0)
    //         )
    //     })
    // }

    pub fn last_line(&self) -> usize {
        self.phantom_text.last_line
    }

    pub fn adjust(&mut self, line_delta: Offset, offset_delta: Offset) {
        self.phantom_text.adjust(line_delta, offset_delta);
        self.semantic_styles
            .iter_mut()
            .for_each(|x| x.adjust(offset_delta, line_delta));
        self.diagnostic_styles
            .iter_mut()
            .for_each(|x| x.adjust(offset_delta, line_delta));
    }

    pub fn extra_style(&mut self) -> &[LineExtraStyle] {
        &self.extra_style
    }

    pub fn init_extra_style(&mut self) {
        if !self.init {
            self.apply_diagnostic_styles_2();
            self.apply_layout_styles();
        }
    }

    pub fn document_highlight_style(&mut self) -> &[LineExtraStyle] {
        &self.document_highlight_style
    }

    pub fn init(&self) -> bool {
        self.init
    }

    fn apply_layout_styles(&mut self) {
        self.extra_style.clear();
        let layout = &mut self.text.borrow_mut();
        self.phantom_text.iter_phantom_text().for_each(|phantom| {
            if (phantom.bg.is_none() && phantom.under_line.is_none())
                || phantom.text.is_empty()
            {
                return;
            }
            let iter = extra_styles_for_range(
                layout,
                phantom.final_col,
                phantom.final_col + phantom.text.len(),
                phantom.bg,
                phantom.under_line,
                None,
                None,
                true,
            );
            for style in iter {
                self.extra_style.push(style)
            }
        });
    }

    pub fn init_document_highlight(
        &mut self,
        highlight: Vec<DocumentHighlight>,
        fg_color: Color,
        line_height: usize,
    ) {
        let layout = &mut self.text.borrow_mut();
        let phantom_text = &self.phantom_text;
        let mut highlight_styles = vec![];
        for highlight in highlight {
            if let Some((start, end)) = phantom_text.final_col_of_origin_line_col(
                highlight.range.start.line as usize,
                highlight.range.start.character as usize,
                highlight.range.end.line as usize,
                highlight.range.end.character as usize,
            ) {
                let styles = util::extra_styles_for_range(
                    layout,
                    start,
                    end,
                    Some(fg_color),
                    None,
                    None,
                    Some(line_height as f64),
                    false,
                );
                highlight_styles.extend(styles);
            }
        }
        self.document_highlight_style = highlight_styles;
    }

    fn apply_diagnostic_styles_2(&mut self) {
        let layout = &mut self.text.borrow_mut();
        let phantom_text = &self.phantom_text;
        let line_styles = &self.diagnostic_styles;

        // 暂不考虑
        for NewLineStyle {
            fg_color,
            start_of_buffer,
            end_of_buffer,
            ..
        } in line_styles
        {
            match (
                phantom_text.final_col_of_origin_merge_col(
                    *start_of_buffer - phantom_text.offset_of_line,
                ),
                phantom_text.final_col_of_origin_merge_col(
                    *end_of_buffer - phantom_text.offset_of_line,
                ),
            ) {
                (Ok(Some(start)), Ok(Some(end))) => {
                    let styles = util::extra_styles_for_range(
                        layout,
                        start,
                        end + 1,
                        None,
                        None,
                        Some(*fg_color),
                        None,
                        true,
                    );
                    self.extra_style.extend(styles);
                },
                _ => {
                    // maybe be folded
                    continue;
                },
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct TextLayout {
    // only for tracing
    line:       usize,
    pub buffer: BufferLine,
    // ?
    // pub lines_range: Range<usize>,
    width_opt:  Option<f32>,
    height_opt: Option<f32>,

    metrics: Metrics,
    scroll: Scroll,
    /// True if a redraw is requires. Set to false after processing
    redraw: bool,
    wrap: Wrap,
    monospace_width: Option<f32>,
    tab_width: u16,
    /// Scratch buffer for shaping and laying out.
    scratch: ShapeBuffer,
    /// 最终文本长度，包括虚拟文本
    pub(crate) text_len: usize,
    /// 最终文本长度，包括虚拟文本，但不包括末尾的\r\n
    pub(crate) text_len_without_rn: usize,
    init: bool,
}

impl Clone for TextLayout {
    fn clone(&self) -> Self {
        Self {
            line:                self.line,
            buffer:              self.buffer.clone(),
            metrics:             self.metrics,
            width_opt:           self.width_opt,
            height_opt:          self.height_opt,
            scroll:              self.scroll,
            redraw:              self.redraw,
            wrap:                self.wrap,
            monospace_width:     self.monospace_width,
            tab_width:           self.tab_width,
            scratch:             ShapeBuffer::default(),
            text_len:            self.text_len,
            text_len_without_rn: self.text_len_without_rn,
            init:                self.init,
        }
    }
}

impl Default for TextLayout {
    fn default() -> Self {
        Self::new("", AttrsList::new(Attrs::default()), "")
    }
}

impl TextLayout {
    pub fn new<T: Into<String>>(
        text: T,
        attrs_list: AttrsList,
        line_ending: &'static str,
    ) -> Self {
        let mut font_system = FONT_SYSTEM.lock();
        Self::new_with_font_system(
            0,
            text,
            attrs_list,
            &mut font_system,
            line_ending,
        )
    }

    pub fn new_with_font_system<T: Into<String>>(
        line: usize,
        text: T,
        attrs_list: AttrsList,
        font_system: &mut FontSystem,
        line_ending: &'static str,
    ) -> Self {
        Self::new_with_config(
            line,
            text,
            attrs_list,
            font_system,
            None,
            Wrap::WordOrGlyph,
            line_ending,
        )
    }

    pub fn new_without_init<T: Into<String>>(
        line: usize,
        text: T,
        attrs_list: AttrsList,
        width_opt: Option<f32>,
        wrap: Wrap,
        line_ending: &'static str,
    ) -> Self {
        let text = text.into();
        let text_len = text.len();
        // log::info!("{text:?} {line_ending:?}");
        let new_text = text
            .strip_suffix(line_ending)
            .map(|x| x.to_string())
            .unwrap_or(text);
        let text_len_without_rn = new_text.len();
        // log::info!("{new_text:?}");
        let ending = LineEnding::None;
        let text_layout = Self {
            text_len,
            text_len_without_rn,
            line,
            buffer: BufferLine::new(
                new_text,
                ending,
                attrs_list.0,
                Shaping::Advanced,
            ),
            width_opt,
            height_opt: None,
            metrics: Metrics::new(16.0, 16.0),
            scroll: Default::default(),
            redraw: false,
            wrap,
            monospace_width: None,
            tab_width: 8,
            scratch: Default::default(),
            init: false,
        };

        text_layout
    }

    pub fn new_with_config<T: Into<String>>(
        line: usize,
        text: T,
        attrs_list: AttrsList,
        font_system: &mut FontSystem,
        width_opt: Option<f32>,
        wrap: Wrap,
        line_ending: &'static str,
    ) -> Self {
        let text = text.into();
        let text_len = text.len();
        // log::info!("{text:?} {line_ending:?}");
        let new_text = text
            .strip_suffix(line_ending)
            .map(|x| x.to_string())
            .unwrap_or(text);
        let text_len_without_rn = new_text.len();
        // log::info!("{new_text:?}");
        let ending = LineEnding::None;
        let mut text_layout = Self {
            text_len,
            text_len_without_rn,
            line,
            buffer: BufferLine::new(
                new_text,
                ending,
                attrs_list.0,
                Shaping::Advanced,
            ),
            width_opt,
            height_opt: None,
            metrics: Metrics::new(16.0, 16.0),
            scroll: Default::default(),
            redraw: false,
            wrap,
            monospace_width: None,
            tab_width: 8,
            scratch: Default::default(),
            init: false,
        };

        text_layout.shape_until_scroll(font_system, false);
        text_layout
    }

    pub fn line_layout_with_font_system(
        &mut self,
        font_system: &mut FontSystem,
    ) -> &[LayoutLine] {
        self.buffer.layout(
            font_system,
            self.metrics.font_size,
            self.width_opt,
            self.wrap,
            self.monospace_width,
            self.tab_width,
        )
    }

    pub fn line_layout(&mut self) -> &[LayoutLine] {
        self.init_line();
        self.buffer.layout_opt().as_ref().expect("layout_opt empty")
    }

    pub fn init(&self) -> bool {
        self.init
    }

    pub fn init_line(&mut self) {
        if !self.init {
            let mut font_system = FONT_SYSTEM.lock();
            self.shape_until_scroll(&mut font_system, false);
        }
    }

    /// Shape lines until scroll
    fn shape_until_scroll(&mut self, font_system: &mut FontSystem, prune: bool) {
        let metrics = self.metrics;
        let old_scroll = self.scroll;

        loop {
            // Adjust scroll.layout to be positive by moving
            // scroll.line backwards
            while self.scroll.vertical < 0.0 {
                if self.scroll.line > 0 {
                    let line_i = self.scroll.line - 1;
                    let layout = self.line_layout_with_font_system(font_system);
                    let mut layout_height = 0.0;
                    for layout_line in layout.iter() {
                        layout_height += layout_line
                            .line_height_opt
                            .unwrap_or(metrics.line_height);
                    }
                    self.scroll.line = line_i;
                    self.scroll.vertical += layout_height;
                    // } else {
                    //     // If layout is missing, just assume line
                    // height     self.scroll.line
                    // = line_i;     self.scroll.
                    // vertical += metrics.line_height;
                    // }
                } else {
                    self.scroll.vertical = 0.0;
                    break;
                }
            }

            let scroll_start = self.scroll.vertical;
            let scroll_end = scroll_start + self.height_opt.unwrap_or(f32::INFINITY);

            let mut total_height = 0.0;
            for line_i in 0..1 {
                if line_i < self.scroll.line {
                    if prune {
                        self.buffer.reset_shaping();
                    }
                    continue;
                }
                if total_height > scroll_end {
                    if prune {
                        self.buffer.reset_shaping();
                        continue;
                    } else {
                        break;
                    }
                }

                let mut layout_height = 0.0;
                let layout = self.line_layout_with_font_system(font_system);
                for layout_line in layout.iter() {
                    let line_height =
                        layout_line.line_height_opt.unwrap_or(metrics.line_height);
                    layout_height += line_height;
                    total_height += line_height;
                }

                // Adjust scroll.vertical to be smaller by moving
                // scroll.line forwards TODO: do we
                // want to adjust it exactly to a layout line?
                if line_i == self.scroll.line && layout_height < self.scroll.vertical
                {
                    self.scroll.line += 1;
                    self.scroll.vertical -= layout_height;
                }
            }

            if total_height < scroll_end && self.scroll.line > 0 {
                // Need to scroll up to stay inside of buffer
                self.scroll.vertical -= scroll_end - total_height;
            } else {
                // Done adjusting scroll
                break;
            }
        }

        if old_scroll != self.scroll {
            self.redraw = true;
        }
        self.init = true;
    }

    // pub fn set_wrap(&mut self, wrap: Wrap) {
    //     if wrap != self.wrap {
    //         let mut font_system = FONT_SYSTEM.lock();
    //         self.wrap = wrap;
    //         self.relayout(&mut font_system);
    //         self.shape_until_scroll(&mut font_system, false);
    //     }
    // }

    // pub fn set_tab_width(&mut self, tab_width: usize) {
    //     let mut font_system = FONT_SYSTEM.lock();
    //     if tab_width == 0 {
    //         return;
    //     }
    //     let tab_width = tab_width as u16;
    //     if tab_width != self.tab_width {
    //         self.tab_width = tab_width;
    //         // Shaping must be reset when tab width is changed
    //         if self.buffer.shape_opt().is_some() &&
    // self.buffer.text().contains('\t')         {
    //             self.buffer.reset_shaping();
    //         }
    //         self.redraw = true;
    //         self.shape_until_scroll(&mut font_system, false);
    //     }
    // }
    //
    // pub fn set_size(&mut self, width: f32, height: f32) {
    //     let mut font_system = FONT_SYSTEM.lock();
    //     self.width_opt = Some(width);
    //     self.height_opt = Some(height);
    //     self.set_metrics_and_size(
    //         &mut font_system,
    //         self.metrics,
    //         self.width_opt,
    //         self.height_opt
    //     );
    // }
    //
    // fn set_metrics_and_size(
    //     &mut self,
    //     font_system: &mut FontSystem,
    //     metrics: Metrics,
    //     width_opt: Option<f32>,
    //     height_opt: Option<f32>
    // ) {
    //     let clamped_width_opt = width_opt.map(|width| width.max(0.0));
    //     let clamped_height_opt = height_opt.map(|height| height.max(0.0));
    //     // println!("set_metrics_and_size {width_opt:?} {height_opt:?}
    //     // {} {}", metrics != self.metrics, clamped_width_opt !=
    //     // self.width_opt);
    //
    //     if metrics != self.metrics
    //         || clamped_width_opt != self.width_opt
    //         || clamped_height_opt != self.height_opt
    //     {
    //         assert_ne!(metrics.font_size, 0.0, "font size cannot be 0");
    //         self.metrics = metrics;
    //         self.width_opt = clamped_width_opt;
    //         self.height_opt = clamped_height_opt;
    //         self.relayout(font_system);
    //         self.shape_until_scroll(font_system, false);
    //     }
    // }

    pub(crate) fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn layout_runs(&mut self) -> LayoutRunIter {
        self.init_line();
        LayoutRunIter::new(self)
    }

    // pub fn layout_cursor(&mut self, _cursor: Cursor) -> LayoutCursor {
    //     todo!()
    //     // let line = cursor.line;
    //     // let mut font_system = FONT_SYSTEM.lock();
    //     // self.buffer
    //     //     .layout_cursor(&mut font_system, cursor)
    //     //     .unwrap_or_else(|| LayoutCursor::new(line, 0, 0))
    // }

    // fn relayout(&mut self, font_system: &mut FontSystem) {
    //     let line = &mut self.buffer;
    //     if line.shape_opt().is_some() {
    //         line.reset_layout();
    //         line.layout(
    //             font_system,
    //             self.metrics.font_size,
    //             self.width_opt,
    //             self.wrap,
    //             self.monospace_width,
    //             self.tab_width
    //         );
    //     }
    //
    //     self.redraw = true;
    // }

    pub fn hit_position(&mut self, idx: usize) -> HitPosition {
        self.init_line();
        let mut last_line = 0;
        let mut last_end: usize = 0;
        let mut offset = 0;
        let mut last_glyph_width = 0.0;
        let mut last_position = HitPosition {
            line:          0,
            point:         Point::ZERO,
            glyph_ascent:  0.0,
            glyph_descent: 0.0,
        };
        for (line, run) in self.layout_runs().enumerate() {
            if run.line_i > last_line {
                last_line = run.line_i;
                offset += last_end + 1;
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
                    glyph_descent: run.max_descent as f64,
                };
                if (glyph.start + offset..glyph.end + offset).contains(&idx) {
                    return last_position;
                }
            }
        }

        if idx > 0 {
            last_position.point.x += last_glyph_width as f64;
            return last_position;
        }

        HitPosition {
            line:          0,
            point:         Point::ZERO,
            glyph_ascent:  0.0,
            glyph_descent: 0.0,
        }
    }

    pub fn hit_point(&mut self, point: Point) -> HitPoint {
        self.init_line();
        if let Some(cursor) = self.hit(point.x as f32, point.y as f32) {
            let size = self.size();
            let is_inside = point.x <= size.width && point.y <= size.height;
            HitPoint {
                line: cursor.line,
                index: cursor.index,
                is_inside,
            }
        } else {
            HitPoint {
                line:      0,
                index:     0,
                is_inside: false,
            }
        }
    }

    /// Convert x, y position to Cursor (hit detection)
    pub fn hit(&mut self, x: f32, y: f32) -> Option<Cursor> {
        self.init_line();
        let mut new_cursor_opt = None;

        let mut runs = self.layout_runs().peekable();
        let mut first_run = true;
        while let Some(run) = runs.next() {
            let line_top = run.line_top;
            let line_height = run.line_height;

            if first_run && y < line_top {
                first_run = false;
                let new_cursor = Cursor::new(run.line_i, 0);
                new_cursor_opt = Some(new_cursor);
            } else if y >= line_top && y < line_top + line_height {
                let mut new_cursor_glyph = run.glyphs.len();
                let mut new_cursor_char = 0;
                let mut new_cursor_affinity = Affinity::After;

                let mut first_glyph = true;

                'hit: for (glyph_i, glyph) in run.glyphs.iter().enumerate() {
                    if first_glyph {
                        first_glyph = false;
                        if (run.rtl && x > glyph.x) || (!run.rtl && x < 0.0) {
                            new_cursor_glyph = 0;
                            new_cursor_char = 0;
                        }
                    }
                    if x >= glyph.x && x <= glyph.x + glyph.w {
                        new_cursor_glyph = glyph_i;

                        let cluster = &run.text[glyph.start..glyph.end];
                        let total = cluster.grapheme_indices(true).count();
                        let mut egc_x = glyph.x;
                        let egc_w = glyph.w / (total as f32);
                        for (egc_i, egc) in cluster.grapheme_indices(true) {
                            if x >= egc_x && x <= egc_x + egc_w {
                                new_cursor_char = egc_i;

                                let right_half = x >= egc_x + egc_w / 2.0;
                                if right_half != glyph.level.is_rtl() {
                                    // If clicking on last half of
                                    // glyph, move cursor past glyph
                                    new_cursor_char += egc.len();
                                    new_cursor_affinity = Affinity::Before;
                                }
                                break 'hit;
                            }
                            egc_x += egc_w;
                        }

                        let right_half = x >= glyph.x + glyph.w / 2.0;
                        if right_half != glyph.level.is_rtl() {
                            // If clicking on last half of glyph, move
                            // cursor past glyph
                            new_cursor_char = cluster.len();
                            new_cursor_affinity = Affinity::Before;
                        }
                        break 'hit;
                    }
                }

                let mut new_cursor = Cursor::new(run.line_i, 0);

                match run.glyphs.get(new_cursor_glyph) {
                    Some(glyph) => {
                        // Position at glyph
                        new_cursor.index = glyph.start + new_cursor_char;
                        new_cursor.affinity = new_cursor_affinity;
                    },
                    None => {
                        if let Some(glyph) = run.glyphs.last() {
                            // Position at end of line
                            new_cursor.index = glyph.end;
                            new_cursor.affinity = Affinity::Before;
                        }
                    },
                }

                new_cursor_opt = Some(new_cursor);

                break;
            } else if runs.peek().is_none() && y > run.line_y {
                let mut new_cursor = Cursor::new(run.line_i, 0);
                if let Some(glyph) = run.glyphs.last() {
                    new_cursor = run.cursor_from_glyph_right(glyph);
                }
                new_cursor_opt = Some(new_cursor);
            }
        }

        new_cursor_opt
    }

    // pub fn line_col_position(&self, line: usize, col: usize) -> HitPosition {
    //     let mut last_glyph: Option<&LayoutGlyph> = None;
    //     let mut last_line = 0;
    //     let mut last_line_y = 0.0;
    //     let mut last_glyph_ascent = 0.0;
    //     let mut last_glyph_descent = 0.0;
    //     for (current_line, run) in self.layout_runs().enumerate() {
    //         for glyph in run.glyphs {
    //             match run.line_i.cmp(&line) {
    //                 std::cmp::Ordering::Equal => {
    //                     if glyph.start > col {
    //                         return HitPosition {
    //                             line:          last_line,
    //                             point:         Point::new(
    //                                 last_glyph
    //                                     .map(|g| (g.x + g.w) as f64)
    //                                     .unwrap_or(0.0),
    //                                 last_line_y as f64
    //                             ),
    //                             glyph_ascent:  last_glyph_ascent as f64,
    //                             glyph_descent: last_glyph_descent as f64
    //                         };
    //                     }
    //                     if (glyph.start..glyph.end).contains(&col) {
    //                         return HitPosition {
    //                             line:          current_line,
    //                             point:         Point::new(
    //                                 glyph.x as f64,
    //                                 run.line_y as f64
    //                             ),
    //                             glyph_ascent:  run.max_ascent as f64,
    //                             glyph_descent: run.max_descent as f64
    //                         };
    //                     }
    //                 },
    //                 std::cmp::Ordering::Greater => {
    //                     return HitPosition {
    //                         line:          last_line,
    //                         point:         Point::new(
    //                             last_glyph
    //                                 .map(|g| (g.x + g.w) as f64)
    //                                 .unwrap_or(0.0),
    //                             last_line_y as f64
    //                         ),
    //                         glyph_ascent:  last_glyph_ascent as f64,
    //                         glyph_descent: last_glyph_descent as f64
    //                     };
    //                 },
    //                 std::cmp::Ordering::Less => {}
    //             };
    //             last_glyph = Some(glyph);
    //         }
    //         last_line = current_line;
    //         last_line_y = run.line_y;
    //         last_glyph_ascent = run.max_ascent;
    //         last_glyph_descent = run.max_descent;
    //     }
    //
    //     HitPosition {
    //         line:          last_line,
    //         point:         Point::new(
    //             last_glyph.map(|g| (g.x + g.w) as f64).unwrap_or(0.0),
    //             last_line_y as f64
    //         ),
    //         glyph_ascent:  last_glyph_ascent as f64,
    //         glyph_descent: last_glyph_descent as f64
    //     }
    // }

    pub fn size(&mut self) -> Size {
        if self.init {
            self.layout_runs()
                .fold(Size::new(0.0, 0.0), |mut size, run| {
                    let new_width = run.line_w as f64;
                    // if line == 9 {
                    //     println!("new_width {new_width}");
                    // }
                    if new_width > size.width {
                        size.width = new_width;
                    }

                    size.height += run.line_height as f64;

                    size
                })
        } else {
            Size::new(0.0, 0.0)
        }
    }
}

/// An iterator of visible text lines, see [`LayoutRun`]
#[derive(Debug)]
pub struct LayoutRunIter<'b> {
    text_layout:  &'b TextLayout,
    line_i:       usize,
    layout_i:     usize,
    total_height: f32,
    line_top:     f32,
}

impl<'b> LayoutRunIter<'b> {
    pub fn new(text_layout: &'b TextLayout) -> Self {
        Self {
            text_layout,
            line_i: 0,
            layout_i: 0,
            total_height: 0.0,
            line_top: 0.0,
        }
    }
}

impl<'b> Iterator for LayoutRunIter<'b> {
    type Item = LayoutRun<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.line_i > 0 {
            return None;
        }
        let line = &self.text_layout.buffer;
        let layout = line.layout_opt().as_ref()?;
        let shape = line.shape_opt().as_ref()?;
        assert_eq!(self.line_i, 0);
        while let Some(layout_line) = layout.get(self.layout_i) {
            self.layout_i += 1;

            let line_height = layout_line.line_height_opt.unwrap();
            self.total_height += line_height;

            let line_top = self.line_top;
            let glyph_height = layout_line.max_ascent + layout_line.max_descent;
            let centering_offset = (line_height - glyph_height) / 2.0;
            let line_y = line_top + centering_offset + layout_line.max_ascent;
            if let Some(height) = self.text_layout.height_opt {
                if line_y > height {
                    return None;
                }
            }
            self.line_top += line_height;
            if line_y < 0.0 {
                continue;
            }

            return Some(LayoutRun {
                line_i: self.line_i,
                text: line.text(),
                rtl: shape.rtl,
                glyphs: &layout_line.glyphs,
                max_ascent: layout_line.max_ascent,
                max_descent: layout_line.max_descent,
                line_y,
                line_top,
                line_height,
                line_w: layout_line.w,
            });
        }
        self.line_i += 1;
        None
    }
}

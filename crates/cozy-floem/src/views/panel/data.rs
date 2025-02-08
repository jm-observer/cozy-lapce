pub use cursor::*;
use doc::lines::{line_ending::LineEnding, word::WordCursor};
use floem::{
    Clipboard, ViewId,
    kurbo::{Point, Rect, Size},
    peniko::Color,
    pointer::{PointerInputEvent, PointerMoveEvent},
    prelude::{RwSignal, SignalGet, SignalUpdate, SignalWith, palette},
    reactive::{Scope, batch},
    taffy::NodeId,
    text::{Attrs, FamilyOwned, LineHeightValue}
};
pub use lines::*;
use log::{error, info};

use crate::views::tree_with_panel::data::{StyledText, VisualLine};

mod cursor;
mod lines;

#[derive(Clone, Copy)]
pub struct DocManager {
    pub panel_id:   ViewId,
    pub inner_node: Option<NodeId>,
    doc:            RwSignal<SimpleDoc>
}

impl DocManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(cx: Scope, id: ViewId, doc_style: DocStyle) -> Self {
        let hover_hyperlink = cx.create_rw_signal(None);
        Self {
            panel_id:   id,
            inner_node: None,
            doc:        cx.create_rw_signal(SimpleDoc::new(
                id,
                hover_hyperlink,
                doc_style
            ))
        }
    }

    pub fn with_untracked<O>(&self, f: impl FnOnce(&SimpleDoc) -> O) -> O {
        self.doc.with_untracked(f)
    }

    pub fn get(&self) -> SimpleDoc {
        self.doc.get()
    }

    pub fn update(&self, f: impl FnOnce(&mut SimpleDoc)) {
        // not remove `batch`!
        batch(|| {
            self.doc.update(f);
        });
    }

    pub fn try_update<O>(&self, f: impl FnOnce(&mut SimpleDoc) -> O) -> Option<O> {
        // not remove `batch`!
        batch(|| self.doc.try_update(f))
    }
}

#[derive(Clone, Debug)]
pub struct DocStyle {
    pub font_family:  String,
    pub font_size:    f32,
    pub line_height:  f64,
    pub selection_bg: Color,
    pub fg_color:     Color
}

impl DocStyle {
    pub fn attrs<'a>(&self, family: &'a [FamilyOwned]) -> Attrs<'a> {
        Attrs::new()
            .family(family)
            .font_size(self.font_size)
            .line_height(LineHeightValue::Px(self.line_height as f32))
    }
}

impl Default for DocStyle {
    fn default() -> Self {
        Self {
            font_family:  "JetBrains Mono".to_string(),
            font_size:    13.0,
            line_height:  23.0,
            selection_bg: palette::css::BLUE_VIOLET,
            fg_color:     Color::BLACK
        }
    }
}

#[derive(Clone)]
pub struct SimpleDoc {
    pub id:              ViewId,
    // pub visual_line:       Vec<VisualLine>,
    pub line_ending:     LineEnding,
    pub viewport:        Rect,
    pub cursor:          Cursor,
    pub hover_hyperlink: RwSignal<Option<usize>>,
    pub style:           DocStyle,
    pub auto_scroll:     bool,
    pub lines:           Lines
}

impl SimpleDoc {
    pub fn new(
        id: ViewId,
        hover_hyperlink: RwSignal<Option<usize>>,
        style: DocStyle
    ) -> Self {
        Self {
            id,
            // visual_line: vec![],
            line_ending: LineEnding::Lf,
            viewport: Default::default(),
            cursor: Cursor {
                dragging: false,
                position: Position::None
            },
            hover_hyperlink,
            style,
            auto_scroll: true,
            lines: Default::default()
        }
    }

    /// return (offset_of_buffer, line)
    pub fn offset_of_pos(&self, point: Point) -> anyhow::Result<(usize, usize)> {
        let last_line = self.lines.lines_len()?;
        let line = (point.y / self.style.line_height) as usize;
        if line >= last_line {
            return Ok((
                self.lines.line_info()?.0.len().max(1) - 1,
                last_line.max(1) - 1
            ));
        }
        let text = self.lines.text_layout_of_line(line)?;

        let hit_point = text.hit_point(Point::new(point.x, 0.0));
        let offset = self.offset_of_line(line)? + hit_point.index;
        // debug!(
        //     "offset_of_pos point={point:?} line={line} index={}
        // offset={offset}\      self.visual_line.len()={}",
        //     hit_point.index,
        //     self.lines.lines_len()
        // );
        Ok((offset, line))
    }

    pub fn pointer_down(&mut self, event: PointerInputEvent) -> anyhow::Result<()> {
        match event.count {
            1 => {
                if self.hover_hyperlink.get_untracked().is_some() {
                    if let Some(link) = self.lines.hyperlink_by_point(event.pos)? {
                        info!("todo {:?}", link);
                    }
                }
                let offset = self.offset_of_pos(event.pos)?.0;
                self.cursor.dragging = true;
                if event.modifiers.shift() {
                    self.cursor.position = Position::Region {
                        start: self.cursor.start().unwrap_or(offset),
                        end:   offset
                    };
                } else {
                    self.cursor.position = Position::Caret(offset);
                }
                self.id.request_paint();
            },
            2 => {
                let offset = self.offset_of_pos(event.pos)?.0;
                let (start_code, end_code) =
                    WordCursor::new(&self.lines.line_info()?.0, offset)
                        .select_word();
                self.cursor.position = Position::Region {
                    start: start_code,
                    end:   end_code
                };
                self.id.request_paint();
            },
            _ => {
                let line = self.offset_of_pos(event.pos)?.1;
                let offset = self.offset_of_line(line)?;
                let next_line_offset = self.offset_of_line(line + 1)?;
                // info!(
                //     "line={line} offset={offset} \
                //      next_line_offset={next_line_offset} len={} \
                //      line={}",
                //     self.lines.rope().len(),
                //     self.lines
                //         .rope()
                //         .line_of_offset(self.lines.rope().len())
                // );
                self.cursor.position = Position::Region {
                    start: offset,
                    end:   next_line_offset
                };
                self.id.request_paint();
            }
        }
        Ok(())
    }

    pub fn pointer_move(&mut self, event: PointerMoveEvent) -> anyhow::Result<()> {
        if let Some(x) = self.lines.in_hyperlink_region(event.pos)? {
            if self.hover_hyperlink.get_untracked().is_none() {
                self.hover_hyperlink.set(Some(x));
            }
        } else if self.hover_hyperlink.get_untracked().is_some() {
            self.hover_hyperlink.set(None);
        }
        if self.cursor.dragging {
            let offset = self.offset_of_pos(event.pos)?.0;
            self.cursor.position = Position::Region {
                start: self.cursor.start().unwrap_or(offset),
                end:   offset
            };
            self.id.request_paint();
        }
        Ok(())
    }

    pub fn pointer_up(&mut self, _event: PointerInputEvent) -> anyhow::Result<()> {
        self.cursor.dragging = false;
        Ok(())
    }

    pub fn copy_select(&self) -> anyhow::Result<()> {
        if let Some((start, end)) = self.cursor.region() {
            let content = self
                .lines
                .line_info()?
                .0
                .slice_to_cow(start..end)
                .to_string();
            if let Err(err) = Clipboard::set_contents(content) {
                error!("{err:?}");
            }
        }
        Ok(())
    }

    pub fn position_of_cursor(&self) -> anyhow::Result<Option<Rect>> {
        let Some(offset) = self.cursor.offset() else {
            return Ok(None);
        };
        let Some((point, _line, _)) = self.point_of_offset(offset)? else {
            return Ok(None);
        };
        // debug!(
        //     "position_of_cursor offset={offset}, point={point:?}, \
        //      line={_line}"
        // );
        let rect = Rect::from_origin_size(
            (point.x - 1.0, point.y),
            (2.0, self.style.line_height)
        );
        Ok(Some(rect))
    }

    fn point_of_offset(
        &self,
        offset: usize
    ) -> anyhow::Result<Option<(Point, usize, usize)>> {
        let rs = self.lines.point_of_offset(offset)?;
        Ok(rs.map(|(mut point, line, offset)| {
            point.y = self.height_of_line(line);
            (point, line, offset)
        }))
    }

    fn height_of_line(&self, line: usize) -> f64 {
        line as f64 * self.style.line_height
    }

    pub fn select_of_cursor(&self) -> anyhow::Result<Vec<Rect>> {
        let Some((start_offset, end_offset)) = self.cursor.region() else {
            return Ok(vec![]);
        };
        let Some((start_point, mut start_line, _)) =
            self.point_of_offset(start_offset)?
        else {
            return Ok(vec![]);
        };
        let Some((mut end_point, end_line, _)) = self.point_of_offset(end_offset)?
        else {
            return Ok(vec![]);
        };
        end_point.y += self.style.line_height;
        if start_line == end_line {
            Ok(vec![Rect::from_points(start_point, end_point)])
        } else {
            let mut rects = Vec::with_capacity(end_line - start_line + 1);
            let viewport_width = self.viewport.width();
            rects.push(Rect::from_origin_size(
                start_point,
                (viewport_width, self.style.line_height)
            ));
            start_line += 1;
            while start_line < end_line {
                rects.push(Rect::from_origin_size(
                    Point::new(0.0, self.height_of_line(start_line)),
                    (viewport_width, self.style.line_height)
                ));
                start_line += 1;
            }
            rects.push(Rect::from_points(
                Point::new(0.0, self.height_of_line(start_line)),
                end_point
            ));
            Ok(rects)
        }
    }

    // pub fn append_line(
    //     &mut self,
    //     Line {
    //         content,
    //         attrs_list,
    //         hyperlink
    //     }: Line
    // ) {
    //     let len = self.rope.len();
    //     if len > 0 {
    //         self.rope.edit(len..len, self.line_ending.get_chars());
    //     }
    //     self.rope.edit(self.rope.len()..self.rope.len(), &content);
    //     let line_index = self.line_of_offset(self.rope.len());
    //     let y =
    //         self.height_of_line(line_index) +
    // self.style.line_height;     let mut font_system =
    // FONT_SYSTEM.lock();     let text =
    // TextLayout::new_with_font_system(         line_index,
    //         content,
    //         attrs_list,
    //         &mut font_system
    //     );
    //     let points: Vec<(f64, f64, Hyperlink)> = hyperlink
    //         .into_iter()
    //         .map(|x| {
    //             let range = x.range();
    //             let x0 = text.hit_position(range.start).point.x;
    //             let x1 = text.hit_position(range.end).point.x;
    //             (x0, x1, x)
    //         })
    //         .collect();
    //     let hyperlinks: Vec<(Point, Point, Color)> = points
    //         .iter()
    //         .map(|(x0, x1, _link)| {
    //             (
    //                 Point::new(*x0, y - 1.0),
    //                 Point::new(*x1, y - 1.0),
    //                 self.style.fg_color
    //             )
    //         })
    //         .collect();
    //     let mut hyperlink_region: Vec<(Rect, Hyperlink)> = points
    //         .into_iter()
    //         .map(|(x0, x1, data)| {
    //             (
    //                 Rect::new(x0, y - self.style.line_height, x1,
    // y),                 data
    //             )
    //         })
    //         .collect();
    //     self.visual_line.push(VisualLine {
    //         pos_y: self.height_of_line(line_index),
    //         line_index,
    //         text_layout: TextLayoutLine { text, hyperlinks },
    //         text_src: TextSrc::StdErr { level: ErrLevel::None },
    //     });
    //     self.hyperlink_regions.append(&mut hyperlink_region);
    //     self.id.request_layout();
    //     self.id.request_paint();
    //     if self.auto_scroll {
    //         self.id.scroll_to(Some(Rect::from_origin_size(
    //             Point::new(
    //                 self.viewport.x0,
    //                 self.height_of_line(line_index)
    //             ),
    //             Size::new(
    //                 self.style.line_height,
    //                 self.style.line_height
    //             )
    //         )));
    //     }
    // }
    //
    // pub fn append_lines<T: Styled>(
    //     &mut self,
    //     lines: T
    // ) -> Result<()> {
    //     let mut old_len = self.rope.len();
    //     if old_len > 0 && self.rope.byte_at(old_len - 1) != '\n' as
    // u8 {             self.rope.edit(
    //                 old_len..old_len,
    //                 self.line_ending.get_chars()
    //             );
    //             old_len += self.line_ending.len();
    //     }
    //     self.rope
    //         .edit(self.rope.len()..self.rope.len(),
    // lines.content());
    //
    //     let old_line = self.line_of_offset(old_len);
    //     let mut last_line = self.line_of_offset(self.rope.len());
    //     // 新内容如果没有\n则会导致二者相等
    //     if last_line == old_line {
    //         last_line += 1;
    //     }
    //     let family = Cow::Owned(
    //         FamilyOwned::parse_list(&self.style.font_family)
    //             .collect()
    //     );
    //     // debug!(
    //     //     "last_line={last_line} old_line={old_line}
    // content={}",     //     lines.content().len()
    //     // );
    //     let mut delta = 0;
    //     let trim_str = ['\r', '\n'];
    //     let text_src = lines.src();
    //     for line_index in old_line..last_line {
    //         let start_offset =
    //             self.offset_of_line(line_index)?;
    //         let end_offset =
    //             self.offset_of_line(line_index + 1)?;
    //         let mut attrs_list =
    //             AttrsList::new(self.style.attrs(&family));
    //         let rang = start_offset - old_len..end_offset -
    // old_len;         let mut font_system = FONT_SYSTEM.lock();
    //         let content_origin =
    //             self.rope.slice_to_cow(start_offset..end_offset);
    //         let content =
    // content_origin.trim_end_matches(&trim_str);         //
    // debug!("line_index={line_index} rang={rang:?}         //
    // content={content}");         let hyperlink =
    // lines.line_attrs(             &mut attrs_list,
    //             self.style.attrs(&family),
    //             rang,
    //             delta
    //         );
    //         let text = TextLayout::new_with_font_system(
    //             line_index,
    //             content,
    //             attrs_list,
    //             &mut font_system
    //         );
    //         let points: Vec<(f64, f64, Hyperlink)> = hyperlink
    //             .into_iter()
    //             .map(|x| {
    //                 let range = x.range();
    //                 let x0 =
    // text.hit_position(range.start).point.x;                 let
    // x1 = text.hit_position(range.end).point.x;
    // (x0, x1, x)             })
    //             .collect();
    //
    //         let y = self.height_of_line(line_index)
    //             + self.style.line_height;
    //         // let hyperlinks: Vec<(Point, Point, Color)> = vec![];
    //         let hyperlinks: Vec<(Point, Point, Color)> = points
    //             .iter()
    //             .map(|(x0, x1, _link)| {
    //                 (
    //                     Point::new(*x0, y - 1.0),
    //                     Point::new(*x1, y - 1.0),
    //                     self.style.fg_color
    //                 )
    //             })
    //             .collect();
    //         let mut hyperlink_region: Vec<(Rect, Hyperlink)> =
    // points             .into_iter()
    //             .map(|(x0, x1, data)| {
    //                 (
    //                     Rect::new(
    //                         x0,
    //                         y - self.style.line_height,
    //                         x1,
    //                         y
    //                     ),
    //                     data
    //                 )
    //             })
    //             .collect();
    //         self.visual_line.push(VisualLine {
    //             pos_y: self.height_of_line(line_index),
    //             line_index,
    //             text_layout: TextLayoutLine { text, hyperlinks },
    //             text_src: text_src.clone(),
    //         });
    //         self.hyperlink_regions.append(&mut hyperlink_region);
    //         delta += end_offset - start_offset;
    //     }
    //
    //     self.id.request_layout();
    //     self.id.request_paint();
    //     if self.auto_scroll {
    //         self.id.scroll_to(Some(Rect::from_origin_size(
    //             Point::new(
    //                 self.viewport.x0,
    //                 self.height_of_line(
    //                     self.line_of_offset(self.rope.len())
    //                 )
    //             ),
    //             Size::new(
    //                 self.style.line_height,
    //                 self.style.line_height
    //             )
    //         )));
    //     }
    //     Ok(())
    // }

    pub fn append_lines(&mut self, lines: StyledText) -> anyhow::Result<()> {
        let lines = lines.to_lines()?;
        self.lines
            .append_lines(lines, self.line_ending, &self.style)?;

        self.id.request_layout();
        self.id.request_paint();
        if let Err(err) = self.auto_scroll(false) {
            error!("{err:?}");
        }
        Ok(())
    }

    fn offset_of_line(&self, line: usize) -> anyhow::Result<usize> {
        self.lines.line_info()?.0.offset_of_line(line)
    }

    fn line_of_offset(&self, offset: usize) -> anyhow::Result<usize> {
        Ok(self.lines.line_info()?.0.line_of_offset(offset))
    }

    pub fn view_size(&self) -> Size {
        match self
            .lines
            .visual_lines_size(self.viewport, self.style.line_height)
        {
            Ok(size) => size,
            Err(err) => {
                error!("{err:?}");
                Size::new(0., 0.)
            }
        }
    }

    pub fn viewport_lines(&self) -> Vec<VisualLine> {
        match self.lines.visual_lines(
            self.viewport,
            self.style.line_height,
            self.style.fg_color
        ) {
            Ok(lines) => lines,
            Err(err) => {
                error!("{err:?}");
                vec![]
            }
        }
    }

    pub fn update_viewport_by_scroll(&mut self, viewport: Rect) {
        let viewport_size = viewport.size();
        // viewport_size.height -= self.style.line_height / 0.5;
        // viewport_size.width -= self.style.line_height * 1.5;
        self.viewport = viewport.with_size(viewport_size);
        // info!("update_viewport_by_scroll {:?} {:?}",
        // viewport.size(), self.viewport.size());
        self.id.request_layout();
    }

    pub fn update_display(&mut self, id: DisplayId) {
        // info!("update_display {:?}", id);
        self.lines.display(id);
        self.id.request_layout();
        self.id.request_paint();
        self.id.scroll_to(Some(Rect::new(
            0.0,
            0.0,
            self.style.line_height,
            self.style.line_height
        )));
        // self.auto_scroll(true);
    }

    fn auto_scroll(&self, force: bool) -> anyhow::Result<()> {
        if self.auto_scroll || force {
            let len = self.lines.line_info()?.0.len();
            let line = self.line_of_offset(len)?;
            let rect = Rect::from_origin_size(
                Point::new(self.viewport.x0, self.height_of_line(line)),
                Size::new(self.style.line_height, self.style.line_height)
            );
            // debug!("auto_scroll {rect:?} len={len} line={line}",);
            self.id.scroll_to(Some(rect));
        }
        Ok(())
    }
}

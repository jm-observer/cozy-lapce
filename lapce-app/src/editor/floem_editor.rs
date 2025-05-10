use std::{borrow::Cow, ops::Range};

use anyhow::Result;
use doc::lines::{
    DocLinesManager,
    command::EditCommand,
    cursor::{Cursor, CursorAffinity, CursorMode},
    layout::LineExtraStyle,
    mode::{MotionMode, VisualMode},
    register::Register,
    screen_lines::{ScreenLines, VisualLineInfo},
};
use floem::{
    Renderer,
    context::PaintCx,
    kurbo::{BezPath, Line, Point, Rect, Size, Stroke},
    peniko,
    peniko::Color,
    text::{Attrs, AttrsList, FamilyOwned, TextLayout},
};
use log::error;

use super::EditorData;

// pub(crate) const CHAR_WIDTH: f64 = 7.5;

// /// The main structure for the editor view itself.
// /// This can be considered to be the data part of the `View`.
// /// It holds an `Rc<Doc>` within as the document it is a view into.
// #[derive(Clone)]
// pub struct Editor {
//     pub cx:     Cell<Scope>,
//     effects_cx: Cell<Scope>,

//     id: EditorId,
//     // pub active: RwSignal<bool>,

//     // /// Whether you can edit within this editor.
//     // pub read_only: RwSignal<bool>,

//     // pub cursor: RwSignal<Cursor>,
//     // pub editor_view_focused:    Trigger,
//     // pub editor_view_focus_lost: Trigger,
//     // /// Modal mode register
//     // pub register: RwSignal<Register>,
//     // pub last_movement: RwSignal<Movement>,
//     // /// Whether ime input is allowed.
//     // /// Should not be set manually outside of the specific handling for
// ime.     // pub ime_allowed: RwSignal<bool>,

//     // /// The Editor Style
//     // pub es: RwSignal<EditorStyle>,
//     // pub floem_style_id:       RwSignal<u64>, // pub lines:
// DocLinesManager,     // pub sticky_header_height: RwSignal<f64>,
//     // pub sticky_header_info:   RwSignal<StickyHeaderInfo>,
// }
// impl Editor {
//     /// Create a new editor into the given document, using the styling.
//     /// `id` should typically be constructed by [`EditorId::next`]
//     /// `doc`: The backing [`Document`], such as
//     /// [TextDocument](self::text_document::TextDocument) `style`: How the
//     /// editor should be styled, such as
//     /// [SimpleStyling](self::text::SimpleStyling)
//     pub fn new(cx: Scope, id: EditorId) -> Editor {
//         let editor = Editor::new_direct(cx, id);
//         editor.recreate_view_effects();

//         editor
//     }

//     // TODO: shouldn't this accept an `RwSignal<Rc<Doc>>` so that it can
// listen for     // changes in other editors?
//     // TODO: should we really allow callers to arbitrarily specify the Id?
// That     // could open up confusing behavior.

//     /// Create a new editor into the given document, using the styling.
//     /// `id` should typically be constructed by [`EditorId::next`]
//     /// `doc`: The backing [`Document`], such as
//     /// [TextDocument](self::text_document::TextDocument) `style`: How the
//     /// editor should be styled, such as
//     /// [SimpleStyling](self::text::SimpleStyling) This does *not* create
//     /// the view effects. Use this if you're creating an editor and then
//     /// replacing signals. Invoke [`Editor::recreate_view_effects`] when you
// are     /// done. ```rust,ignore
//     /// let shared_scroll_beyond_last_line = /* ... */;
//     /// let editor = Editor::new_direct(cx, id, doc, style);
//     /// editor.scroll_beyond_last_line.set(shared_scroll_beyond_last_line);
//     /// ```
//     pub fn new_direct(cx: Scope, id: EditorId) -> Editor {
//         // let id = doc.editor_id();
//         // let viewport = doc.viewport();
//         let cx = cx.create_child();

//         Editor {
//             cx: Cell::new(cx),
//             // lines,
//             effects_cx: Cell::new(cx.create_child()),
//             id,
//             // active: cx.create_rw_signal(false),
//             // read_only: cx.create_rw_signal(false),
//             // editor_view_focused: cx.create_trigger(),
//             // editor_view_focus_lost: cx.create_trigger(),
//             // editor_view_id: cx.create_rw_signal(None),
//             // screen_lines,
//             // register: cx.create_rw_signal(Register::default()),
//             // cursor_info: CursorInfo::new(cx),
//             // last_movement: cx.create_rw_signal(Movement::Left),
//             // ime_allowed: cx.create_rw_signal(false),
//             // floem_style_id: cx.create_rw_signal(0),
//             // screen_lines,
//             // folding_display_item,
//             // sticky_header_height: cx.create_rw_signal(0.0),
//             // sticky_header_info:
// cx.create_rw_signal(StickyHeaderInfo::default()),             // kind,
//             // visual_lines,
//         }
//     }

//     pub fn id(&self) -> EditorId {
//         self.id
//     }

//     // /// Get the document untracked
//     // pub fn doc(&self) -> Rc<Doc> {
//     //     self.doc.get_untracked()
//     // }

//     // pub fn doc_track(&self) -> Rc<Doc> {
//     //     self.doc.get()
//     // }

//     // // TODO: should this be `ReadSignal`? but read signal doesn't have
// .track     // pub fn doc_signal(&self) -> RwSignal<Rc<Doc>> {
//     //     self.doc
//     // }

//     // pub fn config_id(&self) -> ConfigId {
//     //     let style_id = self.doc.with(|s| s.id());
//     //     let floem_style_id = self.floem_style_id;
//     //     ConfigId::new(style_id, floem_style_id.get_untracked())
//     // }

//     pub fn recreate_view_effects(&self) {
//         batch(|| {
//             self.effects_cx.get().dispose();
//             self.effects_cx.set(self.cx.get().create_child());
//             // create_view_effects(self.effects_cx.get(), self);
//         });
//     }

//     // /// Swap the underlying document out
//     // pub fn update_doc(&self, doc: Rc<Doc>) {
//     //     info!("update_doc");
//     //     batch(|| {
//     //         // Get rid of all the effects
//     //         self.effects_cx.get().dispose();
//     //         self.doc.set(doc);
//     //         // self.doc()
//     //         //     .lines
//     //         //     .update(|lines| lines.trigger_signals_force());
//     //
//     //         // Recreate the effects
//     //         self.effects_cx.set(self.cx.get().create_child());
//     //         // create_view_effects(self.effects_cx.get(), self);
//     //     });
//     // }

//     // pub fn update_styling(&self, styling: Rc<dyn Styling>) {
//     //     batch(|| {
//     //         // Get rid of all the effects
//     //         self.effects_cx.get().dispose();
//     //
//     //         // let font_sizes = Rc::new(EditorFontSizes {
//     //         //     id: self.id(),
//     //         //     style: self.style.read_only(),
//     //         //     doc: self.doc.read_only(),
//     //         // });
//     //
//     //         let ed = self.clone();
//     //         self.lines.update(|x| {
//     //             x.update(&ed);
//     //         });
//     //         //
//     //         // *self.lines.font_sizes.borrow_mut() =
//     //         // self.lines.clear(0, None);
//     //
//     //         self.style.set(styling);
//     //
//     //         self.screen_lines.update(|screen_lines| {
//     //             screen_lines.clear(self.viewport.get_untracked());
//     //         });
//     //
//     //         // Recreate the effects
//     //         self.effects_cx.set(self.cx.get().create_child());
//     //         create_view_effects(self.effects_cx.get(), self);
//     //     });
//     // }

//     // pub fn duplicate(&self, editor_id: Option<EditorId>) -> Editor {
//     //     let doc = self.doc();
//     //     let style = self.style();
//     //     let mut editor = Editor::new_direct(
//     //         self.cx.get(),
//     //         editor_id.unwrap_or_else(EditorId::next),
//     //         doc,
//     //         style,
//     //         false,
//     //     );
//     //
//     //     batch(|| {
//     //         editor.read_only.set(self.read_only.get_untracked());
//     //         editor.es.set(self.es.get_untracked());
//     //         editor
//     //             .floem_style_id
//     //             .set(self.floem_style_id.get_untracked());
//     //         editor.cursor.set(self.cursor.get_untracked());
//     //         editor.scroll_delta.set(self.scroll_delta.get_untracked());
//     //         editor.scroll_to.set(self.scroll_to.get_untracked());
//     //         editor.window_origin.set(self.window_origin.get_untracked());
//     //         editor.viewport.set(self.viewport.get_untracked());
//     //         editor.parent_size.set(self.parent_size.get_untracked());
//     //         editor.register.set(self.register.get_untracked());
//     //         editor.cursor_info = self.cursor_info.clone();
//     //         editor.last_movement.set(self.last_movement.get_untracked());
//     //         // ?
//     //         // editor.ime_allowed.set(self.ime_allowed.get_untracked());
//     //     });
//     //
//     //     editor.recreate_view_effects();
//     //
//     //     editor
//     // }

//     // /// Get the styling untracked
//     // pub fn style(&self) -> Rc<dyn Styling> {
//     //     self.doc.get_untracked()
//     // }

//     // /// Get the text of the document
//     // /// You should typically prefer [`Self::rope_text`]
//     // pub fn text(&self) -> Rope {
//     //     self.doc().text()
//     // }

//     // /// Get the [`RopeTextVal`] from `doc` untracked
//     // pub fn rope_text(&self) -> RopeTextVal {
//     //     self.doc().rope_text()
//     // }

//     // pub fn vline_infos(&self, start: usize, end: usize) ->
// Vec<VLineInfo<VLine>>     // {     self.doc()
//     //         .lines
//     //         .with_untracked(|x| x.vline_infos(start, end))
//     // }

//     pub fn text_prov(&self) -> &Self {
//         self
//     }

//     pub fn set_preedit(
//         &self,
//         text: String,
//         cursor: Option<(usize, usize)>,
//         offset: usize,
//         doc: &Doc,
//     ) {
//         batch(|| {
//             doc.preedit().preedit.set(Some(Preedit {
//                 text,
//                 cursor,
//                 offset,
//             }));

//             doc.cache_rev().update(|cache_rev| {
//                 *cache_rev += 1;
//             });
//         });
//     }

//     pub fn clear_preedit(&self, doc: &Doc) {
//         let preedit = doc.preedit();
//         if preedit.preedit.with_untracked(|preedit| preedit.is_none()) {
//             return;
//         }

//         batch(|| {
//             preedit.preedit.set(None);
//             doc.cache_rev().update(|cache_rev| {
//                 *cache_rev += 1;
//             });
//         });
//     }

//     // pub fn receive_char(&self, c: &str, doc: &Doc) {
//     //     doc.receive_char(self, c)
//     // }

//     // pub fn single_click(
//     //     &self,
//     //     pointer_event: &PointerInputEvent,
//     //     common_data: &CommonData,
//     // ) -> Option<usize> {
//     //     let mode = self.cursor.with_untracked(|c| c.mode().clone());
//     //     let (new_offset, _is_inside, cursor_affinity) =
//     //         match self.nearest_buffer_offset_of_click(&mode,
// pointer_event.pos) {     //             Ok(Some(rs)) => rs,
//     //             Ok(None) => return None,
//     //             Err(err) => {
//     //                 error!("{err:?}");
//     //                 return None;
//     //             },
//     //         };
//     //     log::info!(
//     //         "offset_of_point single_click {:?} {new_offset} {_is_inside} \
//     //          {cursor_affinity:?}",
//     //         pointer_event.pos
//     //     );
//     //     self.cursor.update(|cursor| {
//     //         cursor.set_offset_with_affinity(
//     //             new_offset,
//     //             pointer_event.modifiers.shift(),
//     //             pointer_event.modifiers.alt(),
//     //             Some(cursor_affinity),
//     //         );
//     //         cursor.affinity = cursor_affinity;
//     //     });
//     //     common_data
//     //         .internal_command
//     //         .send(InternalCommand::ResetBlinkCursor);
//     //     Some(new_offset)
//     // }

//     // pub fn double_click(&self, pointer_event: &PointerInputEvent) {}

//     // pub fn triple_click(&self, pointer_event: &PointerInputEvent) {}

//     // pub fn pointer_up(&self, _pointer_event: &PointerInputEvent) {
//     //     self.active.set(false);
//     // }

//     // pub fn center_window(&self) {
//     //     let viewport = self.viewport_untracked();
//     //     // TODO: don't assume line height is constant
//     //     let line_height = self.line_height(0) as f64;
//     //     let offset = self.cursor.with_untracked(|cursor| cursor.offset());
//     //     let (line, _col) = match self.offset_to_line_col(offset) {
//     //         Ok(rs) => rs,
//     //         Err(err) => {
//     //             error!("{err:?}");
//     //             return;
//     //         },
//     //     };

//     //     let viewport_center = viewport.height() / 2.0;

//     //     let current_line_position = line as f64 * line_height;

//     //     let desired_top =
//     //         current_line_position - viewport_center + (line_height / 2.0);

//     //     let scroll_delta = desired_top - viewport.y0;

//     //     self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
//     // }

//     // pub fn top_of_window(&self, scroll_off: usize) {
//     //     let viewport = self.viewport_untracked();
//     //     // TODO: don't assume line height is constant
//     //     let line_height = self.line_height(0) as f64;
//     //     let offset = self.cursor.with_untracked(|cursor| cursor.offset());
//     //     let (line, _col) = match self.offset_to_line_col(offset) {
//     //         Ok(rs) => rs,
//     //         Err(err) => {
//     //             error!("{err:?}");
//     //             return;
//     //         },
//     //     };

//     //     let desired_top = (line.saturating_sub(scroll_off)) as f64 *
// line_height;

//     //     let scroll_delta = desired_top - viewport.y0;

//     //     self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
//     // }

//     // pub fn bottom_of_window(&self, scroll_off: usize) {
//     //     let viewport = self.viewport_untracked();
//     //     // TODO: don't assume line height is constant
//     //     let line_height = self.line_height(0) as f64;
//     //     let offset = self.cursor.with_untracked(|cursor| cursor.offset());
//     //     let (line, _col) = match self.offset_to_line_col(offset) {
//     //         Ok(rs) => rs,
//     //         Err(err) => {
//     //             error!("{err:?}");
//     //             return;
//     //         },
//     //     };

//     //     let desired_bottom =
//     //         (line + scroll_off + 1) as f64 * line_height -
// viewport.height();

//     //     let scroll_delta = desired_bottom - viewport.y0;

//     //     self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
//     // }

//     // pub fn scroll(&self, top_shift: f64, down: bool, count: usize, mods:
//     // Modifiers) {     let viewport = self.viewport_untracked();
//     //     // TODO: don't assume line height is constant
//     //     let line_height = self.line_height(0) as f64;
//     //     let diff = line_height * count as f64;
//     //     let diff = if down { diff } else { -diff };

//     //     let offset = self.cursor.with_untracked(|cursor| cursor.offset());
//     //     let (line, _col) = match self.offset_to_line_col(offset) {
//     //         Ok(rs) => rs,
//     //         Err(err) => {
//     //             error!("{err:?}");
//     //             return;
//     //         },
//     //     };
//     //     let top = viewport.y0 + diff + top_shift;
//     //     let bottom = viewport.y0 + diff + viewport.height();

//     //     let new_line = if (line + 1) as f64 * line_height + line_height >
// bottom     // {         let line = (bottom / line_height).floor() as usize;
//     //         if line > 2 { line - 2 } else { 0 }
//     //     } else if line as f64 * line_height - line_height < top {
//     //         let line = (top / line_height).ceil() as usize;
//     //         line + 1
//     //     } else {
//     //         line
//     //     };

//     //     self.scroll_delta.set(Vec2::new(0.0, diff));

//     //     let res = match new_line.cmp(&line) {
//     //         Ordering::Greater => Some((MoveCommand::Down, new_line -
// line)),     //         Ordering::Less => Some((MoveCommand::Up, line -
// new_line)),     //         _ => None,
//     //     };

//     //     if let Some((cmd, count)) = res {
//     //         let cmd = Command::Move(cmd);
//     //         self.doc().run_command(self, &cmd, Some(count), mods);
//     //     }
//     // }

//     // === Information ===

//     // pub fn phantom_text(&self, line: usize) -> PhantomTextLine {
//     //     self.doc()
//     //         .phantom_text(self.id(), &self.es.get_untracked(), line)
//     // }

//     // pub fn line_height(&self, line: usize) -> f32 {
//     //     self.doc().line_height(line)
//     // }

//     // === Line Information ===

//     // /// Iterate over the visual lines in the view, starting at the given
// line.     // pub fn iter_vlines(
//     //     &self,
//     //     backwards: bool,
//     //     start: VLine,
//     // ) -> impl Iterator<Item = VLineInfo> + '_ {
//     //     self.lines.iter_vlines(self.text_prov(), backwards, start)
//     // }

//     // /// Iterate over the visual lines in the view, starting at the given
// line and     // ending at the /// given line. `start_line..end_line`
//     // pub fn iter_vlines_over(
//     //     &self,
//     //     backwards: bool,
//     //     start: VLine,
//     //     end: VLine,
//     // ) -> impl Iterator<Item = VLineInfo> + '_ {
//     //     self.lines
//     //         .iter_vlines_over(self.text_prov(), backwards, start, end)
//     // }

//     // /// Iterator over *relative* [`VLineInfo`]s, starting at the buffer
// line,     // `start_line`. /// The `visual_line`s provided by this will start
// at 0     // from your `start_line`. /// This is preferable over `iter_lines`
// if you     // do not need to absolute visual line value. pub fn iter_rvlines(
//     //     &self,
//     //     backwards: bool,
//     //     start: RVLine,
//     // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
//     //     self.lines
//     //         .iter_rvlines(self.text_prov().clone(), backwards, start)
//     // }

//     // /// Iterator over *relative* [`VLineInfo`]s, starting at the buffer
// line,     // `start_line` and /// ending at `end_line`.
//     // /// `start_line..end_line`
//     // /// This is preferable over `iter_lines` if you do not need to
// absolute     // visual line value. pub fn iter_rvlines_over(
//     //     &self,
//     //     backwards: bool,
//     //     start: RVLine,
//     //     end_line: usize,
//     // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
//     //     self.lines
//     //         .iter_rvlines_over(self.text_prov(), backwards, start,
// end_line)     // }

//     // ==== Position Information ====
//     //
//     // pub fn first_rvline_info(&self) -> VLineInfo<VLine> {
//     //     self.doc().lines.with_untracked(|x| x.first_vline_info())
//     // }

//     // /// The number of lines in the document.
//     // pub fn num_lines(&self) -> usize {
//     //     self.rope_text().num_lines()
//     // }

//     // /// The last allowed buffer line in the document.
//     // pub fn last_line(&self) -> usize {
//     //     self.rope_text().last_line()
//     // }

//     // pub fn last_vline(&self) -> VLine {
//     //     self.doc()
//     //         .lines
//     //         .with_untracked(|x| x.last_visual_line().into())
//     // }

//     // pub fn last_rvline(&self) -> RVLine {
//     //     self.doc()
//     //         .lines
//     //         .with_untracked(|x| x.last_visual_line().into())
//     // }

//     // pub fn last_rvline_info(&self) -> VLineInfo<()> {
//     //     self.rvline_info(self.last_rvline())
//     // }

//     // ==== Line/Column Positioning ====

//     // /// Convert an offset into the buffer into a line and idx.
//     // pub fn offset_to_line_col(&self, offset: usize) -> Result<(usize,
// usize)> {     //     self.rope_text().offset_to_line_col(offset)
//     // }

//     // pub fn offset_of_line(&self, line: usize) -> Result<usize> {
//     //     self.rope_text().offset_of_line(line)
//     // }

//     // pub fn offset_of_line_col(&self, line: usize, col: usize) ->
// Result<usize> {     //     self.rope_text().offset_of_line_col(line, col)
//     // }

//     // /// Returns the offset into the buffer of the first non blank
// character on     // /// the given line.
//     // pub fn first_non_blank_character_on_line(&self, line: usize) ->
// Result<usize>     // {
// self.rope_text().first_non_blank_character_on_line(line)     // }

//     // pub fn line_end_col(&self, line: usize, caret: bool) -> Result<usize>
// {     //     self.rope_text().line_end_col(line, caret)
//     // }

//     // pub fn select_word(&self, offset: usize) -> (usize, usize) {
//     //     self.rope_text().select_word(offset)
//     // }

//     // /// `affinity` decides whether an offset at a soft line break is
// considered     // to be on the /// previous line or the next line.
//     // /// If `affinity` is `CursorAffinity::Forward` and is at the very end
// of the     // wrapped line, then /// the offset is considered to be on the
// next line.     // pub fn vline_of_offset(
//     //     &self,
//     //     offset: usize,
//     //     affinity: CursorAffinity,
//     // ) -> Result<VLine> {
//     //     let (origin_line, offset_of_line) = self.doc.with_untracked(|x| {
//     //         let text = x.text();
//     //         let origin_line = text.line_of_offset(offset);
//     //         let origin_line_start_offset =
// text.offset_of_line(origin_line);     //         (origin_line,
// origin_line_start_offset)     //     });
//     //     let offset = offset - offset_of_line;
//     //     self.doc().lines.with_untracked(|x| {
//     //         let rs =
//     //             x.visual_line_of_origin_line_offset(origin_line, offset,
//     // affinity);         if rs.is_err() {
//     //             x.log();
//     //         }
//     //         rs.map(|x| x.0.vline)
//     //     })
//     // }

//     // pub fn vline_of_line(&self, line: usize) -> VLine {
//     //     self.lines.vline_of_line(self.text_prov(), line)
//     // }

//     // pub fn rvline_of_line(&self, line: usize) -> RVLine {
//     //     self.lines.rvline_of_line(self.text_prov(), line)
//     // }

//     // pub fn vline_of_rvline(&self, rvline: RVLine) -> Result<VLine> {
//     //     self.doc().lines.with_untracked(|x| {
//     //         x.visual_line_of_folded_line_and_sub_index(
//     //             rvline.line,
//     //             rvline.line_index,
//     //         )
//     //         .map(|x| x.into())
//     //     })
//     // }

//     // /// Get the nearest offset to the start of the visual line.
//     // pub fn offset_of_vline(&self, vline: VLine) -> usize {
//     //     self.lines.offset_of_vline(self.text_prov(), vline)
//     // }

//     // /// Get the visual line and column of the given offset.
//     // /// The column is before phantom text is applied.
//     // pub fn vline_col_of_offset(&self, offset: usize, affinity:
// CursorAffinity) ->     // (VLine, usize) {     self.lines
//     //         .vline_col_of_offset(self.text_prov(), offset, affinity)
//     // }

//     // /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移
//     // pub fn visual_line_of_offset(
//     //     &self,
//     //     offset: usize,
//     //     affinity: CursorAffinity,
//     // ) -> Result<(VLineInfo, usize, bool)> {
//     //     let (origin_line, offset_of_line) = self.doc.with_untracked(|x| {
//     //         let text = x.text();
//     //         let origin_line = text.line_of_offset(offset);
//     //         let origin_line_start_offset =
// text.offset_of_line(origin_line);     //         (origin_line,
// origin_line_start_offset)     //     });
//     //     let offset = offset - offset_of_line;
//     //     self.doc().lines.with_untracked(|x| {
//     //         x.visual_line_of_origin_line_offset(origin_line, offset,
// affinity)     //     })
//     // }

//     // /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移
//     // fn cursor_position_of_buffer_offset(
//     //     &self,
//     //     offset: usize,
//     //     affinity: CursorAffinity,
//     // ) -> Result<(
//     //     VisualLine,
//     //     usize,
//     //     usize,
//     //     bool,
//     //     // Point,
//     //     Option<Point>,
//     //     f64,
//     //     Point,
//     // )> {
//     //     self.doc()
//     //         .lines
//     //         .with_untracked(|x| x.cursor_position_of_buffer_offset(offset,
//     // affinity)) }

//     // /// return visual_line, offset_of_visual, offset_of_folded, last_char
//     // /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移，是否是最后的字符
//     // pub fn visual_line_of_offset_v2(
//     //     &self,
//     //     offset: usize,
//     //     affinity: CursorAffinity
//     // ) -> Result<(OriginFoldedLine, usize, bool)> {
//     //     self.doc().lines.with_untracked(|x| {
//     //         x.folded_line_of_offset(offset, affinity)
//     //             .map(|x| (x.0.clone(), x.1, x.2))
//     //     })
//     // }

//     // ///
// 视觉行的偏移位置，对应的上一行的偏移位置（原始文本）和是否为最后一个字符
//     // pub fn previous_visual_line(
//     //     &self,
//     //     visual_line_index: usize,
//     //     line_offset: usize,
//     //     _affinity: CursorAffinity
//     // ) -> Option<(OriginFoldedLine, usize, bool)> {
//     //     self.doc().lines.with_untracked(|x| {
//     //         x.previous_visual_line(visual_line_index, line_offset,
// _affinity)     //     })
//     // }

//     // ///
// 视觉行的偏移位置，对应的上一行的偏移位置（原始文本）和是否为最后一个字符
//     // pub fn next_visual_line(
//     //     &self,
//     //     visual_line_index: usize,
//     //     line_offset: usize,
//     //     _affinity: CursorAffinity
//     // ) -> (OriginFoldedLine, usize, bool) {
//     //     self.doc().lines.with_untracked(|x| {
//     //         x.next_visual_line(visual_line_index, line_offset, _affinity)
//     //     })
//     // }

//     // pub fn folded_line_of_offset(
//     //     &self,
//     //     offset: usize,
//     //     _affinity: CursorAffinity,
//     // ) -> OriginFoldedLine {
//     //     let line = self.visual_line_of_offset(offset,
// _affinity).0.rvline.line;     //     self.doc()
//     //         .lines
//     //         .with_untracked(|x|
// x.folded_line_of_origin_line(line).clone())     // }

//     // pub fn rvline_info_of_offset(
//     //     &self,
//     //     offset: usize,
//     //     affinity: CursorAffinity,
//     // ) -> Result<VLineInfo<VLine>> {
//     //     self.visual_line_of_offset(offset, affinity).map(|x| x.0)
//     // }

//     // /// Get the first column of the overall line of the visual line
//     // pub fn first_col<T: std::fmt::Debug>(
//     //     &self,
//     //     info: VLineInfo<T>,
//     // ) -> Result<usize> {
//     //     let line_start = info.interval.start;
//     //     let start_offset = self.text().offset_of_line(info.origin_line)?;
//     //     Ok(line_start - start_offset)
//     // }

//     // /// Get the last column in the overall line of the visual line
//     // pub fn last_col<T: std::fmt::Debug>(
//     //     &self,
//     //     info: VLineInfo<T>,
//     //     caret: bool,
//     // ) -> Result<usize> {
//     //     let vline_end = info.interval.end;
//     //     let start_offset = self.text().offset_of_line(info.origin_line)?;
//     //     // If these subtractions crash, then it is likely due to a bad
// vline     // being kept around     // somewhere
//     //     Ok(if !caret && !info.is_empty() {
//     //         let vline_pre_end =
//     //             self.rope_text().prev_grapheme_offset(vline_end, 1, 0);
//     //         vline_pre_end - start_offset
//     //     } else {
//     //         vline_end - start_offset
//     //     })
//     // }

//     // ==== Points of locations ====

//     // /// Returns the point into the text layout of the line at the given
// offset.     // /// `x` being the leading edge of the character, and `y` being
// the baseline.     // pub fn line_point_of_offset(
//     //     &self,
//     //     offset: usize,
//     //     affinity: CursorAffinity
//     // ) -> Result<Point> {
//     //     let (line, col) = self.offset_to_line_col(offset)?;
//     //     self.line_point_of_visual_line_col(line, col, affinity, false)
//     // }

//     // /// Returns the point into the text layout of the line at the given
// line and     // /// col. `x` being the leading edge of the character, and `y`
// being the     // /// baseline.
//     // pub fn line_point_of_visual_line_col(
//     //     &self,
//     //     visual_line: usize,
//     //     col: usize,
//     //     affinity: CursorAffinity,
//     //     _force_affinity: bool
//     // ) -> Result<Point> {
//     //     self.doc().lines.with_untracked(|x| {
//     //         x.line_point_of_visual_line_col(
//     //             visual_line,
//     //             col,
//     //             affinity,
//     //             _force_affinity
//     //         )
//     //     })
//     // }

//     // /// Get the (point above, point below) of a particular offset within
// the     // /// editor.
//     // pub fn points_of_offset(&self, offset: usize) -> Result<(Point,
// Point)> {     //     let Some((point_above, line_height)) =
//     //         self.screen_lines.with_untracked(|screen_lines| {
//     //             match
// screen_lines.visual_position_of_buffer_offset(offset) {     //
// Ok(point) => {     //                     point.map(|point| (point,
// screen_lines.line_height))     //                 },
//     //                 Err(err) => {
//     //                     error!("{}", err.to_string());
//     //                     None
//     //                 },
//     //             }
//     //         })
//     //     else {
//     //         // log::info!("points_of_offset point is none {offset}");
//     //         return Ok((Point::new(0.0, 0.0), Point::new(0.0, 0.0)));
//     //     };
//     //     let mut point_below = point_above;
//     //     point_below.y += line_height;
//     //     Ok((point_above, point_below))
//     // }

//     // /// Get the offset of a particular point within the editor.
//     // /// The boolean indicates whether the point is inside the text or not
//     // /// Points outside of vertical bounds will return the last line.
//     // /// Points outside of horizontal bounds will return the last column on
// the     // /// line.
//     // pub fn offset_of_point(
//     //     &self,
//     //     mode: &CursorMode,
//     //     mut point: Point,
//     // ) -> Result<Option<(usize, bool, CursorAffinity)>> {
//     //     let viewport = self.viewport_untracked();
//     //     // point.x += viewport.x0;
//     //     point.y -= viewport.y0;
//     //     // log::info!("offset_of_point point={point:?},
// viewport={viewport:?} ");     //     self.screen_lines
//     //         .with_untracked(|x| x.buffer_offset_of_click(mode, point))
//     // }

//     // /// 获取该坐标所在的视觉行和行偏离
//     // pub fn line_col_of_point_with_phantom(
//     //     &self,
//     //     point: Point,
//     // ) -> (usize, usize, TextLayoutLine) {
//     //     let line_height = f64::from(self.doc().line_height(0));
//     //     let y = point.y.max(0.0);
//     //     let visual_line = (y / line_height) as usize;
//     //     let text_layout = self.text_layout_of_visual_line(visual_line);
//     //     let hit_point = text_layout.text.hit_point(Point::new(point.x,
// y));     //     (visual_line, hit_point.index, text_layout)
//     // }

//     // /// Get the (line, col) of a particular point within the editor.
//     // /// The boolean indicates whether the point is within the text bounds.
//     // /// Points outside of vertical bounds will return the last line.
//     // /// Points outside of horizontal bounds will return the last column on
// the     // line. pub fn line_col_of_point(
//     //     &self,
//     //     _mode: &CursorMode,
//     //     point: Point,
//     //     _tracing: bool,
//     // ) -> ((usize, usize), bool) {
//     //     // TODO: this assumes that line height is constant!
//     //     let line_height = f64::from(self.doc().line_height(0));
//     //     let info = if point.y <= 0.0 {
//     //         self.first_rvline_info()
//     //     } else {
//     //         self.doc().lines.with_untracked(|sl| {
//     //             let sl = &sl.screen_lines();
//     //             if let Some(info) = sl.iter_line_info().find(|info| {
//     //                 info.vline_y <= point.y && info.vline_y + line_height
// >=     // point.y             }) {
//     //                 info.vline_info
//     //             } else {
//     //                 if sl.lines.last().is_none() {
//     //                     panic!("point: {point:?} {:?} {:?}", sl.lines,
// sl.info);     //                 }
//     //                 let info = sl.info(*sl.lines.last().unwrap());
//     //                 if info.is_none() {
//     //                     panic!("point: {point:?} {:?} {:?}", sl.lines,
// sl.info);     //                 }
//     //                 info.unwrap().vline_info
//     //             }
//     //         })
//     //     };
//     //
//     //     let rvline = info.rvline;
//     //     let line = rvline.line;
//     //     let text_layout = self.text_layout_of_visual_line(line);
//     //
//     //     let y =
// text_layout.get_layout_y(rvline.line_index).unwrap_or(0.0);     //
//     //     let hit_point = text_layout.text.hit_point(Point::new(point.x, y
// as     // f64));     // We have to unapply the phantom text shifting in order
// to     // get back to the column in     // the actual buffer
//     //     let (line, col, _) = text_layout
//     //         .phantom_text
//     //         .cursor_position_of_final_col(hit_point.index);
//     //
//     //     ((line, col), hit_point.is_inside)
//     // }

//     // pub fn line_horiz_col(
//     //     &self,
//     //     line: usize,
//     //     horiz: &ColPosition,
//     //     caret: bool, visual_line: &VisualLine,
//     // ) -> usize {
//     //     match *horiz {
//     //         ColPosition::Col(x) => {
//     //             // TODO: won't this be incorrect with phantom text?
// Shouldn't     // this just use             // line_col_of_point and get the
// col from that?     //             let text_layout =
// self.text_layout_of_visual_line(line);     //             let hit_point =
// text_layout.text.hit_point(Point::new(x, 0.0));     //             let n =
// hit_point.index;     //
// text_layout.phantom_text.origin_position_of_final_col(n)     //         }
//     //         ColPosition::End => (line, self.line_end_col(line, caret)),
//     //         ColPosition::Start => (line, 0),
//     //         ColPosition::FirstNonBlank => {
//     //             (line, self.first_non_blank_character_on_line(line))
//     //         }
//     //     }
//     // }

//     // /// Advance to the right in the manner of the given mode.
//     // /// Get the column from a horizontal at a specific line index (in a
// text     // layout) pub fn rvline_horiz_col(
//     //     &self,
//     //     // RVLine { line, line_index }: RVLine,
//     //     horiz: &ColPosition,
//     //     _caret: bool,
//     //     visual_line: &VisualLine,
//     // ) -> usize {
//     //     match *horiz {
//     //         ColPosition::Col(x) => {
//     //             let text_layout = &visual_line.text_layout;
//     //             let y_pos = text_layout
//     //                 .text
//     //                 .layout_runs()
//     //                 .nth(visual_line.origin_folded_line_sub_index)
//     //                 .map(|run| run.line_y)
//     //                 .or_else(|| {
//     //                     text_layout.text.layout_runs().last().map(|run|
//     // run.line_y)                 })
//     //                 .unwrap_or(0.0);
//     //             let hit_point =
//     //                 text_layout.text.hit_point(Point::new(x, y_pos as
// f64));     //             let n = hit_point.index;
//     //             let rs =
//     // text_layout.phantom_text.cursor_position_of_final_col(n);
//     // rs.2 + rs.1         }
//     //         ColPosition::End => visual_line.origin_interval.end,
//     //         ColPosition::Start => visual_line.origin_interval.start,
//     //         ColPosition::FirstNonBlank => {
//     //             let final_offset =
// visual_line.text_layout.text.line().text()     //
// [visual_line.visual_interval.start     //
// ..visual_line.visual_interval.end]     //                 .char_indices()
//     //                 .find(|(_, c)| !c.is_whitespace())
//     //                 .map(|(idx, _)| visual_line.visual_interval.start +
// idx)     //                 .unwrap_or(visual_line.visual_interval.end);
//     //             let rs = visual_line
//     //                 .text_layout
//     //                 .phantom_text
//     //                 .cursor_position_of_final_col(final_offset);
//     //             rs.2 + rs.1
//     //         }
//     //     }
//     // }

//     // /// Advance to the right in the manner of the given mode.
//     // /// This is not the same as the [`Movement::Right`] command.
//     // pub fn move_right(
//     //     &self,
//     //     offset: usize,
//     //     mode: Mode,
//     //     count: usize,
//     // ) -> Result<usize> {
//     //     self.rope_text().move_right(offset, mode, count)
//     // }
// }

// impl std::fmt::Debug for Editor {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_tuple("Editor").field(&self.id).finish()
//     }
// }

// /// (x, y, line_height, width)
// pub fn cursor_caret_v2(
//     offset: usize,
//     affinity: CursorAffinity,
//     lines: DocLinesManager, point: Option<Point>
// ) -> Option<(f64, f64, f64, f64)> {
//     let (
//         _offset_folded,
//         _after_last_char,
//         point,
//         // screen,
//         line_height,
//         _origin_point,
//         _
//     ) = match lines
//         .with_untracked(|x| x.visual_position_of_cursor_position(offset,
// affinity))     {
//         Ok(rs) => rs?,
//         Err(err) => {
//             error!("{err:?}");
//             return None;
//         }
//     };
//
//     Some((point.x - 1.0, point.y, 2.0, line_height))
// }

pub fn do_motion_mode(
    action: &dyn CommonAction,
    cursor: &mut Cursor,
    motion_mode: MotionMode,
    register: &mut Register,
) {
    if let Some(cached_motion_mode) = cursor.motion_mode.take() {
        // If it's the same MotionMode discriminant, continue, count is cached in the
        // old motion_mode.
        if core::mem::discriminant(&cached_motion_mode)
            == core::mem::discriminant(&motion_mode)
        {
            let offset = cursor.offset();
            action.exec_motion_mode(
                cursor,
                cached_motion_mode,
                offset..offset,
                true,
                register,
            );
        }
    } else {
        cursor.motion_mode = Some(motion_mode);
    }
}

/// Trait for common actions needed for the default implementation of the
/// operations.
pub trait CommonAction {
    // TODO: should this use Rope's Interval instead of Range?
    fn exec_motion_mode(
        &self,
        cursor: &mut Cursor,
        motion_mode: MotionMode,
        range: Range<usize>,
        is_vertical: bool,
        register: &mut Register,
    );

    // TODO: should we have a more general cursor state structure?
    // since modal is about cursor, and register is sortof about cursor
    // but also there might be other state it wants. Should we just pass Editor to
    // it?
    /// Perform an edit.
    /// Returns `true` if there was any change.
    fn do_edit(
        &self,
        cursor: &mut Cursor,
        cmd: &EditCommand,
        modal: bool,
        register: &mut Register,
        smart_tab: bool,
        screen_lines: &ScreenLines,
    ) -> bool;
}

pub fn get_selection(
    cursor: &Cursor,
) -> Vec<(usize, usize, Option<CursorAffinity>, Option<CursorAffinity>)> {
    match cursor.mode() {
        CursorMode::Normal(_) => {
            vec![]
        },
        CursorMode::Visual {
            start: _start,
            end: _end,
            mode: VisualMode::Normal,
        } => {
            error!("todo implement");
            vec![]
            // let start_offset = start.min(end);
            // let end_offset = match ed.move_right(*start.max(end),
            // Mode::Insert, 1) {     Ok(rs) => rs,
            //     Err(err) => {
            //         error!("{err:?}");
            //         return;
            //     }
            // };
            //
            // if let Err(err) = paint_normal_selection(
            //     cx,
            //     selection_color,
            //     *start_offset,
            //     end_offset,
            //     _screen_lines, cursor.affinity
            // ) {
            //     error!("{err:?}");
            // }
        },
        CursorMode::Visual {
            start: _start,
            end: _end,
            mode: VisualMode::Linewise,
        } => {
            error!("todo implement paint_linewise_selection");
            vec![]
            // if let Err(err) = paint_linewise_selection(
            //     cx,
            //     ed,
            //     selection_color,
            //     screen_lines,
            //     *start.min(end),
            //     *start.max(end),
            //     cursor.affinity,
            // ) {
            //     error!("{err:?}");
            // }
        },
        CursorMode::Visual {
            start: _start,
            end: _end,
            mode: VisualMode::Blockwise,
        } => {
            error!("todo implement paint_blockwise_selection");
            vec![]
            // if let Err(err) = paint_blockwise_selection(
            //     cx,
            //     ed,
            //     selection_color,
            //     screen_lines,
            //     *start.min(end),
            //     *start.max(end),
            //     cursor.affinity,
            //     cursor.horiz,
            // ) {
            //     error!("{err:?}");
            // }
        },
        CursorMode::Insert(_) => cursor
            .regions_iter()
            .filter(|(start, end, ..)| start != end)
            .map(|(start, end, start_affinity, end_affinity)| {
                if start > end {
                    (end, start, end_affinity, start_affinity)
                } else {
                    (start, end, start_affinity, end_affinity)
                }
            })
            .collect(),
    }
}

// pub fn paint_selection(cx: &mut PaintCx, ed: &Editor, _screen_lines:
// &ScreenLines) {     let cursor = ed.cursor;

//     let selection_color = ed.doc().lines.with_untracked(|es|
// es.selection_color());

//     cursor.with_untracked(|cursor| match cursor.mode() {rope_text
//         CursorMode::Normal(_) => {},
//         CursorMode::Visual {
//             start: _start,
//             end: _end,
//             mode: VisualMode::Normal,
//         } => {
//             error!("todo implement");
//             // let start_offset = start.min(end);
//             // let end_offset = match ed.move_right(*start.max(end),
//             // Mode::Insert, 1) {     Ok(rs) => rs,
//             //     Err(err) => {
//             //         error!("{err:?}");
//             //         return;
//             //     }
//             // };
//             //
//             // if let Err(err) = paint_normal_selection(
//             //     cx,
//             //     selection_color,
//             //     *start_offset,
//             //     end_offset,
//             //     _screen_lines, cursor.affinity
//             // ) {
//             //     error!("{err:?}");
//             // }
//         },
//         CursorMode::Visual {
//             start: _start,
//             end: _end,
//             mode: VisualMode::Linewise,
//         } => {
//             error!("todo implement paint_linewise_selection");
//             // if let Err(err) = paint_linewise_selection(
//             //     cx,
//             //     ed,
//             //     selection_color,
//             //     screen_lines,
//             //     *start.min(end),
//             //     *start.max(end),
//             //     cursor.affinity,
//             // ) {
//             //     error!("{err:?}");
//             // }
//         },
//         CursorMode::Visual {
//             start: _start,
//             end: _end,
//             mode: VisualMode::Blockwise,
//         } => {
//             error!("todo implement paint_blockwise_selection");
//             // if let Err(err) = paint_blockwise_selection(
//             //     cx,
//             //     ed,
//             //     selection_color,
//             //     screen_lines,
//             //     *start.min(end),
//             //     *start.max(end),
//             //     cursor.affinity,
//             //     cursor.horiz,
//             // ) {
//             //     error!("{err:?}");
//             // }
//         },
//         CursorMode::Insert(_) => {
//             for (start, end, start_affinity, end_affinity) in cursor
//                 .regions_iter()
//                 .filter(|(start, end, ..)| start != end)
//             {
//                 let (start, end, start_affinity, end_affinity) = if start >
// end {                     (end, start, end_affinity, start_affinity)
//                 } else {
//                     (start, end, start_affinity, end_affinity)
//                 };
//                 // log::info!("start={start} end={end}
//                 // start_affinity={start_affinity:?}");
//                 if let Err(err) = paint_normal_selection(
//                     cx,
//                     selection_color,
//                     start,
//                     end,
//                     _screen_lines,
//                     start_affinity,
//                     end_affinity,
//                 ) {
//                     error!("{err:?}");
//                 }
//             }
//         },
//     });
// }

//
// #[allow(clippy::too_many_arguments)]
// pub fn paint_blockwise_selection(
//     cx: &mut PaintCx,
//     ed: &Editor,
//     color: Color,
//     screen_lines: &ScreenLines,
//     start_offset: usize,
//     end_offset: usize,
//     affinity: CursorAffinity,
//     horiz: Option<ColPosition>,
// ) -> Result<()> {
//     error!("todo replace paint_blockwise_selection
// start_offset={start_offset} end_offset={end_offset}");     let (start_rvline,
// start_col, _) =         ed.visual_line_of_offset(start_offset, affinity)?;
//     let (end_rvline, end_col, _) = ed.visual_line_of_offset(end_offset,
// affinity)?;     let start_rvline = start_rvline.rvline;
//     let end_rvline = end_rvline.rvline;
//     let left_col = start_col.min(end_col);
//     let right_col = start_col.max(end_col) + 1;
//
//     let lines = screen_lines
//         .iter_line_info_r(start_rvline..=end_rvline)
//         .filter_map(|line_info| {
//             let max_col = ed.last_col(line_info.vline_info, true);
//             (max_col > left_col).then_some((line_info, max_col))
//         });
//
//     for (line_info, max_col) in lines {
//         let line = line_info.vline_info.origin_line;
//         let right_col = if let Some(ColPosition::End) = horiz {
//             max_col
//         } else {
//             right_col.min(max_col)
//         };
//
//         // TODO: what affinity to use?
//         let x0 = ed
//             .line_point_of_visual_line_col(
//                 line,
//                 left_col,
//                 CursorAffinity::Forward,
//                 true,
//             )
//             .x;
//         let x1 = ed
//             .line_point_of_visual_line_col(
//                 line,
//                 right_col,
//                 CursorAffinity::Backward,
//                 true,
//             )
//             .x;
//
//         let line_height = ed.line_height(line);
//         let rect = Rect::from_origin_size(
//             (x0, line_info.vline_y),
//             (x1 - x0, f64::from(line_height)),
//         );
//         cx.fill(&rect, color, 0.0);
//     }
//     Ok(())
// }

// fn paint_cursor(
//     cx: &mut PaintCx,
//     ed: &Editor,
//     screen_lines: &ScreenLines,
// ) -> Result<()> {
//     let cursor = ed.cursor;
//
//     let viewport = ed.viewport();
//
//     let current_line_color =
//         ed.doc().lines.with_untracked(|es| es.current_line_color());
//
//     let cursor = cursor.get_untracked();
//     let highlight_current_line = match cursor.mode() {
//         // TODO: check if shis should be 0 or 1
//         CursorMode::Normal(size) => *size == 0,
//         CursorMode::Insert(ref sel) => sel.is_caret(),
//         CursorMode::Visual { .. } => false,
//     };
//
//     if let Some(current_line_color) = current_line_color {
//         // Highlight the current line
//         if highlight_current_line {
//             for (_, end) in cursor.regions_iter() {
//                 // TODO: unsure if this is correct for wrapping lines
//                 let rvline = ed.visual_line_of_offset(end, cursor.affinity)?;
//
//                 if let Some(info) = screen_lines.info(rvline.0.rvline) {
//                     let line_height =
// ed.line_height(info.vline_info.origin_line);                     let rect =
// Rect::from_origin_size(                         (viewport.x0, info.vline_y),
//                         (viewport.width(), f64::from(line_height)),
//                     );
//
//                     cx.fill(&rect, current_line_color, 0.0);
//                 }
//             }
//         }
//     }
//
//     paint_selection(cx, ed, screen_lines);
//     Ok(())
// }

#[allow(clippy::too_many_arguments)]
fn paint_normal_selection(
    cx: &mut PaintCx,
    color: Color,
    start_offset: usize,
    end_offset: usize,
    screen_lines: &ScreenLines,
    start_affinity: Option<CursorAffinity>,
    end_affinity: Option<CursorAffinity>,
) -> Result<()> {
    let rs = screen_lines.normal_selection(
        start_offset,
        end_offset,
        start_affinity,
        end_affinity,
    )?;
    // log::info!(
    //     "normal_selection {start_offset}-{end_offset} \
    //      {start_affinity:?}-{end_affinity:?} {rs:?}"
    // );
    for rect in rs {
        cx.fill(&rect, color, 0.0);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub fn paint_text(
    cx: &mut PaintCx,
    viewport: Rect,
    is_active: bool,
    hide_cursor: bool,
    screen_lines: &ScreenLines,
    lines: DocLinesManager,
    font_family: Cow<[FamilyOwned]>,
    visible_whitespace: Color,
    font_size: f32,
    cursor_points: Vec<Point>,
    line_height: f64,
    dim_color: Color,
    diff_color: Color,
    selections: Vec<(usize, usize, Option<CursorAffinity>, Option<CursorAffinity>)>,
    select_color: Color,
    cursor_offset: usize,
    editor: &EditorData,
) -> Result<()> {
    {
        let mut visual_lines = screen_lines.visual_lines.iter().peekable();
        while let Some(line_info) = visual_lines.next() {
            let y = line_info.paint_point(screen_lines.base).y;
            match line_info {
                VisualLineInfo::OriginText {
                    text: line_info, ..
                } => {
                    if line_info.is_diff {
                        cx.fill(
                            &Rect::ZERO
                                .with_size(Size::new(viewport.width(), line_height))
                                .with_origin(Point::new(viewport.x0, y)),
                            diff_color.multiply_alpha(0.2),
                            0.0,
                        );
                    }
                    paint_extra_style(
                        cx,
                        line_info.folded_line.extra_style(),
                        y,
                        viewport,
                    );
                    paint_document_highlight_style(
                        cx,
                        line_info.folded_line.document_highlight_style(),
                        y,
                        viewport,
                    );
                },
                VisualLineInfo::DiffDelete { .. } => {
                    let mut count = 1.0f64;
                    while let Some(VisualLineInfo::DiffDelete { .. }) =
                        visual_lines.peek()
                    {
                        count += 1.0;
                        visual_lines.next();
                    }
                    paint_diff_no_code(
                        cx,
                        viewport,
                        y,
                        dim_color,
                        count * line_height,
                    );
                },
            }
        }
    }
    {
        for (start_offset, end_offset, start_affinity, end_affinity) in selections {
            // log::warn!(
            //     "start {start_offset} {start_affinity:?} end {end_offset} \
            //      {end_affinity:?}"
            // );
            paint_normal_selection(
                cx,
                select_color,
                start_offset,
                end_offset,
                screen_lines,
                start_affinity,
                end_affinity,
            )?;
        }
    }
    let visual_lines = screen_lines.visual_lines.iter();
    for line_info in visual_lines {
        let y = line_info.paint_point(screen_lines.base).y;
        if let VisualLineInfo::OriginText {
            text: line_info, ..
        } = line_info
        {
            if let Some(whitespaces) = &line_info.folded_line.whitespaces() {
                let attrs = Attrs::new()
                    .color(visible_whitespace)
                    .family(&font_family)
                    .font_size(font_size);
                let attrs_list = AttrsList::new(attrs);
                let space_text = TextLayout::new_with_text("·", attrs_list.clone());
                let tab_text = TextLayout::new_with_text("→", attrs_list);

                for (c, (x0, _x1)) in whitespaces.iter() {
                    match *c {
                        '\t' => {
                            cx.draw_text_with_layout(
                                tab_text.layout_runs(),
                                Point::new(*x0, y),
                            );
                        },
                        ' ' => {
                            cx.draw_text_with_layout(
                                space_text.layout_runs(),
                                Point::new(*x0, y),
                            );
                        },
                        _ => {},
                    }
                }
            }
            cx.draw_text_with_layout(
                line_info.folded_line.borrow_text().layout_runs(),
                Point::new(0.0, y),
            );

            if line_info
                .folded_line
                .origin_interval
                .contains(cursor_offset)
            {
                let line_w = line_info
                    .folded_line
                    .borrow_text()
                    .layout_runs()
                    .fold(0.0, |sum, line| sum + line.line_w)
                    as f64;
                if line_w < viewport.x1
                    && let Some(text) = editor
                        .find_most_serious_diag_by_offset_for_paint(cursor_offset)
                {
                    cx.draw_text_with_layout(
                        text.layout_runs(),
                        Point::new(line_w + 20.0, y),
                    );
                }
            }

            // for line in line_info.folded_line.borrow_text().layout_runs() {
            //     log::warn!(
            //         "line_index={} line_w={}",
            //         line_info.folded_line.line_index,
            //         line.line_w,
            //     );
            // }
        }
    }
    if is_active && !hide_cursor {
        paint_cursor_caret(cx, lines, cursor_points, line_height);
    }
    Ok(())
}

fn paint_diff_no_code(
    cx: &mut PaintCx,
    viewport: Rect,
    y: f64,
    color: Color,
    section_height: f64,
) {
    let y_end = y + section_height;

    if y_end < viewport.y0 || y > viewport.y1 {
        return;
    }

    let y = y.max(viewport.y0 - 10.0);
    let y_end = y_end.min(viewport.y1 + 10.0);
    let height = y_end - y;

    let start_x = viewport.x0.floor() as usize;
    let start_x = start_x - start_x % 8;

    for x in (start_x
        ..viewport.x1.ceil() as usize + 1 + section_height.ceil() as usize)
        .step_by(8)
    {
        let p0 = if x as f64 > viewport.x1.ceil() {
            Point::new(viewport.x1.ceil(), y + (x as f64 - viewport.x1.ceil()))
        } else {
            Point::new(x as f64, y)
        };

        let height = if x as f64 - height < viewport.x0.floor() {
            x as f64 - viewport.x0.floor()
        } else {
            height
        };
        if height > 0.0 {
            let p1 = Point::new(x as f64 - height, y + height);
            cx.stroke(&Line::new(p0, p1), color, &Stroke::new(1.0));
        }
    }
}

pub fn paint_document_highlight_style(
    cx: &mut PaintCx,
    extra_styles: &[LineExtraStyle],
    y: f64,
    viewport: Rect,
) {
    for style in extra_styles {
        let height = style.height;
        if let Some(bg) = style.bg_color {
            let width = style.width.unwrap_or_else(|| viewport.width());
            let base = if style.width.is_none() {
                viewport.x0
            } else {
                0.0
            };
            let x = style.x + base;
            cx.fill(
                &Rect::ZERO
                    .with_size(Size::new(width, height))
                    .with_origin(Point::new(x, y))
                    .to_rounded_rect(2.0),
                bg,
                0.0,
            );
        }
    }
}

pub fn paint_extra_style(
    cx: &mut PaintCx,
    extra_styles: &[LineExtraStyle],
    y: f64,
    viewport: Rect,
) {
    for style in extra_styles {
        let height = style.height - 2.0;
        if let Some(bg) = style.bg_color {
            let width = style.width.unwrap_or_else(|| viewport.width());
            let base = if style.width.is_none() {
                viewport.x0
            } else {
                0.0
            };
            let x = style.x + base;
            let y = y + style.y + 1.0;

            cx.fill(
                &Rect::ZERO
                    .with_size(Size::new(width, height))
                    .with_origin(Point::new(x, y))
                    .to_rounded_rect(2.0),
                bg,
                0.0,
            );
        }

        if let Some(color) = style.under_line {
            let width = style.width.unwrap_or_else(|| viewport.width());
            let base = if style.width.is_none() {
                viewport.x0
            } else {
                0.0
            };
            let x = style.x + base;
            let y = y + style.y + height;
            cx.stroke(
                &Line::new(Point::new(x, y), Point::new(x + width, y)),
                color,
                &Stroke::new(1.0),
            );
        }

        if let Some(color) = style.wave_line {
            let width = style.width.unwrap_or_else(|| viewport.width());
            let y = y + style.y + height;
            paint_wave_line(cx, width, Point::new(style.x, y), color);
        }
    }
}

pub fn paint_wave_line(cx: &mut PaintCx, width: f64, point: Point, color: Color) {
    let radius = 2.0;
    let origin = Point::new(point.x, point.y + radius);
    let mut path = BezPath::new();
    path.move_to(origin);

    let mut x = 0.0;
    let mut direction = -1.0;
    while x < width {
        let point = origin + (x, 0.0);
        let p1 = point + (radius, -radius * direction);
        let p2 = point + (radius * 2.0, 0.0);
        path.quad_to(p1, p2);
        x += radius * 2.0;
        direction *= -1.0;
    }

    cx.stroke(&path, color, &peniko::kurbo::Stroke::new(1.));
}

fn paint_cursor_caret(
    cx: &mut PaintCx,
    lines: DocLinesManager,
    cursor_points: Vec<Point>,
    line_height: f64,
) {
    let caret_color = lines.with_untracked(|es| es.ed_caret());
    cursor_points.into_iter().for_each(|point| {
        let (x, y, width, line_height) =
            (point.x - 1.0, point.y + 1.0, 2.0, line_height - 2.0);
        let rect = Rect::from_origin_size((x, y), (width, line_height));
        cx.fill(&rect, &caret_color, 0.0);
    });
    // cursor.with_untracked(|cursor| {
    //     for (_, end) in cursor.regions_iter() {
    //         if let Some() =
    //             cursor_caret_v2(end, cursor.affinity, lines)
    //         {
    //
    //         }
    //     }
    // });
}
//
// #[allow(clippy::too_many_arguments)]
// pub fn paint_linewise_selection(
//     cx: &mut PaintCx,
//     ed: &Editor,
//     color: Color,
//     screen_lines: &ScreenLines,
//     start_offset: usize,
//     end_offset: usize,
//     affinity: CursorAffinity,
// ) -> Result<()> {
//     let viewport = ed.viewport();
//     error!("todo replace paint_linewise_selection start_offset={start_offset}
// end_offset={end_offset} affinity={affinity:?}");     let (start_rvline, _, _)
// = ed.visual_line_of_offset(start_offset, affinity)?;     let (end_rvline, _,
// _) = ed.visual_line_of_offset(end_offset, affinity)?;     let start_rvline =
// start_rvline.rvline;     let end_rvline = end_rvline.rvline;
//     // Linewise selection is by *line* so we move to the start/end rvlines of
// the line     let start_rvline = screen_lines
//         .first_rvline_for_line(start_rvline.line)
//         .unwrap_or(start_rvline);
//     let end_rvline = screen_lines
//         .last_rvline_for_line(end_rvline.line)
//         .unwrap_or(end_rvline);
//
//     for LineInfo {
//         vline_info: info,
//         vline_y,
//         ..
//     } in screen_lines.iter_line_info_r(start_rvline..=end_rvline)
//     {
//         let line = info.origin_line;
//
//         // The left column is always 0 for linewise selections.
//         let right_col = ed.last_col(info, true);
//
//         // TODO: what affinity to use?
//         let x1 =
//             ed.line_point_of_visual_line_col(
//                 line,
//                 right_col,
//                 CursorAffinity::Backward,
//                 true,
//             )
//             .x + CHAR_WIDTH;
//
//         let line_height = ed.line_height(line);
//         let rect = Rect::from_origin_size(
//             (viewport.x0, vline_y),
//             (x1 - viewport.x0, f64::from(line_height)),
//         );
//         cx.fill(&rect, color, 0.0);
//     }
//     Ok(())
// }

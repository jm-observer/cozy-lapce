use doc::lines::line::{OriginFoldedLine, VisualLine};
use doc::lines::screen_lines::{ScreenLines, VisualLineInfo};
use std::ops::Range;
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use lapce_xi_rope::Rope;

use doc::lines::{
    buffer::rope_text::{RopeText, RopeTextVal},
    cursor::{ColPosition, Cursor, CursorAffinity, CursorMode},
};
use floem::views::editor::core::{
    command::MoveCommand, mode::Mode, movement::Movement, register::Register,
};

use crate::doc::Doc;
use anyhow::Result;
use doc::lines::layout::{LineExtraStyle, TextLayoutLine};
use doc::lines::phantom_text::PhantomTextMultiLine;
use doc::lines::{
    selection::{InsertDrift, Selection},
    word::{get_char_property, CharClassification, WordCursor},
};
use floem::context::PaintCx;
use floem::kurbo::{Line, Size, Stroke};
use floem::reactive::{SignalGet, SignalTrack, SignalUpdate, SignalWith, Trigger};
use floem::text::FONT_SYSTEM;
use floem::views::editor::command::Command;
use floem::views::editor::core::command::MultiSelectionCommand::{
    InsertCursorAbove, InsertCursorBelow, InsertCursorEndOfLine, SelectAll,
    SelectAllCurrent, SelectCurrentLine, SelectNextCurrent, SelectSkipCurrent,
    SelectUndo,
};
use floem::views::editor::core::command::{EditCommand, MultiSelectionCommand};
use floem::views::editor::core::mode::{MotionMode, VisualMode};
use floem::views::editor::core::selection::SelRegion;
use floem::views::editor::id::EditorId;
use floem::views::editor::movement::{move_offset, move_selection};
use floem::views::editor::text::{
    Document, Preedit, PreeditData, Styling, WrapMethod,
};
use floem::views::editor::view::{
    EditorView, LineInfo, LineRegion, ScreenLinesBase,
};
use floem::views::editor::visual_line::{
    hit_position_aff, ConfigId, FontSizeCacheId, RVLine, ResolvedWrap,
    TextLayoutProvider, VLine, VLineInfo,
};
use floem::views::editor::EditorStyle;
use floem::{
    action::{exec_after, TimerToken},
    keyboard::Modifiers,
    kurbo,
    kurbo::{Point, Rect, Vec2},
    peniko::Color,
    pointer::{PointerButton, PointerInputEvent, PointerMoveEvent},
    reactive::{batch, ReadSignal, RwSignal, Scope},
    text::{Attrs, AttrsList, LineHeightValue, TextLayout, Wrap},
    Renderer, ViewId,
};
use log::{error, info, warn};

// pub(crate) const CHAR_WIDTH: f64 = 7.5;

/// The main structure for the editor view itself.  
/// This can be considered to be the data part of the `View`.
/// It holds an `Rc<Doc>` within as the document it is a view into.  
#[derive(Clone)]
pub struct Editor {
    pub cx: Cell<Scope>,
    effects_cx: Cell<Scope>,

    id: EditorId,

    pub active: RwSignal<bool>,

    /// Whether you can edit within this editor.
    pub read_only: RwSignal<bool>,

    pub(crate) doc: RwSignal<Rc<Doc>>,

    pub cursor: RwSignal<Cursor>,

    pub window_origin: RwSignal<Point>,
    // pub viewport: RwSignal<Rect>,
    pub parent_size: RwSignal<Rect>,

    pub editor_view_focused: Trigger,
    pub editor_view_focus_lost: Trigger,
    pub editor_view_id: RwSignal<Option<ViewId>>,

    /// The current scroll position.
    pub scroll_delta: RwSignal<Vec2>,
    pub scroll_to: RwSignal<Option<Vec2>>,

    /// Holds the cache of the lines and provides many utility functions for them.
    // lines: RwSignal<DocLines>,
    // pub screen_lines: RwSignal<ScreenLines>,

    /// Modal mode register
    pub register: RwSignal<Register>,
    /// Cursor rendering information, such as the cursor blinking state.
    pub cursor_info: CursorInfo,

    pub last_movement: RwSignal<Movement>,

    /// Whether ime input is allowed.  
    /// Should not be set manually outside of the specific handling for ime.
    pub ime_allowed: RwSignal<bool>,

    // /// The Editor Style
    // pub es: RwSignal<EditorStyle>,
    pub floem_style_id: RwSignal<u64>,
    // pub lines: DocLinesManager,
}
impl Editor {
    /// Create a new editor into the given document, using the styling.  
    /// `doc`: The backing [`Document`], such as [TextDocument](self::text_document::TextDocument)
    /// `style`: How the editor should be styled, such as [SimpleStyling](self::text::SimpleStyling)
    // pub fn new(cx: Scope, doc: Rc<Doc>, style: Rc<dyn Styling>, modal: bool) -> Editor {
    //     let id = doc.editor_id();
    //     Editor::new_id(cx, id, doc, style, modal)
    // }

    /// Create a new editor into the given document, using the styling.  
    /// `id` should typically be constructed by [`EditorId::next`]  
    /// `doc`: The backing [`Document`], such as [TextDocument](self::text_document::TextDocument)
    /// `style`: How the editor should be styled, such as [SimpleStyling](self::text::SimpleStyling)
    pub fn new(cx: Scope, doc: Rc<Doc>, modal: bool) -> Editor {
        let editor = Editor::new_direct(cx, doc, modal);
        editor.recreate_view_effects();

        editor
    }

    // TODO: shouldn't this accept an `RwSignal<Rc<Doc>>` so that it can listen for
    // changes in other editors?
    // TODO: should we really allow callers to arbitrarily specify the Id? That could open up
    // confusing behavior.

    /// Create a new editor into the given document, using the styling.  
    /// `id` should typically be constructed by [`EditorId::next`]  
    /// `doc`: The backing [`Document`], such as [TextDocument](self::text_document::TextDocument)
    /// `style`: How the editor should be styled, such as [SimpleStyling](self::text::SimpleStyling)
    /// This does *not* create the view effects. Use this if you're creating an editor and then
    /// replacing signals. Invoke [`Editor::recreate_view_effects`] when you are done.
    /// ```rust,ignore
    /// let shared_scroll_beyond_last_line = /* ... */;
    /// let editor = Editor::new_direct(cx, id, doc, style);
    /// editor.scroll_beyond_last_line.set(shared_scroll_beyond_last_line);
    /// ```
    pub fn new_direct(cx: Scope, doc: Rc<Doc>, modal: bool) -> Editor {
        let id = doc.editor_id();
        // let viewport = doc.viewport();
        let cx = cx.create_child();

        let cursor_mode = if modal {
            CursorMode::Normal(0)
        } else {
            CursorMode::Insert(Selection::caret(0))
        };
        let cursor = Cursor::new(cursor_mode, None, None);
        let cursor = cx.create_rw_signal(cursor);
        // let lines = doc.doc_lines;
        let doc = cx.create_rw_signal(doc);
        // let font_sizes = Rc::new(EditorFontSizes {
        //     id,
        //     style: style.read_only(),
        //     doc: doc.read_only(),
        // });
        // let lines = Rc::new(Lines::new(cx, font_sizes));

        // let screen_lines =
        //     cx.create_rw_signal(ScreenLines::new(cx, viewport.get_untracked()));

        let ed = Editor {
            cx: Cell::new(cx),
            // lines,
            effects_cx: Cell::new(cx.create_child()),
            id,
            active: cx.create_rw_signal(false),
            read_only: cx.create_rw_signal(false),
            doc,
            cursor,
            window_origin: cx.create_rw_signal(Point::ZERO),
            // viewport,
            parent_size: cx.create_rw_signal(Rect::ZERO),
            scroll_delta: cx.create_rw_signal(Vec2::ZERO),
            scroll_to: cx.create_rw_signal(None),
            editor_view_focused: cx.create_trigger(),
            editor_view_focus_lost: cx.create_trigger(),
            editor_view_id: cx.create_rw_signal(None),
            // screen_lines,
            register: cx.create_rw_signal(Register::default()),
            cursor_info: CursorInfo::new(cx),
            last_movement: cx.create_rw_signal(Movement::Left),
            ime_allowed: cx.create_rw_signal(false),
            floem_style_id: cx.create_rw_signal(0),
        };

        create_view_effects(ed.effects_cx.get(), &ed);

        ed
    }

    pub fn id(&self) -> EditorId {
        self.id
    }

    /// Get the document untracked
    pub fn doc(&self) -> Rc<Doc> {
        self.doc.get_untracked()
    }

    pub fn doc_track(&self) -> Rc<Doc> {
        self.doc.get()
    }

    // TODO: should this be `ReadSignal`? but read signal doesn't have .track
    pub fn doc_signal(&self) -> RwSignal<Rc<Doc>> {
        self.doc
    }

    // pub fn config_id(&self) -> ConfigId {
    //     let style_id = self.doc.with(|s| s.id());
    //     let floem_style_id = self.floem_style_id;
    //     ConfigId::new(style_id, floem_style_id.get_untracked())
    // }

    pub fn recreate_view_effects(&self) {
        batch(|| {
            self.effects_cx.get().dispose();
            self.effects_cx.set(self.cx.get().create_child());
            create_view_effects(self.effects_cx.get(), self);
        });
    }

    /// Swap the underlying document out
    pub fn update_doc(&self, doc: Rc<Doc>) {
        info!("update_doc");
        batch(|| {
            // Get rid of all the effects
            self.effects_cx.get().dispose();
            self.doc.set(doc);
            // self.doc()
            //     .lines
            //     .update(|lines| lines.trigger_signals_force());

            // Recreate the effects
            self.effects_cx.set(self.cx.get().create_child());
            create_view_effects(self.effects_cx.get(), self);
        });
    }

    // pub fn update_styling(&self, styling: Rc<dyn Styling>) {
    //     batch(|| {
    //         // Get rid of all the effects
    //         self.effects_cx.get().dispose();
    //
    //         // let font_sizes = Rc::new(EditorFontSizes {
    //         //     id: self.id(),
    //         //     style: self.style.read_only(),
    //         //     doc: self.doc.read_only(),
    //         // });
    //
    //         let ed = self.clone();
    //         self.lines.update(|x| {
    //             x.update(&ed);
    //         });
    //         //
    //         // *self.lines.font_sizes.borrow_mut() =
    //         // self.lines.clear(0, None);
    //
    //         self.style.set(styling);
    //
    //         self.screen_lines.update(|screen_lines| {
    //             screen_lines.clear(self.viewport.get_untracked());
    //         });
    //
    //         // Recreate the effects
    //         self.effects_cx.set(self.cx.get().create_child());
    //         create_view_effects(self.effects_cx.get(), self);
    //     });
    // }

    // pub fn duplicate(&self, editor_id: Option<EditorId>) -> Editor {
    //     let doc = self.doc();
    //     let style = self.style();
    //     let mut editor = Editor::new_direct(
    //         self.cx.get(),
    //         editor_id.unwrap_or_else(EditorId::next),
    //         doc,
    //         style,
    //         false,
    //     );
    //
    //     batch(|| {
    //         editor.read_only.set(self.read_only.get_untracked());
    //         editor.es.set(self.es.get_untracked());
    //         editor
    //             .floem_style_id
    //             .set(self.floem_style_id.get_untracked());
    //         editor.cursor.set(self.cursor.get_untracked());
    //         editor.scroll_delta.set(self.scroll_delta.get_untracked());
    //         editor.scroll_to.set(self.scroll_to.get_untracked());
    //         editor.window_origin.set(self.window_origin.get_untracked());
    //         editor.viewport.set(self.viewport.get_untracked());
    //         editor.parent_size.set(self.parent_size.get_untracked());
    //         editor.register.set(self.register.get_untracked());
    //         editor.cursor_info = self.cursor_info.clone();
    //         editor.last_movement.set(self.last_movement.get_untracked());
    //         // ?
    //         // editor.ime_allowed.set(self.ime_allowed.get_untracked());
    //     });
    //
    //     editor.recreate_view_effects();
    //
    //     editor
    // }

    // /// Get the styling untracked
    // pub fn style(&self) -> Rc<dyn Styling> {
    //     self.doc.get_untracked()
    // }

    /// Get the text of the document  
    /// You should typically prefer [`Self::rope_text`]
    pub fn text(&self) -> Rope {
        self.doc().text()
    }

    /// Get the [`RopeTextVal`] from `doc` untracked
    pub fn rope_text(&self) -> RopeTextVal {
        self.doc().rope_text()
    }

    // pub fn vline_infos(&self, start: usize, end: usize) -> Vec<VLineInfo<VLine>> {
    //     self.doc()
    //         .lines
    //         .with_untracked(|x| x.vline_infos(start, end))
    // }

    pub fn text_prov(&self) -> &Self {
        self
    }

    fn preedit(&self) -> PreeditData {
        self.doc.with_untracked(|doc| doc.preedit())
    }

    pub fn set_preedit(
        &self,
        text: String,
        cursor: Option<(usize, usize)>,
        offset: usize,
    ) {
        batch(|| {
            self.preedit().preedit.set(Some(Preedit {
                text,
                cursor,
                offset,
            }));

            self.doc().cache_rev().update(|cache_rev| {
                *cache_rev += 1;
            });
        });
    }

    pub fn clear_preedit(&self) {
        let preedit = self.preedit();
        if preedit.preedit.with_untracked(|preedit| preedit.is_none()) {
            return;
        }

        batch(|| {
            preedit.preedit.set(None);
            self.doc().cache_rev().update(|cache_rev| {
                *cache_rev += 1;
            });
        });
    }

    pub fn receive_char(&self, c: &str) {
        self.doc().receive_char(self, c)
    }

    // fn compute_screen_lines(&self, base: RwSignal<ScreenLinesBase>) -> ScreenLines {
    //     // This function *cannot* access `ScreenLines` with how it is currently implemented.
    //     // This is being called from within an update to screen lines.
    //
    //     self.doc().compute_screen_lines(self, base)
    // }

    /// Default handler for `PointerDown` event
    pub fn pointer_down(&self, pointer_event: &PointerInputEvent) {
        match pointer_event.button {
            PointerButton::Primary => {
                self.active.set(true);
                self.left_click(pointer_event);
            }
            PointerButton::Secondary => {
                self.right_click(pointer_event);
            }
            _ => {}
        }
    }

    pub fn left_click(&self, pointer_event: &PointerInputEvent) {
        match pointer_event.count {
            1 => {
                self.single_click(pointer_event);
            }
            2 => {
                self.double_click(pointer_event);
            }
            3 => {
                self.triple_click(pointer_event);
            }
            _ => {}
        }
    }

    pub fn single_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.mode().clone());
        let (new_offset, _) = match self.offset_of_point(&mode, pointer_event.pos) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };
        self.cursor.update(|cursor| {
            cursor.set_offset(
                new_offset,
                pointer_event.modifiers.shift(),
                pointer_event.modifiers.alt(),
            )
        });
    }

    pub fn double_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.mode().clone());

        let (mouse_offset, _) = match self.offset_of_point(&mode, pointer_event.pos)
        {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };
        let (start, end) = self.select_word(mouse_offset);
        info!("double_click {:?} {:?} mouse_offset={mouse_offset},  start={start} end={end}", pointer_event.pos, mode);
        self.cursor.update(|cursor| {
            cursor.add_region(
                start,
                end,
                pointer_event.modifiers.shift(),
                pointer_event.modifiers.alt(),
            )
        });
    }

    pub fn triple_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.mode().clone());
        let (mouse_offset, _) = match self.offset_of_point(&mode, pointer_event.pos)
        {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };
        let lines = match self.doc().lines.lines_of_origin_offset(mouse_offset) {
            Ok(lines) => lines,
            Err(err) => {
                error!("{}", err);
                return;
            }
        };
        // let vline = self
        //     .visual_line_of_offset(mouse_offset, CursorAffinity::Backward)
        //     .0;

        self.cursor.update(|cursor| {
            cursor.add_region(
                lines.origin_folded_line.origin_interval.start,
                lines.origin_folded_line.origin_interval.end,
                pointer_event.modifiers.shift(),
                pointer_event.modifiers.alt(),
            )
        });
    }

    pub fn pointer_move(&self, pointer_event: &PointerMoveEvent) {
        let mode = self.cursor.with_untracked(|c| c.mode().clone());
        let (offset, _is_inside) =
            match self.offset_of_point(&mode, pointer_event.pos) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return;
                }
            };
        if self.active.get_untracked()
            && self.cursor.with_untracked(|c| c.offset()) != offset
        {
            self.cursor.update(|cursor| {
                cursor.set_offset(offset, true, pointer_event.modifiers.alt())
            });
        }
    }

    pub fn pointer_up(&self, _pointer_event: &PointerInputEvent) {
        self.active.set(false);
    }

    fn right_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.mode().clone());
        let (offset, _) = match self.offset_of_point(&mode, pointer_event.pos) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };
        let doc = self.doc();
        let pointer_inside_selection = self.cursor.with_untracked(|c| {
            match c.edit_selection(&doc.rope_text()) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return false;
                }
            }
            .contains(offset)
        });
        if !pointer_inside_selection {
            // move cursor to pointer position if outside current selection
            self.single_click(pointer_event);
        }
    }

    // TODO: should this have modifiers state in its api
    pub fn page_move(&self, down: bool, mods: Modifiers) {
        let viewport = self.viewport();
        // TODO: don't assume line height is constant
        let line_height = self.line_height(0) as f64;
        let lines = (viewport.height() / line_height / 2.0).round() as usize;
        let distance = (lines as f64) * line_height;
        self.scroll_delta
            .set(Vec2::new(0.0, if down { distance } else { -distance }));
        let cmd = if down {
            MoveCommand::Down
        } else {
            MoveCommand::Up
        };
        let cmd = Command::Move(cmd);
        self.doc().run_command(self, &cmd, Some(lines), mods);
    }

    pub fn center_window(&self) {
        let viewport = self.viewport();
        // TODO: don't assume line height is constant
        let line_height = self.line_height(0) as f64;
        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = match self.offset_to_line_col(offset) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };

        let viewport_center = viewport.height() / 2.0;

        let current_line_position = line as f64 * line_height;

        let desired_top =
            current_line_position - viewport_center + (line_height / 2.0);

        let scroll_delta = desired_top - viewport.y0;

        self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
    }

    pub fn top_of_window(&self, scroll_off: usize) {
        let viewport = self.viewport();
        // TODO: don't assume line height is constant
        let line_height = self.line_height(0) as f64;
        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = match self.offset_to_line_col(offset) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };

        let desired_top = (line.saturating_sub(scroll_off)) as f64 * line_height;

        let scroll_delta = desired_top - viewport.y0;

        self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
    }

    pub fn bottom_of_window(&self, scroll_off: usize) {
        let viewport = self.viewport();
        // TODO: don't assume line height is constant
        let line_height = self.line_height(0) as f64;
        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = match self.offset_to_line_col(offset) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };

        let desired_bottom =
            (line + scroll_off + 1) as f64 * line_height - viewport.height();

        let scroll_delta = desired_bottom - viewport.y0;

        self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
    }

    pub fn scroll(&self, top_shift: f64, down: bool, count: usize, mods: Modifiers) {
        let viewport = self.viewport();
        // TODO: don't assume line height is constant
        let line_height = self.line_height(0) as f64;
        let diff = line_height * count as f64;
        let diff = if down { diff } else { -diff };

        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = match self.offset_to_line_col(offset) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };
        let top = viewport.y0 + diff + top_shift;
        let bottom = viewport.y0 + diff + viewport.height();

        let new_line = if (line + 1) as f64 * line_height + line_height > bottom {
            let line = (bottom / line_height).floor() as usize;
            if line > 2 {
                line - 2
            } else {
                0
            }
        } else if line as f64 * line_height - line_height < top {
            let line = (top / line_height).ceil() as usize;
            line + 1
        } else {
            line
        };

        self.scroll_delta.set(Vec2::new(0.0, diff));

        let res = match new_line.cmp(&line) {
            Ordering::Greater => Some((MoveCommand::Down, new_line - line)),
            Ordering::Less => Some((MoveCommand::Up, line - new_line)),
            _ => None,
        };

        if let Some((cmd, count)) = res {
            let cmd = Command::Move(cmd);
            self.doc().run_command(self, &cmd, Some(count), mods);
        }
    }

    // === Information ===

    // pub fn phantom_text(&self, line: usize) -> PhantomTextLine {
    //     self.doc()
    //         .phantom_text(self.id(), &self.es.get_untracked(), line)
    // }

    // pub fn line_height(&self, line: usize) -> f32 {
    //     self.doc().line_height(line)
    // }

    // === Line Information ===

    // /// Iterate over the visual lines in the view, starting at the given line.
    // pub fn iter_vlines(
    //     &self,
    //     backwards: bool,
    //     start: VLine,
    // ) -> impl Iterator<Item = VLineInfo> + '_ {
    //     self.lines.iter_vlines(self.text_prov(), backwards, start)
    // }

    // /// Iterate over the visual lines in the view, starting at the given line and ending at the
    // /// given line. `start_line..end_line`
    // pub fn iter_vlines_over(
    //     &self,
    //     backwards: bool,
    //     start: VLine,
    //     end: VLine,
    // ) -> impl Iterator<Item = VLineInfo> + '_ {
    //     self.lines
    //         .iter_vlines_over(self.text_prov(), backwards, start, end)
    // }

    // /// Iterator over *relative* [`VLineInfo`]s, starting at the buffer line, `start_line`.
    // /// The `visual_line`s provided by this will start at 0 from your `start_line`.
    // /// This is preferable over `iter_lines` if you do not need to absolute visual line value.
    // pub fn iter_rvlines(
    //     &self,
    //     backwards: bool,
    //     start: RVLine,
    // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
    //     self.lines
    //         .iter_rvlines(self.text_prov().clone(), backwards, start)
    // }

    // /// Iterator over *relative* [`VLineInfo`]s, starting at the buffer line, `start_line` and
    // /// ending at `end_line`.
    // /// `start_line..end_line`
    // /// This is preferable over `iter_lines` if you do not need to absolute visual line value.
    // pub fn iter_rvlines_over(
    //     &self,
    //     backwards: bool,
    //     start: RVLine,
    //     end_line: usize,
    // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
    //     self.lines
    //         .iter_rvlines_over(self.text_prov(), backwards, start, end_line)
    // }

    // ==== Position Information ====
    //
    // pub fn first_rvline_info(&self) -> VLineInfo<VLine> {
    //     self.doc().lines.with_untracked(|x| x.first_vline_info())
    // }

    /// The number of lines in the document.
    pub fn num_lines(&self) -> usize {
        self.rope_text().num_lines()
    }

    /// The last allowed buffer line in the document.
    pub fn last_line(&self) -> usize {
        self.rope_text().last_line()
    }

    // pub fn last_vline(&self) -> VLine {
    //     self.doc()
    //         .lines
    //         .with_untracked(|x| x.last_visual_line().into())
    // }

    // pub fn last_rvline(&self) -> RVLine {
    //     self.doc()
    //         .lines
    //         .with_untracked(|x| x.last_visual_line().into())
    // }

    // pub fn last_rvline_info(&self) -> VLineInfo<()> {
    //     self.rvline_info(self.last_rvline())
    // }

    // ==== Line/Column Positioning ====

    /// Convert an offset into the buffer into a line and idx.  
    pub fn offset_to_line_col(&self, offset: usize) -> Result<(usize, usize)> {
        self.rope_text().offset_to_line_col(offset)
    }

    pub fn offset_of_line(&self, line: usize) -> Result<usize> {
        self.rope_text().offset_of_line(line)
    }

    pub fn offset_of_line_col(&self, line: usize, col: usize) -> Result<usize> {
        self.rope_text().offset_of_line_col(line, col)
    }

    /// Get the buffer line of an offset
    // pub fn line_of_offset(&self, offset: usize) -> usize {
    //     self.rope_text().line_of_offset(offset)
    // }

    /// Returns the offset into the buffer of the first non blank character on the given line.
    pub fn first_non_blank_character_on_line(&self, line: usize) -> Result<usize> {
        self.rope_text().first_non_blank_character_on_line(line)
    }

    pub fn line_end_col(&self, line: usize, caret: bool) -> Result<usize> {
        self.rope_text().line_end_col(line, caret)
    }

    pub fn select_word(&self, offset: usize) -> (usize, usize) {
        self.rope_text().select_word(offset)
    }

    // /// `affinity` decides whether an offset at a soft line break is considered to be on the
    // /// previous line or the next line.
    // /// If `affinity` is `CursorAffinity::Forward` and is at the very end of the wrapped line, then
    // /// the offset is considered to be on the next line.
    // pub fn vline_of_offset(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity,
    // ) -> Result<VLine> {
    //     let (origin_line, offset_of_line) = self.doc.with_untracked(|x| {
    //         let text = x.text();
    //         let origin_line = text.line_of_offset(offset);
    //         let origin_line_start_offset = text.offset_of_line(origin_line);
    //         (origin_line, origin_line_start_offset)
    //     });
    //     let offset = offset - offset_of_line;
    //     self.doc().lines.with_untracked(|x| {
    //         let rs =
    //             x.visual_line_of_origin_line_offset(origin_line, offset, affinity);
    //         if rs.is_err() {
    //             x.log();
    //         }
    //         rs.map(|x| x.0.vline)
    //     })
    // }

    // pub fn vline_of_line(&self, line: usize) -> VLine {
    //     self.lines.vline_of_line(self.text_prov(), line)
    // }

    // pub fn rvline_of_line(&self, line: usize) -> RVLine {
    //     self.lines.rvline_of_line(self.text_prov(), line)
    // }

    // pub fn vline_of_rvline(&self, rvline: RVLine) -> Result<VLine> {
    //     self.doc().lines.with_untracked(|x| {
    //         x.visual_line_of_folded_line_and_sub_index(
    //             rvline.line,
    //             rvline.line_index,
    //         )
    //         .map(|x| x.into())
    //     })
    // }

    // /// Get the nearest offset to the start of the visual line.
    // pub fn offset_of_vline(&self, vline: VLine) -> usize {
    //     self.lines.offset_of_vline(self.text_prov(), vline)
    // }

    // /// Get the visual line and column of the given offset.
    // /// The column is before phantom text is applied.
    // pub fn vline_col_of_offset(&self, offset: usize, affinity: CursorAffinity) -> (VLine, usize) {
    //     self.lines
    //         .vline_col_of_offset(self.text_prov(), offset, affinity)
    // }

    // /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移
    // pub fn visual_line_of_offset(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity,
    // ) -> Result<(VLineInfo, usize, bool)> {
    //     let (origin_line, offset_of_line) = self.doc.with_untracked(|x| {
    //         let text = x.text();
    //         let origin_line = text.line_of_offset(offset);
    //         let origin_line_start_offset = text.offset_of_line(origin_line);
    //         (origin_line, origin_line_start_offset)
    //     });
    //     let offset = offset - offset_of_line;
    //     self.doc().lines.with_untracked(|x| {
    //         x.visual_line_of_origin_line_offset(origin_line, offset, affinity)
    //     })
    // }

    // /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移
    // fn cursor_position_of_buffer_offset(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity,
    // ) -> Result<(
    //     VisualLine,
    //     usize,
    //     usize,
    //     bool,
    //     // Point,
    //     Option<Point>,
    //     f64,
    //     Point,
    // )> {
    //     self.doc()
    //         .lines
    //         .with_untracked(|x| x.cursor_position_of_buffer_offset(offset, affinity))
    // }

    /// return visual_line, offset_of_visual, offset_of_folded, last_char
    /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移，是否是最后的字符
    pub fn visual_line_of_offset_v2(
        &self,
        offset: usize,
        affinity: CursorAffinity,
    ) -> Result<(VisualLine, usize, usize, bool)> {
        self.doc().lines.with_untracked(|x| {
            x.visual_line_of_offset(offset, affinity)
                .map(|x| (x.0, x.1, x.2, x.3))
        })
    }

    /// 视觉行的偏移位置，对应的上一行的偏移位置（原始文本）和是否为最后一个字符
    pub fn previous_visual_line(
        &self,
        visual_line_index: usize,
        line_offset: usize,
        _affinity: CursorAffinity,
    ) -> (VisualLine, usize, bool) {
        self.doc().lines.with_untracked(|x| {
            x.previous_visual_line(visual_line_index, line_offset, _affinity)
        })
    }

    /// 视觉行的偏移位置，对应的上一行的偏移位置（原始文本）和是否为最后一个字符
    pub fn next_visual_line(
        &self,
        visual_line_index: usize,
        line_offset: usize,
        _affinity: CursorAffinity,
    ) -> (VisualLine, usize, bool) {
        self.doc().lines.with_untracked(|x| {
            x.next_visual_line(visual_line_index, line_offset, _affinity)
        })
    }

    // pub fn folded_line_of_offset(
    //     &self,
    //     offset: usize,
    //     _affinity: CursorAffinity,
    // ) -> OriginFoldedLine {
    //     let line = self.visual_line_of_offset(offset, _affinity).0.rvline.line;
    //     self.doc()
    //         .lines
    //         .with_untracked(|x| x.folded_line_of_origin_line(line).clone())
    // }

    // pub fn rvline_info_of_offset(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity,
    // ) -> Result<VLineInfo<VLine>> {
    //     self.visual_line_of_offset(offset, affinity).map(|x| x.0)
    // }

    // /// Get the first column of the overall line of the visual line
    // pub fn first_col<T: std::fmt::Debug>(
    //     &self,
    //     info: VLineInfo<T>,
    // ) -> Result<usize> {
    //     let line_start = info.interval.start;
    //     let start_offset = self.text().offset_of_line(info.origin_line)?;
    //     Ok(line_start - start_offset)
    // }

    // /// Get the last column in the overall line of the visual line
    // pub fn last_col<T: std::fmt::Debug>(
    //     &self,
    //     info: VLineInfo<T>,
    //     caret: bool,
    // ) -> Result<usize> {
    //     let vline_end = info.interval.end;
    //     let start_offset = self.text().offset_of_line(info.origin_line)?;
    //     // If these subtractions crash, then it is likely due to a bad vline being kept around
    //     // somewhere
    //     Ok(if !caret && !info.is_empty() {
    //         let vline_pre_end =
    //             self.rope_text().prev_grapheme_offset(vline_end, 1, 0);
    //         vline_pre_end - start_offset
    //     } else {
    //         vline_end - start_offset
    //     })
    // }

    // ==== Points of locations ====

    pub fn max_line_width(&self) -> f64 {
        self.doc().lines.with_untracked(|x| x.max_width())
    }

    /// Returns the point into the text layout of the line at the given offset.
    /// `x` being the leading edge of the character, and `y` being the baseline.
    pub fn line_point_of_offset(
        &self,
        offset: usize,
        affinity: CursorAffinity,
    ) -> Result<Point> {
        let (line, col) = self.offset_to_line_col(offset)?;
        Ok(self.line_point_of_visual_line_col(line, col, affinity, false))
    }

    /// Returns the point into the text layout of the line at the given line and col.
    /// `x` being the leading edge of the character, and `y` being the baseline.  
    pub fn line_point_of_visual_line_col(
        &self,
        visual_line: usize,
        col: usize,
        affinity: CursorAffinity,
        _force_affinity: bool,
    ) -> Point {
        self.doc().lines.with_untracked(|x| {
            x.line_point_of_visual_line_col(
                visual_line,
                col,
                affinity,
                _force_affinity,
            )
        })
    }

    /// Get the (point above, point below) of a particular offset within the editor.
    pub fn points_of_offset(
        &self,
        offset: usize,
        affinity: CursorAffinity,
    ) -> Result<(Point, Point)> {
        let (_, _, _, _, point, _, _, line_height) =
            self.doc.get_untracked().lines.with_untracked(|x| {
                x.cursor_position_of_buffer_offset(offset, affinity)
            })?;
        // let (line_info, line_offset, _) =
        //     self.visual_line_of_offset(offset, affinity)?;
        // let line = line_info.vline.0;
        // let line_height = f64::from(self.doc().line_height(line_info.origin_line));

        // let info = self.doc().lines.with_untracked(|sl| {
        //     sl.screen_lines().iter_line_info().find(|info| {
        //         info.vline_info.interval.start <= offset
        //             && offset <= info.vline_info.interval.end
        //     })
        // });
        let Some(info) = point else {
            // TODO: We could do a smarter method where we get the approximate y position
            // because, for example, this spot could be folded away, and so it would be better to
            // supply the *nearest* position on the screen.
            return Ok((Point::new(0.0, 0.0), Point::new(0.0, 0.0)));
        };

        // let y = info.vline_y;
        //
        // let x = self
        //     .line_point_of_visual_line_col(line, line_offset, affinity, false)
        //     .x;
        let mut low_point = info;
        low_point.y += line_height as f64;
        Ok((info, low_point))
    }

    /// Get the offset of a particular point within the editor.
    /// The boolean indicates whether the point is inside the text or not
    /// Points outside of vertical bounds will return the last line.
    /// Points outside of horizontal bounds will return the last column on the line.
    pub fn offset_of_point(
        &self,
        mode: &CursorMode,
        point: Point,
    ) -> Result<(usize, bool)> {
        self.doc
            .get_untracked()
            .lines
            .with_untracked(|x| x.buffer_offset_of_click(mode, point))
    }

    // /// 获取该坐标所在的视觉行和行偏离
    // pub fn line_col_of_point_with_phantom(
    //     &self,
    //     point: Point,
    // ) -> (usize, usize, TextLayoutLine) {
    //     let line_height = f64::from(self.doc().line_height(0));
    //     let y = point.y.max(0.0);
    //     let visual_line = (y / line_height) as usize;
    //     let text_layout = self.text_layout_of_visual_line(visual_line);
    //     let hit_point = text_layout.text.hit_point(Point::new(point.x, y));
    //     (visual_line, hit_point.index, text_layout)
    // }

    // /// Get the (line, col) of a particular point within the editor.
    // /// The boolean indicates whether the point is within the text bounds.
    // /// Points outside of vertical bounds will return the last line.
    // /// Points outside of horizontal bounds will return the last column on the line.
    // pub fn line_col_of_point(
    //     &self,
    //     _mode: &CursorMode,
    //     point: Point,
    //     _tracing: bool,
    // ) -> ((usize, usize), bool) {
    //     // TODO: this assumes that line height is constant!
    //     let line_height = f64::from(self.doc().line_height(0));
    //     let info = if point.y <= 0.0 {
    //         self.first_rvline_info()
    //     } else {
    //         self.doc().lines.with_untracked(|sl| {
    //             let sl = &sl.screen_lines();
    //             if let Some(info) = sl.iter_line_info().find(|info| {
    //                 info.vline_y <= point.y && info.vline_y + line_height >= point.y
    //             }) {
    //                 info.vline_info
    //             } else {
    //                 if sl.lines.last().is_none() {
    //                     panic!("point: {point:?} {:?} {:?}", sl.lines, sl.info);
    //                 }
    //                 let info = sl.info(*sl.lines.last().unwrap());
    //                 if info.is_none() {
    //                     panic!("point: {point:?} {:?} {:?}", sl.lines, sl.info);
    //                 }
    //                 info.unwrap().vline_info
    //             }
    //         })
    //     };
    //
    //     let rvline = info.rvline;
    //     let line = rvline.line;
    //     let text_layout = self.text_layout_of_visual_line(line);
    //
    //     let y = text_layout.get_layout_y(rvline.line_index).unwrap_or(0.0);
    //
    //     let hit_point = text_layout.text.hit_point(Point::new(point.x, y as f64));
    //     // We have to unapply the phantom text shifting in order to get back to the column in
    //     // the actual buffer
    //     let (line, col, _) = text_layout
    //         .phantom_text
    //         .cursor_position_of_final_col(hit_point.index);
    //
    //     ((line, col), hit_point.is_inside)
    // }

    // pub fn line_horiz_col(
    //     &self,
    //     line: usize,
    //     horiz: &ColPosition,
    //     caret: bool, visual_line: &VisualLine,
    // ) -> usize {
    //     match *horiz {
    //         ColPosition::Col(x) => {
    //             // TODO: won't this be incorrect with phantom text? Shouldn't this just use
    //             // line_col_of_point and get the col from that?
    //             let text_layout = self.text_layout_of_visual_line(line);
    //             let hit_point = text_layout.text.hit_point(Point::new(x, 0.0));
    //             let n = hit_point.index;
    //             text_layout.phantom_text.origin_position_of_final_col(n)
    //         }
    //         ColPosition::End => (line, self.line_end_col(line, caret)),
    //         ColPosition::Start => (line, 0),
    //         ColPosition::FirstNonBlank => {
    //             (line, self.first_non_blank_character_on_line(line))
    //         }
    //     }
    // }

    // /// Advance to the right in the manner of the given mode.
    // /// Get the column from a horizontal at a specific line index (in a text layout)
    // pub fn rvline_horiz_col(
    //     &self,
    //     // RVLine { line, line_index }: RVLine,
    //     horiz: &ColPosition,
    //     _caret: bool,
    //     visual_line: &VisualLine,
    // ) -> usize {
    //     match *horiz {
    //         ColPosition::Col(x) => {
    //             let text_layout = &visual_line.text_layout;
    //             let y_pos = text_layout
    //                 .text
    //                 .layout_runs()
    //                 .nth(visual_line.origin_folded_line_sub_index)
    //                 .map(|run| run.line_y)
    //                 .or_else(|| {
    //                     text_layout.text.layout_runs().last().map(|run| run.line_y)
    //                 })
    //                 .unwrap_or(0.0);
    //             let hit_point =
    //                 text_layout.text.hit_point(Point::new(x, y_pos as f64));
    //             let n = hit_point.index;
    //             let rs = text_layout.phantom_text.cursor_position_of_final_col(n);
    //             rs.2 + rs.1
    //         }
    //         ColPosition::End => visual_line.origin_interval.end,
    //         ColPosition::Start => visual_line.origin_interval.start,
    //         ColPosition::FirstNonBlank => {
    //             let final_offset = visual_line.text_layout.text.line().text()
    //                 [visual_line.visual_interval.start
    //                     ..visual_line.visual_interval.end]
    //                 .char_indices()
    //                 .find(|(_, c)| !c.is_whitespace())
    //                 .map(|(idx, _)| visual_line.visual_interval.start + idx)
    //                 .unwrap_or(visual_line.visual_interval.end);
    //             let rs = visual_line
    //                 .text_layout
    //                 .phantom_text
    //                 .cursor_position_of_final_col(final_offset);
    //             rs.2 + rs.1
    //         }
    //     }
    // }

    /// Advance to the right in the manner of the given mode.  
    /// This is not the same as the [`Movement::Right`] command.
    pub fn move_right(
        &self,
        offset: usize,
        mode: Mode,
        count: usize,
    ) -> Result<usize> {
        self.rope_text().move_right(offset, mode, count)
    }

    /// Advance to the left in the manner of the given mode.
    /// This is not the same as the [`Movement::Left`] command.
    pub fn move_left(
        &self,
        offset: usize,
        mode: Mode,
        count: usize,
    ) -> Result<usize> {
        self.rope_text().move_left(offset, mode, count)
    }

    /// ~~视觉~~行的text_layout信息
    pub fn text_layout_of_visual_line(&self, line: usize) -> TextLayoutLine {
        self.doc()
            .lines
            .with_untracked(|x| x.text_layout_of_visual_line(line).clone())
    }

    // pub fn lines(&self) -> DocLinesManager {
    //     self.doc.with_untracked(|x| x.doc_lines)
    // }

    pub fn viewport(&self) -> Rect {
        self.doc().lines.with_untracked(|x| x.viewport())
    }

    pub fn line_height(&self, _line: usize) -> usize {
        self.doc().lines.with_untracked(|x| x.config.line_height)
    }
    pub fn font_size(&self, _line: usize) -> usize {
        self.doc().lines.with_untracked(|x| x.config.font_size)
    }

    pub fn font_family(&self) -> String {
        self.doc()
            .lines
            .with_untracked(|x| x.config.font_family.clone())
    }

    // pub fn text_layout_trigger(&self, line: usize, trigger: bool) -> Arc<TextLayoutLine> {
    //     let cache_rev = self.doc().cache_rev().get_untracked();
    //     self.lines
    //         .get_init_text_layout(cache_rev, self.config_id(), self, line, trigger)
    // }

    // fn try_get_text_layout(&self, line: usize) -> Option<Arc<TextLayoutLine>> {
    //     let cache_rev = self.doc().cache_rev().get_untracked();
    //     self.lines
    //         .try_get_text_layout(cache_rev, self.config_id(), line)
    // }
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Editor").field(&self.id).finish()
    }
}

// fn strip_suffix(line_content_original: &str) -> String {
//     if let Some(s) = line_content_original.strip_suffix("\r\n") {
//         format!("{s}  ")
//     } else if let Some(s) = line_content_original.strip_suffix('\n') {
//         format!("{s} ",)
//     } else {
//         line_content_original.to_string()
//     }
// }
//
// fn push_strip_suffix(line_content_original: &str, rs: &mut String) {
//     if let Some(s) = line_content_original.strip_suffix("\r\n") {
//         rs.push_str(s);
//         rs.push_str("  ");
//         // format!("{s}  ")
//     } else if let Some(s) = line_content_original.strip_suffix('\n') {
//         rs.push_str(s);
//         rs.push(' ');
//     } else {
//         rs.push_str(line_content_original);
//     }
// }

// impl TextLayoutProvider for Editor {
//     // TODO: should this just return a `Rope`?
//     fn text(&self) -> Rope {
//         Editor::text(self)
//     }
//
//     fn new_text_layout(&self, line: usize) -> Arc<TextLayoutLine> {
//         // TODO: we could share text layouts between different editor views given some knowledge of
//         // their wrapping
//         let doc = self.doc();
//         // line = doc.visual_line_of_line(line);
//         new_text_layout(doc, line)
//     }
//
//     /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义？
//     fn before_phantom_col(&self, line: usize, col: usize) -> (usize, usize) {
//         self.new_text_layout(line)
//             .phantom_text
//             .origin_position_of_final_col(col)
//         // self.doc()
//         //     .before_phantom_col(self.id(), &self.es.get_untracked(), line, col)
//     }
//
//     // fn has_multiline_phantom(&self) -> bool {
//     //     self.doc()
//     //         .has_multiline_phantom(self.id(), &self.es.get_untracked())
//     // }
// }
#[allow(dead_code)]
pub struct EditorFontSizes {
    id: EditorId,
    style: ReadSignal<Rc<dyn Styling>>,
    doc: ReadSignal<Rc<Doc>>,
}
impl EditorFontSizes {
    // fn font_size(&self, line: usize) -> usize {
    //     self.style.with_untracked(|style| style.font_size(line))
    // }

    // fn cache_id(&self) -> FontSizeCacheId {
    //     let mut hasher = DefaultHasher::new();
    //
    //     // TODO: is this actually good enough for comparing cache state?
    //     // We could just have it return an arbitrary type that impl's Eq?
    //     self.style
    //         .with_untracked(|style| style.id().hash(&mut hasher));
    //     self.doc
    //         .with_untracked(|doc| doc.cache_rev().get_untracked().hash(&mut hasher));
    //
    //     hasher.finish()
    // }
}

// /// Minimum width that we'll allow the view to be wrapped at.
// const MIN_WRAPPED_WIDTH: f32 = 100.0;

/// Create various reactive effects to update the screen lines whenever relevant parts of the view,
/// doc, text layouts, viewport, etc. change.
/// This tries to be smart to a degree.
fn create_view_effects(cx: Scope, ed: &Editor) {
    {
        let cursor_info = ed.cursor_info.clone();
        let cursor = ed.cursor;
        cx.create_effect(move |_| {
            cursor.track();
            cursor_info.reset();
        });
    }

    // let update_screen_lines = |ed: &Editor| {
    //     // This function should not depend on the viewport signal directly.
    //
    //     // This is wrapped in an update to make any updates-while-updating very obvious
    //     // which they wouldn't be if we computed and then `set`.
    //     ed.screen_lines.update(|screen_lines| {
    //         let new_screen_lines = ed.compute_screen_lines(screen_lines.base);
    //
    //         *screen_lines = new_screen_lines;
    //     });
    // };

    // Listen for layout events, currently only when a layout is created, and update screen
    // lines based on that
    // ed3.lines.with_untracked(|x| x.layout_event.listen_with(cx, move |val| {
    //     let ed = &ed2;
    //     // TODO: Move this logic onto screen lines somehow, perhaps just an auxiliary
    //     // function, to avoid getting confused about what is relevant where.
    //
    //     match val {
    //         LayoutEvent::CreatedLayout { line, .. } => {
    //             let sl = ed.screen_lines.get_untracked();
    //
    //             // Intelligently update screen lines, avoiding recalculation if possible
    //             let should_update = sl.on_created_layout(ed, line);
    //
    //             if should_update {
    //                 untrack(|| {
    //                     update_screen_lines(ed);
    //                 });
    //
    //                 // Ensure that it is created even after the base/viewport signals have been
    //                 // updated.
    //                 // But we have to trigger an event since it could alter the screenlines
    //                 // TODO: this has some risk for infinite looping if we're unlucky.
    //                 ed2.text_layout_trigger(line, true);
    //             }
    //         }
    //     }
    // }));

    // TODO: should we have some debouncing for editor width? Ideally we'll be fast enough to not
    // even need it, though we might not want to use a bunch of cpu whilst resizing anyway.

    // let viewport_changed_trigger = cx.create_trigger();

    // Watch for changes to the viewport so that we can alter the wrapping
    // As well as updating the screen lines base
    // cx.create_effect(move |_| {
    //     let ed = &ed3;
    //
    //     let viewport = ed.viewport.get();
    //
    //     // let wrap = match ed.es.with(|s| s.wrap_method()) {
    //     //     WrapMethod::None => ResolvedWrap::None,
    //     //     WrapMethod::EditorWidth => {
    //     //         ResolvedWrap::Width((viewport.width() as f32).max(MIN_WRAPPED_WIDTH))
    //     //     }
    //     //     WrapMethod::WrapColumn { .. } => todo!(),
    //     //     WrapMethod::WrapWidth { width } => ResolvedWrap::Width(width),
    //     // };
    //
    //     // ed.lines.update(|x| x.set_wrap(wrap, ed));
    //     // ed.lines.set_wrap(wrap, ed);
    //
    //     // Update the base
    //     let base = ed.screen_lines.with_untracked(|sl| sl.base);
    //
    //     // TODO: should this be a with or with_untracked?
    //     if viewport != base.with_untracked(|base| base.active_viewport) {
    //         batch(|| {
    //             base.update(|base| {
    //                 base.active_viewport = viewport;
    //             });
    //             // TODO: Can I get rid of this and just call update screen lines with an
    //             // untrack around it?
    //             viewport_changed_trigger.notify();
    //         });
    //     }
    // });
    // Watch for when the viewport as changed in a relevant manner
    // and for anything that `update_screen_lines` tracks.
    // cx.create_effect(move |_| {
    //     viewport_changed_trigger.track();
    //
    //     update_screen_lines(&ed4);
    // });
}

// pub fn normal_compute_screen_lines(
//     editor: &Editor,
//     base: RwSignal<ScreenLinesBase>,
// ) -> ScreenLines {
//     let lines = &editor.lines;
//     let style = editor.style.get();
//     // TODO: don't assume universal line height!
//     let line_height = style.line_height(editor.id(), 0);
//
//     let (y0, y1) = base.with_untracked(|base| (base.active_viewport.y0, base.active_viewport.y1));
//     // Get the start and end (visual) lines that are visible in the viewport
//     let min_vline = VLine((y0 / line_height as f64).floor() as usize);
//     let max_vline = VLine((y1 / line_height as f64).ceil() as usize);
//
//     let cache_rev = editor.doc.get().cache_rev().get();
//     editor.lines.check_cache_rev(cache_rev);
//
//     let min_info = editor.iter_vlines(false, min_vline).next();
//
//     let mut rvlines = Vec::new();
//     let mut info = HashMap::new();
//
//     let Some(min_info) = min_info else {
//         return ScreenLines {
//             lines: Rc::new(rvlines),
//             info: Rc::new(info),
//             diff_sections: None,
//             base,
//         };
//     };
//
//     // TODO: the original was min_line..max_line + 1, are we iterating too little now?
//     // the iterator is from min_vline..max_vline
//     let count = max_vline.get() - min_vline.get();
//     let iter = lines
//         .iter_rvlines_init(
//             editor.text_prov(),
//             cache_rev,
//             editor.config_id(),
//             min_info.rvline,
//             false,
//         )
//         .take(count);
//
//     for (i, vline_info) in iter.enumerate() {
//         rvlines.push(vline_info.rvline);
//
//         let line_height = f64::from(style.line_height(editor.id(), vline_info.rvline.line));
//
//         let y_idx = min_vline.get() + i;
//         let vline_y = y_idx as f64 * line_height;
//         let line_y = vline_y - vline_info.rvline.line_index as f64 * line_height;
//
//         // Add the information to make it cheap to get in the future.
//         // This y positions are shifted by the baseline y0
//         info.insert(
//             vline_info.rvline,
//             LineInfo {
//                 y: line_y - y0,
//                 vline_y: vline_y - y0,
//                 vline_info,
//             },
//         );
//     }
//
//     ScreenLines {
//         lines: Rc::new(rvlines),
//         info: Rc::new(info),
//         diff_sections: None,
//         base,
//     }
// }

// TODO: should we put `cursor` on this structure?
/// Cursor rendering information
#[derive(Clone)]
pub struct CursorInfo {
    pub hidden: RwSignal<bool>,

    pub blink_timer: RwSignal<TimerToken>,
    // TODO: should these just be rwsignals?
    pub should_blink: Rc<dyn Fn() -> bool + 'static>,
    pub blink_interval: Rc<dyn Fn() -> u64 + 'static>,
}
impl CursorInfo {
    pub fn new(cx: Scope) -> CursorInfo {
        CursorInfo {
            hidden: cx.create_rw_signal(false),

            blink_timer: cx.create_rw_signal(TimerToken::INVALID),
            should_blink: Rc::new(|| true),
            blink_interval: Rc::new(|| 500),
        }
    }

    pub fn blink(&self) {
        let info = self.clone();
        let blink_interval = (info.blink_interval)();
        if blink_interval > 0 && (info.should_blink)() {
            let blink_timer = info.blink_timer;
            let timer_token = exec_after(
                Duration::from_millis(blink_interval),
                move |timer_token| {
                    if info.blink_timer.try_get_untracked() == Some(timer_token) {
                        info.hidden.update(|hide| {
                            *hide = !*hide;
                        });
                        info.blink();
                    }
                },
            );
            blink_timer.set(timer_token);
        }
    }

    pub fn reset(&self) {
        if self.hidden.get_untracked() {
            self.hidden.set(false);
        }

        self.blink_timer.set(TimerToken::INVALID);

        self.blink();
    }
}

// /// Get the render information for a caret cursor at the given `offset`.
// pub fn cursor_caret(
//     ed: &Editor,
//     offset: usize,
//     block: bool,
//     affinity: CursorAffinity,
// ) -> Result<LineRegion> {
//     let (info, col, after_last_char) = ed.visual_line_of_offset(offset, affinity)?;
//
//     let doc = ed.doc();
//     let preedit_start = doc
//         .preedit()
//         .preedit
//         .with_untracked(|preedit| {
//             preedit.as_ref().and_then(|preedit| {
//                 // todo?
//                 let preedit_line =
//                     ed.visual_line_of_offset(preedit.offset, affinity).ok()?.0;
//                 preedit.cursor.map(|x| (preedit_line, x))
//             })
//         })
//         .filter(|(preedit_line, _)| *preedit_line == info)
//         .map(|(_, (start, _))| start);
//
//     let point = ed.line_point_of_visual_line_col(
//         info.origin_line,
//         col,
//         CursorAffinity::Forward,
//         false,
//     );
//
//     let rvline = if preedit_start.is_some() {
//         // If there's an IME edit, then we need to use the point's y to get the actual y position
//         // that the IME cursor is at. Since it could be in the middle of the IME phantom text
//         let y = point.y;
//
//         // TODO: I don't think this is handling varying line heights properly
//         let line_height = ed.line_height(info.origin_line);
//
//         let line_index = (y / f64::from(line_height)).floor() as usize;
//         RVLine::new(info.origin_line, line_index)
//     } else {
//         info.rvline
//     };
//     // error!("offset={offset} block={block}, point={point:?} rvline={rvline:?} info={info:?} col={col} after_last_char={after_last_char}");
//
//     let x0 = point.x;
//     Ok(if block {
//         let x0 = ed
//             .line_point_of_visual_line_col(
//                 info.origin_line,
//                 col,
//                 CursorAffinity::Forward,
//                 true,
//             )
//             .x;
//         let new_offset = ed.move_right(offset, Mode::Insert, 1);
//         let (_, new_col) = ed.offset_to_line_col(new_offset);
//
//         let width = if after_last_char {
//             CHAR_WIDTH
//         } else {
//             let x1 = ed
//                 .line_point_of_visual_line_col(
//                     info.origin_line,
//                     new_col,
//                     CursorAffinity::Backward,
//                     true,
//                 )
//                 .x;
//             x1 - x0
//         };
//
//         LineRegion {
//             x: x0,
//             width,
//             rvline,
//         }
//     } else {
//         LineRegion {
//             x: x0 - 1.0,
//             width: 2.0,
//             rvline,
//         }
//     })
// }

/// (x, y, line_height, width)
pub fn cursor_caret_v2(
    ed: &Editor,
    offset: usize,
    block: bool,
    affinity: CursorAffinity,
) -> Option<(f64, f64, f64, f64)> {
    let (
        _info,
        _col_visual,
        _offset_folded,
        _after_last_char,
        point,
        // screen,
        line_height,
        _origin_point,
        _,
    ) = match ed
        .doc()
        .lines
        .with_untracked(|x| x.cursor_position_of_buffer_offset(offset, affinity))
    {
        Ok(rs) => rs,
        Err(err) => {
            error!("{err:?}");
            return None;
        }
    };

    if block {
        panic!("block");
    } else {
        point.map(|point| (point.x - 1.0, point.y, 2.0, line_height))
    }
}

pub fn cursor_origin_position(
    ed: &Editor,
    offset: usize,
    block: bool,
    affinity: CursorAffinity,
) -> Result<(Point, f64, usize)> {
    // let (
    //     _info,
    //     _col_visual,
    //     _offset_folded,
    //     _after_last_char,
    //     _point,
    //     // screen,
    //     line_height,
    //     mut origin_point,
    // ) = ed.cursor_position_of_buffer_offset(offset, affinity)?;

    let (
        _info,
        _col_visual,
        _offset_folded,
        _after_last_char,
        _point,
        // screen,
        line_height,
        mut origin_point,
        _,
    ) = ed
        .doc()
        .lines
        .with_untracked(|x| x.cursor_position_of_buffer_offset(offset, affinity))?;
    if block {
        panic!("block");
    } else {
        origin_point.x -= 1.0;
        Ok((origin_point, line_height, _info.line_index))
    }
}

pub fn do_motion_mode(
    ed: &Editor,
    action: &dyn CommonAction,
    cursor: &mut Cursor,
    motion_mode: MotionMode,
    register: &mut Register,
) {
    if let Some(cached_motion_mode) = cursor.motion_mode.take() {
        // If it's the same MotionMode discriminant, continue, count is cached in the old motion_mode.
        if core::mem::discriminant(&cached_motion_mode)
            == core::mem::discriminant(&motion_mode)
        {
            let offset = cursor.offset();
            action.exec_motion_mode(
                ed,
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
        ed: &Editor,
        cursor: &mut Cursor,
        motion_mode: MotionMode,
        range: Range<usize>,
        is_vertical: bool,
        register: &mut Register,
    );

    // TODO: should we have a more general cursor state structure?
    // since modal is about cursor, and register is sortof about cursor
    // but also there might be other state it wants. Should we just pass Editor to it?
    /// Perform an edit.
    /// Returns `true` if there was any change.
    fn do_edit(
        &self,
        ed: &Editor,
        cursor: &mut Cursor,
        cmd: &EditCommand,
        modal: bool,
        register: &mut Register,
        smart_tab: bool,
    ) -> bool;
}

pub fn paint_selection(cx: &mut PaintCx, ed: &Editor, _screen_lines: &ScreenLines) {
    let cursor = ed.cursor;

    let selection_color = ed.doc().lines.with_untracked(|es| es.selection_color());

    cursor.with_untracked(|cursor| match cursor.mode() {
        CursorMode::Normal(_) => {}
        CursorMode::Visual {
            start,
            end,
            mode: VisualMode::Normal,
        } => {
            let start_offset = start.min(end);
            let end_offset = match ed.move_right(*start.max(end), Mode::Insert, 1) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return;
                }
            };

            if let Err(err) = paint_normal_selection(
                cx,
                ed,
                selection_color,
                *start_offset,
                end_offset,
                cursor.affinity,
            ) {
                error!("{err:?}");
            }
        }
        CursorMode::Visual {
            start: _start,
            end: _end,
            mode: VisualMode::Linewise,
        } => {
            error!("todo implement paint_linewise_selection");
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
        }
        CursorMode::Visual {
            start: _start,
            end: _end,
            mode: VisualMode::Blockwise,
        } => {
            error!("todo implement paint_blockwise_selection");
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
        }
        CursorMode::Insert(_) => {
            for (start, end) in
                cursor.regions_iter().filter(|(start, end)| start != end)
            {
                if let Err(err) = paint_normal_selection(
                    cx,
                    ed,
                    selection_color,
                    start.min(end),
                    start.max(end),
                    cursor.affinity,
                ) {
                    error!("{err:?}");
                }
            }
        }
    });
}
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
//     error!("todo replace paint_blockwise_selection start_offset={start_offset} end_offset={end_offset}");
//     let (start_rvline, start_col, _) =
//         ed.visual_line_of_offset(start_offset, affinity)?;
//     let (end_rvline, end_col, _) = ed.visual_line_of_offset(end_offset, affinity)?;
//     let start_rvline = start_rvline.rvline;
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
//                     let line_height = ed.line_height(info.vline_info.origin_line);
//                     let rect = Rect::from_origin_size(
//                         (viewport.x0, info.vline_y),
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
    ed: &Editor,
    color: Color,
    start_offset: usize,
    end_offset: usize,
    _affinity: CursorAffinity,
) -> Result<()> {
    let rs = ed
        .doc()
        .lines
        .with_untracked(|x| x.normal_selection(start_offset, end_offset))?;
    for rect in rs {
        cx.fill(&rect, color, 0.0);
    }
    Ok(())
}

pub fn paint_text(
    cx: &mut PaintCx,
    ed: &Editor,
    viewport: Rect,
    is_active: bool,
    screen_lines: &ScreenLines,
    _show_indent_guide: (bool, Color),
) {
    let style = ed.doc();

    // todo???
    // TODO: cache indent text layout width
    // let indent_unit = ed
    //     .doc()
    //     .lines
    //     .with_untracked(|es| es.indent_style())
    //     .as_str();
    // // TODO: don't assume font family is the same for all lines?
    // let family = style.font_family(0);
    // let attrs = Attrs::new()
    //     .family(&family)
    //     .font_size(style.font_size(0) as f32);
    // let attrs_list = AttrsList::new(attrs);
    //
    // let indent_text = TextLayout::new_with_text(&format!("{indent_unit}a"), attrs_list);
    // let indent_text_width = indent_text.hit_position(indent_unit.len()).point.x;
    // let base = screen_lines.base.y0;
    // if show_indent_guide.0 {
    //     for line_info in &screen_lines.visual_lines {
    //         let line = line_info.visual_line.origin_line;
    //         let y = line_info.visual_line_y + base;
    //         let text_layout = ed.text_layout_of_visual_line(line);
    //         let line_height = f64::from(ed.line_height(line));
    //         let mut x = 0.0;
    //         while x + 1.0 < text_layout.indent {
    //             cx.stroke(
    //                 &Line::new(Point::new(x, y), Point::new(x, y + line_height)),
    //                 show_indent_guide.1,
    //                 1.0,
    //             );
    //             x += indent_text_width;
    //         }
    //     }
    // }
    let base = screen_lines.base.y0;
    paint_cursor_caret(cx, ed, is_active, screen_lines);

    for line_info in &screen_lines.visual_lines {
        let line = line_info.visual_line.origin_line;
        let y = line_info.visual_line_y + base;
        let text_layout = ed.text_layout_of_visual_line(line);

        paint_extra_style(cx, &text_layout.extra_style, y, viewport);

        if let Some(whitespaces) = &text_layout.whitespaces {
            let family = style.font_family(line);
            let font_size = style.font_size(line) as f32;
            let attrs = Attrs::new()
                .color(ed.doc().lines.with_untracked(|es| es.visible_whitespace()))
                .family(&family)
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
                    }
                    ' ' => {
                        cx.draw_text_with_layout(
                            space_text.layout_runs(),
                            Point::new(*x0, y),
                        );
                    }
                    _ => {}
                }
            }
        }

        cx.draw_text_with_layout(text_layout.text.layout_runs(), Point::new(0.0, y));
    }
}

pub fn paint_extra_style(
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
            let y = y + style.y;
            cx.fill(
                &Rect::ZERO
                    .with_size(Size::new(width, height))
                    .with_origin(Point::new(x, y)),
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
            EditorView::paint_wave_line(cx, width, Point::new(style.x, y), color);
        }
    }
}

fn paint_cursor_caret(
    cx: &mut PaintCx,
    ed: &Editor,
    is_active: bool,
    _screen_lines: &ScreenLines,
) {
    let cursor = ed.cursor;
    let hide_cursor = ed.cursor_info.hidden;
    let caret_color = ed.doc().lines.with_untracked(|es| es.ed_caret());

    if !is_active || hide_cursor.get_untracked() {
        return;
    }

    cursor.with_untracked(|cursor| {
        // let style = ed.doc();
        // let cursor_offset = cursor.offset();
        for (_, end) in cursor.regions_iter() {
            let is_block = cursor.is_block();
            if let Some((x, y, width, line_height)) =
                cursor_caret_v2(ed, end, is_block, cursor.affinity)
            {
                // if !style.paint_caret(ed.id(), rvline.line) {
                //     continue;
                // }

                // let line_height = ed.line_height(info.vline_info.origin_line);
                let rect = Rect::from_origin_size((x, y), (width, line_height));
                cx.fill(&rect, &caret_color, 0.0);
            }
        }
    });
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
//     error!("todo replace paint_linewise_selection start_offset={start_offset} end_offset={end_offset} affinity={affinity:?}");
//     let (start_rvline, _, _) = ed.visual_line_of_offset(start_offset, affinity)?;
//     let (end_rvline, _, _) = ed.visual_line_of_offset(end_offset, affinity)?;
//     let start_rvline = start_rvline.rvline;
//     let end_rvline = end_rvline.rvline;
//     // Linewise selection is by *line* so we move to the start/end rvlines of the line
//     let start_rvline = screen_lines
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

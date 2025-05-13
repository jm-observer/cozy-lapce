use std::{
    borrow::Cow,
    ops::{AddAssign, SubAssign},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use anyhow::{Result, bail};
use doc::{
    EditorViewKind,
    lines::{
        buffer::{Buffer, diff::DiffLines, rope_text::RopeText},
        cursor::CursorMode,
        diff::DiffResult,
        fold::{FoldingDisplayItem, FoldingDisplayType},
        layout::TextLayout,
        line::LineTy,
        line_ending::LineEnding,
        screen_lines::{DiffSectionKind, ScreenLines},
        selection::SelRegion,
        style::{
            CurrentLineColor, CursorSurroundingLines, DocumentHighlightColor,
            EditorViewClass, IndentGuideColor, IndentStyleProp, Modal,
            ModalRelativeLine, PhantomColor, PlaceholderColor,
            PreeditUnderlineColor, RenderWhitespaceProp, ScrollBeyondLastLine,
            SelectionColor, ShowIndentGuide, SmartTab, VisibleWhitespaceColor,
            WrapProp,
        },
        text::WrapMethod,
    },
};
use floem::{
    Renderer, View, ViewId,
    action::{set_ime_allowed, set_ime_cursor_area},
    context::{PaintCx, StyleCx},
    event::{Event, EventListener, EventPropagation},
    keyboard::Modifiers,
    kurbo::Stroke,
    peniko::{
        Brush, Color,
        kurbo::{Line, Point, Rect, Size},
    },
    prelude::{SvgColor, h_stack},
    reactive::{
        Memo, RwSignal, SignalGet, SignalTrack, SignalUpdate, SignalWith,
        create_memo, create_rw_signal,
    },
    style::{CursorColor, CursorStyle, Style, TextColor},
    taffy::prelude::NodeId,
    text::{AttrsList, FamilyOwned},
    views::{
        Decorators, container, dyn_stack, empty, label,
        scroll::{PropagatePointerWheel, scroll},
        stack, svg, text_input,
    },
};
use lapce_core::{doc::DocContent, icon::LapceIcons, workspace::LapceWorkspace};
use lapce_xi_rope::find::CaseMatching;
use log::error;

use super::{DocSignal, EditorData, floem_editor::get_selection};
use crate::{
    app::clickable_icon,
    command::InternalCommand,
    common_svg,
    config::{LapceConfig, WithLapceConfig, color::LapceColor, editor::WrapStyle},
    editor::{floem_editor::paint_text, gutter_new::view::editor_gutter_new},
    keypress::KeyPressFocus,
    window_workspace::{CommonData, Focus, WindowWorkspaceData},
};

#[derive(Clone, Debug, Default)]
pub struct StickyHeaderInfo {
    pub sticky_lines:              Vec<usize>,
    pub last_sticky_should_scroll: bool,
    pub y_diff:                    f64,
}

fn editor_wrap(wrap_style: WrapStyle, wrap_with: usize) -> WrapMethod {
    /// Minimum width that we'll allow the view to be wrapped at.
    const MIN_WRAPPED_WIDTH: f32 = 100.0;

    match wrap_style {
        WrapStyle::None => WrapMethod::None,
        WrapStyle::EditorWidth => WrapMethod::EditorWidth,
        WrapStyle::WrapWidth => WrapMethod::WrapWidth {
            width: (wrap_with as f32).max(MIN_WRAPPED_WIDTH),
        },
    }
}

pub fn editor_style(config: WithLapceConfig, doc: DocSignal, s: Style) -> Style {
    let (
        scroll_beyond_last_line,
        show_indent_guide,
        modal,
        modal_mode_relative_line_numbers,
        smart_tab,
        cursor_surrounding_lines,
        render_whitespace,
        wrap_style,
        wrap_with,
        caret_color,
        selection_color,
        document_highlight_color,
        cl_color,
        vw,
        ig,
        fore,
        dim,
    ) = config.signal(|config| {
        (
            config.editor.scroll_beyond_last_line.signal(),
            config.editor.show_indent_guide.signal(),
            config.core.modal.signal(),
            config.editor.modal_mode_relative_line_numbers.signal(),
            config.editor.smart_tab.signal(),
            config.editor.cursor_surrounding_lines.signal(),
            config.editor.render_whitespace.signal(),
            config.editor.wrap_style.signal(),
            config.editor.wrap_width.signal(),
            config.color(LapceColor::EDITOR_CARET),
            config.color(LapceColor::EDITOR_SELECTION),
            config.color(LapceColor::EDITOR_DOCUMENT_HIGHLIGHT),
            config.color(LapceColor::EDITOR_CURRENT_LINE),
            config.color(LapceColor::EDITOR_VISIBLE_WHITESPACE),
            config.color(LapceColor::EDITOR_INDENT_GUIDE),
            config.color(LapceColor::EDITOR_FOREGROUND),
            config.color(LapceColor::EDITOR_DIM),
        )
    });

    let doc = doc.get();
    let fore = fore.get();
    let dim = dim.get();
    s.set(
        IndentStyleProp,
        doc.lines
            .with_untracked(|x| Buffer::indent_style(x.buffer())),
    )
    .set(CursorColor, caret_color.get())
    .set(SelectionColor, selection_color.get())
    .set(DocumentHighlightColor, document_highlight_color.get())
    .set(CurrentLineColor, cl_color.get())
    .set(VisibleWhitespaceColor, vw.get())
    .set(IndentGuideColor, ig.get())
    .set(ScrollBeyondLastLine, scroll_beyond_last_line.get())
    .color(fore)
    .set(TextColor, fore)
    .set(PhantomColor, dim)
    .set(PlaceholderColor, dim)
    .set(PreeditUnderlineColor, fore)
    .set(ShowIndentGuide, show_indent_guide.get())
    .set(Modal, modal.get())
    .set(ModalRelativeLine, modal_mode_relative_line_numbers.get())
    .set(SmartTab, smart_tab.get())
    .set(WrapProp, editor_wrap(wrap_style.get(), wrap_with.get()))
    .set(CursorSurroundingLines, cursor_surrounding_lines.get())
    .set(RenderWhitespaceProp, render_whitespace.get())
}

#[allow(dead_code)]
pub struct EditorView {
    id:              ViewId,
    name:            &'static str,
    editor:          EditorData,
    is_active:       Memo<bool>,
    inner_node:      Option<NodeId>,
    // viewport: RwSignal<Rect>,
    // lines: DocLinesManager,
    debug_breakline: Memo<Option<(usize, PathBuf)>>, // tracing: bool,
}

pub fn editor_view(
    e_data: EditorData,
    debug_breakline: Memo<Option<(usize, PathBuf)>>,
    is_active: impl Fn(bool) -> bool + 'static + Copy,
    // tracing: bool,
    name: &'static str,
) -> EditorView {
    let id = ViewId::new();
    let scope = e_data.scope;
    let is_active = create_memo(move |_| is_active(true));

    let viewport = e_data.viewport.read_only();
    let screen_lines = e_data.screen_lines.read_only();

    let doc = e_data.doc_signal();
    // let lines = doc.with_untracked(|x| x.lines);
    let view_kind = e_data.kind_read();
    scope.create_effect(move |_| {
        doc.with(|x| x.lines.with_untracked(|x| x.signal_max_width()))
            .get();
        view_kind.track();
        log::debug!(
            "signal_max_width focus when tab change but content is not changed"
        );
        // id.request_layout();
        id.request_all();
    });

    scope.create_effect(move |last_rev| {
        let lines = doc.with(|doc| doc.lines);
        let rev = lines.with_untracked(|x| x.signal_buffer_rev()).get();
        if last_rev == Some(rev) {
            return rev;
        }
        id.request_layout();
        rev
    });

    let config = e_data.common.config;
    let sticky_header_height_signal = e_data.sticky_header_height;
    let editor2 = e_data.clone();
    scope.create_effect(move |last_rev| {
        let (line_height, sticky_header) = config.signal(|config| {
            (
                config.editor.line_height.signal(),
                config.editor.sticky_header.signal(),
            )
        });
        if !sticky_header.get() {
            return (DocContent::Local, 0, 0, Rect::ZERO, 0, None);
        }

        let doc = doc.get();
        let rect = viewport.get();
        let screen_lines = screen_lines.get();
        let (screen_lines_len, screen_lines_first) = (
            screen_lines.visual_lines.len(),
            screen_lines
                .first_end_folded_line()
                .map(|x| x.0.folded_line.origin_line_start),
        );
        let buffer_rev = doc.lines.with_untracked(|x| x.signal_buffer_rev());
        let rev = (
            doc.content.get(),
            buffer_rev.get(),
            doc.cache_rev.get(),
            rect,
            screen_lines_len,
            screen_lines_first,
        );
        if last_rev.as_ref() == Some(&rev) {
            return rev;
        }

        let sticky_header_info = get_sticky_header_info(
            &editor2,
            rect,
            sticky_header_height_signal,
            &screen_lines,
            line_height.get() as f64,
        );

        id.update_state(sticky_header_info);

        rev
    });

    let editor_window_origin = e_data.window_origin();
    let cursor = e_data.cursor();
    let find_focus = e_data.find_focus;
    let ime_allowed = e_data.common.window_common.ime_allowed;
    let editor_viewport = e_data.viewport;
    let editor_cursor = e_data.cursor();
    let doc = e_data.doc_signal();
    let e_data_clone = e_data.clone();
    let e_data_event = e_data.clone();
    let e_data_2 = e_data.clone();
    let e_data_3 = e_data.clone();
    scope.create_effect(move |_| {
        let active = is_active.get();
        if active && !find_focus.get() {
            if !cursor.with(|c| c.is_insert()) {
                if ime_allowed.get_untracked() {
                    ime_allowed.set(false);
                    set_ime_allowed(false);
                }
            } else {
                if !ime_allowed.get_untracked() {
                    ime_allowed.set(true);
                    set_ime_allowed(true);
                }
                let offset = cursor.with(|c| c.offset());
                let doc = doc.get_untracked();

                if doc.loaded() {
                    let (_, point_below) = match e_data.points_of_offset(offset) {
                        Ok(rs) => rs,
                        Err(err) => {
                            error!("{err:?}");
                            return;
                        },
                    };
                    let window_origin = editor_window_origin.get();
                    let viewport = editor_viewport.get();
                    let pos = window_origin
                        + (point_below.x - viewport.x0, point_below.y - viewport.y0);
                    set_ime_cursor_area(pos, Size::new(800.0, 600.0));
                    // log::info!("set_ime_cursor_area offset={offset}
                    // window_origin={window_origin:?} viewport={viewport:?}
                    // point_below={point_below:?} pos={pos:?}",);
                }
            }
        }
    });

    EditorView {
        id,
        name,
        editor: e_data_clone,
        is_active,
        inner_node: None,
        // viewport: viewport_rw,
        debug_breakline, // tracing,
    }
    .on_event(EventListener::ImePreedit, move |event| {
        if !is_active.get_untracked() {
            return EventPropagation::Continue;
        }
        if let Event::ImePreedit { text, cursor } = event {
            let doc = doc.get_untracked();
            if text.is_empty() {
                e_data_2.clear_preedit(&doc);
            } else {
                // log::info!("Event::ImePreedit set_ime_cursor_area {text} cursor:
                // {cursor:?}");
                let offset = editor_cursor.with_untracked(|c| c.offset());
                e_data_2.set_preedit(text.clone(), *cursor, offset, &doc);
            }
        }
        EventPropagation::Stop
    })
    .on_event(EventListener::ImeCommit, move |event| {
        if !is_active.get_untracked() {
            return EventPropagation::Continue;
        }
        let doc = doc.get_untracked();
        if let Event::ImeCommit(text) = event {
            e_data_3.clear_preedit(&doc);
            e_data_event.receive_char(text);
        }
        EventPropagation::Stop
    })
    .class(EditorViewClass)
    .style(move |s| editor_style(config, doc, s))
}

impl EditorView {
    #[allow(clippy::too_many_arguments)]
    fn paint_diff_sections(
        &self,
        cx: &mut PaintCx,
        viewport: Rect,
        screen_lines: &ScreenLines,
        source_control_removed_color: &Color,
        source_control_added_color: &Color,
        line_height: usize,
        editor_dim_color: &Color,
    ) {
        let Some(diff_sections) = &screen_lines.diff_sections else {
            return;
        };
        for section in diff_sections.iter() {
            match section.kind {
                DiffSectionKind::NoCode => self.paint_diff_no_code(
                    cx,
                    viewport,
                    section.y_idx,
                    section.height,
                    line_height,
                    editor_dim_color,
                ),
                DiffSectionKind::Added => {
                    cx.fill(
                        &Rect::ZERO
                            .with_size(Size::new(
                                viewport.width(),
                                (line_height * section.height) as f64,
                            ))
                            .with_origin(Point::new(
                                viewport.x0,
                                (section.y_idx * line_height) as f64,
                            )),
                        source_control_added_color.multiply_alpha(0.2),
                        0.0,
                    );
                },
                DiffSectionKind::Removed => {
                    cx.fill(
                        &Rect::ZERO
                            .with_size(Size::new(
                                viewport.width(),
                                (line_height * section.height) as f64,
                            ))
                            .with_origin(Point::new(
                                viewport.x0,
                                (section.y_idx * line_height) as f64,
                            )),
                        source_control_removed_color.multiply_alpha(0.2),
                        0.0,
                    );
                },
            }
        }
    }

    fn paint_diff_no_code(
        &self,
        cx: &mut PaintCx,
        viewport: Rect,
        start_line: usize,
        height: usize,
        line_height: usize,
        editor_dim_color: &Color,
    ) {
        let height = (height * line_height) as f64;
        let y = (start_line * line_height) as f64;
        let y_end = y + height;

        if y_end < viewport.y0 || y > viewport.y1 {
            return;
        }

        let y = y.max(viewport.y0 - 10.0);
        let y_end = y_end.min(viewport.y1 + 10.0);
        let height = y_end - y;

        let start_x = viewport.x0.floor() as usize;
        let start_x = start_x - start_x % 8;

        for x in (start_x..viewport.x1.ceil() as usize + 1 + height.ceil() as usize)
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
                cx.stroke(&Line::new(p0, p1), editor_dim_color, &Stroke::new(1.0));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_current_line(
        &self,
        cx: &mut PaintCx,
        is_local: bool,
        screen_lines: &ScreenLines,
        editor_debug_break_line_color: &Color,
        current_line_color: Option<Color>,
        line_height: f64,
        cursor_highlight_current_line: bool,
        cursor_offset: usize,
    ) -> Option<()> {
        let e_data = self.editor.clone();
        let doc = e_data.doc();
        let viewport = self.editor.viewport_untracked();

        let current_line_color = current_line_color?;
        let breakline = self.debug_breakline.get_untracked().and_then(
            |(breakline, breakline_path)| {
                if doc
                    .content
                    .with_untracked(|c| c.path() == Some(&breakline_path))
                {
                    Some(breakline)
                } else {
                    None
                }
            },
        );
        if let Some(breakline) = breakline
            && let Some(info) =
                screen_lines.visual_line_info_for_origin_line(breakline)
        {
            let rect = Rect::from_origin_size(
                info.paint_point(screen_lines.base),
                (viewport.width(), line_height),
            );
            cx.fill(&rect, editor_debug_break_line_color, 0.0);
        }

        // Highlight the current line
        if !is_local && cursor_highlight_current_line {
            let (_, info) =
                screen_lines.visual_line_for_buffer_offset(cursor_offset)?;
            let origin_folded_line = &info.folded_line;
            if Some(origin_folded_line.origin_line_start) == breakline {
                return None;
            }
            if let Some(info) = screen_lines.visual_line_info_for_origin_line(
                origin_folded_line.origin_line_start,
            ) {
                let rect = Rect::from_origin_size(
                    info.paint_point(screen_lines.base),
                    (viewport.width(), line_height),
                );

                cx.fill(&rect, current_line_color, 0.0);
            }
        }
        None
    }

    fn paint_find(
        &self,
        cx: &mut PaintCx,
        screen_lines: &ScreenLines,
        color: Color,
    ) -> Result<()> {
        let find_visual = self.editor.common.find.visual.get_untracked();
        if !find_visual && self.editor.on_screen_find.with_untracked(|f| !f.active) {
            return Ok(());
        }
        if screen_lines.is_empty() {
            return Ok(());
        }

        let Some((start, end)) = screen_lines.offset_interval() else {
            return Ok(());
        };

        let e_data = &self.editor;
        let doc = e_data.doc();

        let occurrences = doc.find_result.occurrences;

        // let start = ed.offset_of_line(min_line);
        // let end = ed.offset_of_line(max_line + 1);

        // TODO: The selection rect creation logic for find is quite similar to the
        // version within insert cursor. It would be good to deduplicate it.
        if find_visual {
            doc.update_find();
            for region in occurrences.with_untracked(|selection| {
                selection.regions_in_range(start, end).to_vec()
            }) {
                if let Err(err) =
                    self.paint_find_region(cx, &region, color, screen_lines)
                {
                    error!("{err:?}");
                }
            }
        }

        self.editor.on_screen_find.with_untracked(|find| {
            if find.active {
                for region in &find.regions {
                    if let Err(err) =
                        self.paint_find_region(cx, region, color, screen_lines)
                    {
                        error!("{err:?}");
                    }
                }
            }
        });
        Ok(())
    }

    fn paint_find_region(
        &self,
        cx: &mut PaintCx,
        region: &SelRegion,
        color: Color,
        screen_lines: &ScreenLines,
    ) -> Result<()> {
        let (start, end, start_affinity, end_affinity) = if region.start > region.end
        {
            (
                region.end,
                region.start,
                region.end_cursor_affi,
                region.start_cursor_affi,
            )
        } else {
            (
                region.start,
                region.end,
                region.start_cursor_affi,
                region.end_cursor_affi,
            )
        };
        let rs = screen_lines.normal_selection(
            start,
            end,
            start_affinity,
            end_affinity,
        )?;
        for rect in rs {
            cx.stroke(&rect, color, &Stroke::new(1.0));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_sticky_headers(
        &self,
        cx: &mut PaintCx,
        viewport: Rect,
        sticky_header: bool,
        lapce_dropdown_shadow_color: &Color,
        editor_sticky_header_background_color: &Color,
        line_height: usize,
        start_line: usize,
    ) -> Result<()> {
        if !sticky_header {
            return Ok(());
        }
        if !self.editor.kind_read().get_untracked().is_normal() {
            return Ok(());
        }

        let sticky_header_info = self.editor.sticky_header_info.get_untracked();
        let total_sticky_lines = sticky_header_info.sticky_lines.len();

        let paint_last_line = total_sticky_lines > 0
            && (sticky_header_info.last_sticky_should_scroll
                || sticky_header_info.y_diff != 0.0
                || start_line + total_sticky_lines - 1
                    != *sticky_header_info.sticky_lines.last().unwrap());

        let total_sticky_lines = if paint_last_line {
            total_sticky_lines
        } else {
            total_sticky_lines.saturating_sub(1)
        };

        if total_sticky_lines == 0 {
            return Ok(());
        }

        let scroll_offset = if sticky_header_info.last_sticky_should_scroll {
            sticky_header_info.y_diff
        } else {
            0.0
        };

        let sticky_lines = sticky_header_info.sticky_lines.clone();
        let (attrs, line_ending, sticky_lines): (
            AttrsList,
            LineEnding,
            Vec<(usize, String)>,
        ) = self.editor.doc().lines.with_untracked(|lines| {
            (
                lines.init_default_attrs_list(),
                lines.buffer().line_ending(),
                sticky_lines
                    .into_iter()
                    .filter_map(|line| match lines.buffer().line_content(line) {
                        Ok(content) => Some((line, content.to_string())),
                        Err(err) => {
                            error!("{}", err);
                            None
                        },
                    })
                    .collect(),
            )
        });
        let area_height = (sticky_lines.len() * line_height) as f64 - scroll_offset;

        let sticky_area_rect = Size::new(viewport.x1, area_height)
            .to_rect()
            .with_origin(Point::new(0.0, viewport.y0))
            .inflate(10.0, 0.0);

        cx.fill(&sticky_area_rect, lapce_dropdown_shadow_color, 3.0);
        cx.fill(
            &sticky_area_rect,
            editor_sticky_header_background_color,
            0.0,
        );
        self.editor.sticky_header_info.get_untracked();
        // Paint lines
        let mut y_accum = 0.0;

        let line_ending_str = line_ending.get_chars();
        let line_height = line_height as f64;
        for (i, (_line, content)) in sticky_lines.into_iter().enumerate() {
            let y_diff = if i == total_sticky_lines - 1 {
                scroll_offset
            } else {
                0.0
            };
            let mut text = TextLayout::new(content, attrs.clone(), line_ending_str);
            // let text_layout =
            // self.editor.editor.text_layout_of_visual_line(line)?;
            // let text_height = (text_layout.line_count() * line_height) as f64;
            let height = line_height - y_diff;

            cx.save();

            let line_area_rect = Size::new(viewport.width(), height)
                .to_rect()
                .with_origin(Point::new(viewport.x0, viewport.y0 + y_accum));

            cx.clip(&line_area_rect);

            let y = viewport.y0 - y_diff + y_accum;
            cx.draw_text_with_layout(text.layout_runs(), Point::new(viewport.x0, y));

            y_accum += line_height;

            cx.restore();
        }
        Ok(())
    }

    fn paint_scroll_bar(
        &self,
        cx: &mut PaintCx,
        viewport: Rect,
        is_local: bool,
        lapce_scroll_bar_color: &Color,
    ) {
        const BAR_WIDTH: f64 = 10.0;

        if is_local {
            return;
        }

        cx.fill(
            &Rect::ZERO
                .with_size(Size::new(1.0, viewport.height()))
                .with_origin(Point::new(
                    viewport.x0 + viewport.width() - BAR_WIDTH,
                    viewport.y0,
                ))
                .inflate(0.0, 10.0),
            lapce_scroll_bar_color,
            0.0,
        );

        // if !self.editor.kind().get_untracked().is_normal() {
        //     return;
        // }
    }

    fn paint_bracket_highlights_scope_lines(
        &self,
        cx: &mut PaintCx,
        highlight_matching_brackets: bool,
        highlight_scope_lines: bool,
        editor_bracket_color: &Color,
        screen_lines: &ScreenLines,
        bracket_offsets: Option<(usize, usize)>,
    ) -> Result<Option<()>> {
        if highlight_matching_brackets || highlight_scope_lines {
            let Some((bracket_offsets_start, bracket_offsets_end)) = bracket_offsets
            else {
                return Ok(None);
            };

            if let Some(bracket_offsets_start) =
                screen_lines.char_rect_in_viewport(bracket_offsets_start)?
            {
                cx.fill(&bracket_offsets_start, editor_bracket_color, 0.0);
            };
            if let Some(bracket_offsets_end) =
                screen_lines.char_rect_in_viewport(bracket_offsets_end)?
            {
                cx.fill(&bracket_offsets_end, editor_bracket_color, 0.0);
            };

            // todo
            // if config.editor.highlight_scope_lines {
            //     self.paint_scope_lines(
            //         cx,
            //         viewport,
            //         screen_lines,
            //         bracket_line_cols[0],
            //         bracket_line_cols[1],
            //     );
            // }
        }
        Ok(None)
    }
}

impl View for EditorView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn style_pass(&mut self, cx: &mut StyleCx<'_>) {
        if match self
            .editor
            .doc()
            .lines
            .try_update(|s| s.update_editor_style(cx))
            .unwrap()
        {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            },
        } {
            // editor.floem_style_id.update(|val| *val += 1);
            cx.app_state_mut().request_paint(self.id());
        }
    }

    // fn debug_name(&self) -> std::borrow::Cow<'static, str> {
    //
    // }

    fn update(
        &mut self,
        _cx: &mut floem::context::UpdateCx,
        state: Box<dyn std::any::Any>,
    ) {
        if let Ok(state) = state.downcast() {
            self.editor.sticky_header_info.set(*state);
            self.id.request_layout();
        }
    }

    fn layout(
        &mut self,
        cx: &mut floem::context::LayoutCx,
    ) -> floem::taffy::prelude::NodeId {
        cx.layout_node(self.id, true, |_cx| {
            if self.inner_node.is_none() {
                self.inner_node = Some(self.id.new_taffy_node());
            }
            let e_data = &self.editor;
            let viewport_size = self.editor.viewport_untracked().size();
            let inner_node = self.inner_node.unwrap();
            let line_height = self
                .editor
                .common
                .config
                .with_untracked(|config| config.editor.line_height())
                as f64;
            let width = (e_data.max_line_width() + 10.0).max(viewport_size.width);

            let visual_line_len = e_data.visual_lines.with_untracked(|x| x.len());
            // let lines =
            //     editor.last_line() + screen_lines.lines.len() - line_unique.len();
            let last_line_height = line_height * visual_line_len as f64;
            let height = last_line_height.max(line_height).max(viewport_size.height);
            // log::info!("height={height} width={width} {}",
            // editor.max_line_width());
            // let margin_bottom =
            //     viewport_size.height.min(last_line_height) - line_height;

            let style = Style::new()
                .width(width)
                .height(height)
                // .margin_bottom(margin_bottom)
                .to_taffy_style();
            self.id.set_taffy_style(inner_node, style);

            vec![inner_node]
        })
    }

    fn compute_layout(
        &mut self,
        _cx: &mut floem::context::ComputeLayoutCx,
    ) -> Option<Rect> {
        // 会与diff的同步滚动冲突。观察后续的影响
        // let viewport = cx.current_viewport();
        // self.editor.editor.viewport.set(viewport);
        // self.editor.doc().lines.update(|x| {
        //     if let Err(err) = x.update_viewport_size(viewport) {
        //         error!("{err:?}");
        //     }
        // });

        None
    }

    fn paint(&mut self, cx: &mut PaintCx) {
        let doc = self.editor.doc_signal().get();
        let e_data = &self.editor;

        let cursor_hidden = e_data.common.window_common.hide_cursor.get_untracked();

        let (
            editor_debug_break_line_color,
            lapce_scroll_bar_color,
            source_control_removed_color,
            source_control_added_color,
            editor_dim_color,
            line_height,
            highlight_matching_brackets,
            highlight_scope_lines,
            editor_bracket_color,
            sticky_header,
            lapce_dropdown_shadow_color,
            editor_sticky_header_background_color,
            editor_fg,
            font_family_str,
            font_size,
        ) = e_data
            .common
            .config
            .with_untracked(|config| config.paint_editor());
        let font_family =
            Cow::Owned(FamilyOwned::parse_list(&font_family_str).collect());

        let is_local = doc.content.with_untracked(|content| content.is_local());
        let find_focus = self.editor.find_focus;
        let is_active =
            self.is_active.get_untracked() && !find_focus.get_untracked();

        let (
            cursor_offsets,
            cursor_highlight_current_line,
            cursor_offset,
            cursor_affinity,
            selections,
        ) = self.editor.cursor().with_untracked(|cursor| {
            let highlight_current_line = match cursor.mode() {
                CursorMode::Normal(_) | CursorMode::Insert(_) => true,
                CursorMode::Visual { .. } => false,
            };
            let cursor_offset = cursor.mode().offset();
            (
                cursor.regions_iter().map(|x| x.1).collect::<Vec<usize>>(),
                highlight_current_line,
                cursor_offset,
                cursor.affinity,
                get_selection(cursor),
            )
        });
        let screen_lines = self.editor.screen_lines.get_untracked();
        let viewport = self.editor.viewport.get_untracked();
        let cursor_points = cursor_offsets
            .into_iter()
            .filter_map(|offset| {
                match screen_lines
                    .cursor_position_of_buffer_offset(offset, cursor_affinity)
                {
                    Ok(rs) => {
                        // log::info!("buffer offset={offset} cursor_affinity={cursor_affinity:?} cursor position:{:?}", rs);
                        rs.map(|mut x| {
                            x.x -= viewport.x0;
                            x
                        })
                    },
                    Err(err) => {
                        error!("{}", err);
                        None
                    },
                }
            })
            .collect();

        let bracket_offsets = doc.find_enclosing_brackets(cursor_offset);

        let (visible_whitespace, current_line_color, selection_color) =
            doc.lines.with_untracked(|x| {
                (
                    x.visible_whitespace(),
                    x.current_line_color(),
                    x.selection_color(),
                )
            });

        // let screen_lines = screen_lines.get();
        self.paint_current_line(
            cx,
            is_local,
            &screen_lines,
            &editor_debug_break_line_color,
            current_line_color,
            line_height as f64,
            cursor_highlight_current_line,
            cursor_offset,
        );
        // paint_selection(cx, ed, &screen_lines);
        // let screen_lines = ed.screen_lines.get_untracked();

        self.paint_diff_sections(
            cx,
            viewport,
            &screen_lines,
            &source_control_removed_color,
            &source_control_added_color,
            line_height,
            &editor_dim_color,
        );
        // let screen_lines = ed.screen_lines.get_untracked();
        if let Err(err) = self.paint_find(cx, &screen_lines, editor_fg) {
            error!("{err}");
        }
        // let screen_lines = ed.screen_lines.get_untracked();
        if let Err(err) = self.paint_bracket_highlights_scope_lines(
            cx,
            highlight_matching_brackets,
            highlight_scope_lines,
            &editor_bracket_color,
            &screen_lines,
            bracket_offsets,
        ) {
            error!("{err}");
        }
        // let screen_lines = ed.screen_lines.get_untracked();
        // , cursor: RwSignal<Cursor>, lines: DocLinesManager
        let lines = doc.lines;
        let start_vline = screen_lines
            .first_end_folded_line()
            .map(|x| x.0.folded_line.origin_line_start);

        if let Err(err) = paint_text(
            cx,
            viewport,
            is_active,
            cursor_hidden,
            &screen_lines,
            lines,
            font_family,
            visible_whitespace,
            font_size,
            cursor_points,
            line_height as f64,
            editor_dim_color,
            source_control_added_color,
            selections,
            selection_color,
            cursor_offset,
            e_data,
        ) {
            error!("{err}");
        }

        if let Some(start_vline) = start_vline {
            // let screen_lines = ed.screen_lines.get_untracked();
            if let Err(err) = self.paint_sticky_headers(
                cx,
                viewport,
                sticky_header,
                &lapce_dropdown_shadow_color,
                &editor_sticky_header_background_color,
                line_height,
                start_vline,
            ) {
                error!("{err}");
            }
        }

        self.paint_scroll_bar(cx, viewport, is_local, &lapce_scroll_bar_color);
    }
}

fn get_sticky_header_info(
    editor_data: &EditorData,
    _viewport: Rect,
    sticky_header_height_signal: RwSignal<f64>,
    screen_lines: &ScreenLines,
    line_height: f64,
) -> StickyHeaderInfo {
    let doc = editor_data.doc();
    // let start_line = (viewport.y0 / line_height).floor() as usize;
    let Some(start) = screen_lines.first_end_folded_line() else {
        return StickyHeaderInfo {
            sticky_lines:              Vec::new(),
            last_sticky_should_scroll: false,
            y_diff:                    0.0,
        };
    };
    // let start_info = screen_lines.info(*start).unwrap();
    let start_line = start.0.folded_line.origin_line_start;

    // let y_diff = viewport.y0 - start_info.vline_y;
    let y_diff = 0.0;

    let mut last_sticky_should_scroll = false;
    let mut sticky_lines = Vec::new();
    if let Some(lines) = doc.sticky_headers(start_line) {
        let total_lines = lines.len();
        if total_lines > 0 {
            // info!("total_lines={total_lines} start_line={start_line}");
            let line = start_line + total_lines;
            if let Some(new_lines) = doc.sticky_headers(line) {
                // info!("total_lines={} line={line}", new_lines.len());
                if new_lines.len() > total_lines {
                    sticky_lines = new_lines;
                } else {
                    sticky_lines = lines;
                    last_sticky_should_scroll = new_lines.len() < total_lines;
                    if new_lines.len() < total_lines {
                        if let Some(new_new_lines) =
                            doc.sticky_headers(start_line + total_lines - 1)
                        {
                            if new_new_lines.len() < total_lines {
                                sticky_lines.pop();
                                last_sticky_should_scroll = false;
                            }
                        } else {
                            sticky_lines.pop();
                            last_sticky_should_scroll = false;
                        }
                    }
                }
            } else {
                sticky_lines = lines;
                last_sticky_should_scroll = true;
            }
        }
    }

    let total_sticky_lines = sticky_lines.len();

    let paint_last_line = total_sticky_lines > 0
        && (last_sticky_should_scroll
            || y_diff != 0.0
            || start_line + total_sticky_lines - 1 != *sticky_lines.last().unwrap());

    // Fix up the line count in case we don't need to paint the last one.
    let total_sticky_lines = if paint_last_line {
        total_sticky_lines
    } else {
        total_sticky_lines.saturating_sub(1)
    };

    if total_sticky_lines == 0 {
        sticky_header_height_signal.set(0.0);
        return StickyHeaderInfo {
            sticky_lines:              Vec::new(),
            last_sticky_should_scroll: false,
            y_diff:                    0.0,
        };
    }

    let sticky_header_height = sticky_lines.len() as f64 * line_height;
    sticky_header_height_signal.set(sticky_header_height);
    StickyHeaderInfo {
        sticky_lines,
        last_sticky_should_scroll,
        y_diff,
    }
}

pub fn editor_container_view(
    window_tab_data: WindowWorkspaceData,
    workspace: Arc<LapceWorkspace>,
    is_active: impl Fn(bool) -> bool + 'static + Copy,
    editor: EditorData,
) -> impl View {
    let main_split = window_tab_data.main_split.clone();
    // let editors = main_split.editors;
    // let scratch_docs = main_split.scratch_docs;
    let replace_active = main_split.common.find.replace_active;
    // let replace_focus = main_split.common.find.replace_focus;
    let debug_breakline = window_tab_data.terminal.breakline;

    let find_str = main_split.find_str;
    let replace_str = main_split.replace_str;
    let find_view_id = main_split.common.find_view_id;
    let common = main_split.common.clone();
    let config = main_split.common.config;

    let editor_empty = editor.clone();
    stack((
        editor_breadcrumbs(workspace, editor.clone(), config),
        stack((
            editor_gutter_new(window_tab_data.clone(), editor.clone()),
            editor_gutter_folding_range(window_tab_data.clone(), editor.clone()),
            editor_content(editor.clone(), debug_breakline, is_active),
            empty().style(move |s| {
                let sticky_header = config
                    .signal(|config| config.editor.sticky_header.signal())
                    .get();
                let (sticky_header_height, editor_view) =
                    (editor_empty.sticky_header_height, editor_empty.kind_read());
                let sticky_header_height = sticky_header_height.get() as f32;

                s.absolute()
                    .width_pct(100.0)
                    .height(sticky_header_height)
                    // .box_shadow_blur(5.0)
                    // .border_bottom(1.0)
                    // .border_color(
                    //     config.get_color(LapceColor::LAPCE_BORDER),
                    // )
                    .apply_if(
                        !sticky_header
                            || sticky_header_height == 0.0
                            || !editor_view.get().is_normal(),
                        |s| s.hide(),
                    )
            }),
            find_view(
                editor,
                replace_active,
                common,
                find_str,
                find_view_id,
                replace_str,
                window_tab_data,
            )
            .debug_name("find view"),
        ))
        .style(|s| s.width_full().flex_grow(1.0)),
    ))
    .on_cleanup(move || {
        // batch(|| {
        // let editor = editor.get_untracked();
        // editor.scope.dispose();
        // ?
        // editor.cancel_completion();
        // editor.cancel_inline_completion();
        // if editors.contains_untracked(editor.id()) {
        //     // editor still exist, so it might be moved to a different
        // editor     // tab
        //     return;
        // }
        // let doc = editor.doc();
        // // ?
        // // editor.scope.dispose();

        // let scratch_doc_name = if let DocContent::Scratch { name, .. } =
        //     doc.content.get_untracked()
        // {
        //     Some(name.to_string())
        // } else {
        //     None
        // };
        // if let Some(name) = scratch_doc_name {
        //     if !scratch_docs
        //         .with_untracked(|scratch_docs|
        // scratch_docs.contains_key(&name))     {
        //         // ?
        //         // doc.scope.dispose();
        //     }
        // }
        // });
    })
    .style(|s| s.flex_col().absolute().size_pct(100.0, 100.0))
    .debug_name("Editor Container")
}

// fn editor_gutter_breakpoint_view(
//     i: VisualLineInfo,
//     doc: DocSignal,
//     daps: RwSignal<im::HashMap<DapId, DapData>>,
//     breakpoints: RwSignal<BTreeMap<PathBuf, BTreeMap<usize,
// LapceBreakpoint>>>,     common: Rc<CommonData>,
//     icon_padding: f32,
// ) -> impl View {
//     let hovered = create_rw_signal(false);
//     let config = common.config;
//     container(
//         svg(move || config.with_ui_svg(LapceIcons::DEBUG_BREAKPOINT)).style(
//             move |s| {
//                 let config = config.get();
//                 let size = config.ui.icon_size() as f32 + 2.0;
//                 s.size(size, size)
//                     .color(config.color(LapceColor::DEBUG_BREAKPOINT_HOVER))
//                     .apply_if(!hovered.get(), |s| s.hide())
//             },
//         ),
//     )
//     .on_click_stop(move |_| {
//         let doc = doc.get_untracked();
//         let offset = i.visual_line.origin_interval.start;
//         let line = i.visual_line.origin_line;
//         // let offset = doc.buffer.with_untracked(|b|
// b.offset_of_line(line));         log::info!("click breakpoint line={:?}", i);
//         if let Some(path) = doc.content.get_untracked().path() {
//             update_breakpoints(
//                 daps,
//                 common.proxy.clone(),
//                 breakpoints,
//                 crate::debug::BreakpointAction::Add { path, line, offset },
//             );
//             // let path_breakpoints = breakpoints
//             //     .try_update(|breakpoints| {
//             //         let breakpoints =
//             // breakpoints.entry(path.clone()).or_default();
//             //         if let std::collections::btree_map::Entry::Vacant(e) =
//             //             breakpoints.entry(line)
//             //         {
//             //             e.insert(LapceBreakpoint {
//             //                 id: None,
//             //                 verified: false,
//             //                 message: None,
//             //                 line,
//             //                 offset,
//             //                 dap_line: None,
//             //                 active: true,
//             //             });
//             //         } else {
//             //             let mut toggle_active = false;
//             //             if let Some(breakpint) =
// breakpoints.get_mut(&line) {             //                 if
// !breakpint.active {             //                     breakpint.active =
// true;             //                     toggle_active = true;
//             //                 }
//             //             }
//             //             if !toggle_active {
//             //                 breakpoints.remove(&line);
//             //             }
//             //         }
//             //         breakpoints.clone()
//             //     })
//             //     .unwrap();
//             // let source_breakpoints: Vec<SourceBreakpoint> =
// path_breakpoints             //     .iter()
//             //     .filter_map(|(_, b)| {
//             //         if b.active {
//             //             Some(SourceBreakpoint {
//             //                 line: b.line + 1,
//             //                 column: None,
//             //                 condition: None,
//             //                 hit_condition: None,
//             //                 log_message: None,
//             //             })
//             //         } else {
//             //             None
//             //         }
//             //     })
//             //     .collect();
//             // let daps: Vec<DapId> =
//             //     daps.with_untracked(|daps|
// daps.keys().cloned().collect());             // for dap_id in daps {
//             //     common.proxy.dap_set_breakpoints(
//             //         dap_id,
//             //         path.to_path_buf(),
//             //         source_breakpoints.clone(),
//             //     );
//             // }
//         }
//     })
//     .on_event_stop(EventListener::PointerEnter, move |_| {
//         hovered.set(true);
//     })
//     .on_event_stop(EventListener::PointerLeave, move |_| {
//         hovered.set(false);
//     })
//     .style(move |s| {
//         let config = config.get();
//         s.width(config.ui.icon_size() as f32 + icon_padding * 2.0)
//             .height(config.editor.line_height() as f32)
//             .justify_center()
//             .items_center()
//             .cursor(CursorStyle::Pointer)
//     })
// }

// fn editor_gutter_breakpoints(
//     window_tab_data: WindowTabData,
//     e_data: RwSignal<EditorData>,
//     icon_padding: f32,
// ) -> impl View {
//     let breakpoints = window_tab_data.terminal.debug.breakpoints;
//     let (doc, config) = e_data
//         .with_untracked(|e| (e.doc_signal(), e.common.config));
//
//     clip(
//         dyn_stack(
//             move || {
//                 let e_data = e_data.get();
//                 let doc = e_data.doc_signal().get();
//                 let content = doc.content.get();
//                 let breakpoints = if let Some(path) = content.path() {
//                     breakpoints
//                         .with(|b| b.get(path).cloned())
//                         .unwrap_or_default()
//                 } else {
//                     Default::default()
//                 };
//                 breakpoints.into_iter()
//             },
//             move |(line, b)| (*line, b.active),
//             move |(line, breakpoint)| {
//                 let active = breakpoint.active;
//                 container(
//                     svg(move || {
//                         config.with_ui_svg(LapceIcons::DEBUG_BREAKPOINT)
//                     })
//                     .style(move |s| {
//                         let config = config.get();
//                         let size = config.ui.icon_size() as f32 + 2.0;
//                         let color = if active {
//                             LapceColor::DEBUG_BREAKPOINT
//                         } else {
//                             LapceColor::EDITOR_DIM
//                         };
//                         let color = config.color(color);
//                         s.size(size, size).color(color)
//                     }),
//                 )
//                 .style(move |s| {
//                     // todo improve
//                     let config = config.get();
//                     let screen_lines = doc
//                         .get()
//                         .lines
//                         .with_untracked(|x| x.signal_screen_lines())
//                         .get_untracked();
//                     let line_y = screen_lines
//                         .visual_line_info_of_origin_line(line)
//                         .map(|l| l.folded_line_y)
//                         .unwrap_or_default();
//                     s.absolute()
//                         .width(config.ui.icon_size() as f32 + icon_padding *
// 2.0)                         .height(config.editor.line_height() as f32)
//                         .justify_center()
//                         .items_center()
//                         .margin_top(line_y as f32 - screen_lines.base.y0 as
// f32)                 })
//             },
//         )
//         .style(|s| s.absolute().size_pct(100.0, 100.0)),
//     )
//     .style(move |s| {
//         s.absolute().size_pct(100.0, 100.0)
//         // .background(config.with_color(LapceColor::EDITOR_BACKGROUND))
//     })
//     .debug_name("Breakpoint Clip")
// }

// fn editor_gutter_code_lens_view(
//     window_tab_data: WindowTabData,
//     line: usize,
//     lens: (PluginId, usize, im::Vector<CodeLens>),
//     screen_lines: ReadSignal<ScreenLines>,
//     viewport: ReadSignal<Rect>,
//     icon_padding: f32,
// ) -> impl View {
//     let config = window_tab_data.common.config;
//     let view = container(svg(move ||
// config.with_ui_svg(LapceIcons::START)).style(         move |s| {
//             let config = config.get();
//             let size = config.ui.icon_size() as f32;
//             s.size(size, size)
//                 .color(config.color(LapceColor::LAPCE_ICON_ACTIVE))
//         },
//     ))
//     .style(move |s| {
//         let config = config.get();
//         s.padding(4.0)
//             .border_radius(6.0)
//             .hover(|s| {
//                 s.cursor(CursorStyle::Pointer)
//
// .background(config.color(LapceColor::PANEL_HOVERED_BACKGROUND))
// })             .active(|s| {
//                 s.background(
//
// config.color(LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND),                 )
//             })
//     })
//     .on_click_stop({
//         move |_| {
//             let (plugin_id, offset, lens) = lens.clone();
//             window_tab_data.show_code_lens(true, plugin_id, offset, lens);
//         }
//     });
//     container(view).style(move |s| {
//         let line_y = screen_lines.with(|s| {
//             s.visual_line_info_of_origin_line(line)
//                 .map(|x| x.folded_line_y)
//                 .unwrap_or(-100.0)
//         });
//         // let line_y = line_info.clone().map(|l| l.y).unwrap_or(-100.0);
//         let rect = viewport.get();
//         let config = config.get();
//         let icon_size = config.ui.icon_size();
//         let width = icon_size as f32 + icon_padding * 2.0;
//         s.absolute()
//             .width(width)
//             .height(config.editor.line_height() as f32)
//             .justify_center()
//             .items_center()
//             .margin_top(line_y as f32 - rect.y0 as f32)
//     })
// }

fn editor_gutter_folding_view(
    window_tab_data: WindowWorkspaceData,
    folding_display_item: FoldingDisplayItem,
) -> impl View {
    let config = window_tab_data.common.config;
    let line_height = window_tab_data.common.ui_line_height;

    let view = container(
        svg(move || {
            let icon_str = match folding_display_item.ty {
                FoldingDisplayType::UnfoldStart => LapceIcons::EDITOR_FOLDING_START,
                FoldingDisplayType::Folded => LapceIcons::EDITOR_FOLDING_FOLDED,
                FoldingDisplayType::UnfoldEnd => LapceIcons::EDITOR_FOLDING_END,
            };
            config.with_ui_svg(icon_str)
        })
        .style(move |s| {
            let (active, icon_size) = config.signal(|config| {
                (
                    config.color(LapceColor::LAPCE_ICON_ACTIVE),
                    config.ui.icon_size.signal(),
                )
            });

            let size = icon_size.get() as f32;
            s.size(size, size)
                .set_style_value(
                    SvgColor,
                    (Some(Brush::Solid(Color::from_rgba8(0, 0, 0, 80)))).into(),
                )
                .hover(|s| {
                    s.cursor(CursorStyle::Pointer)
                        .set_style_value(
                            SvgColor,
                            (Some(Brush::Solid(Color::BLACK))).into(),
                        )
                        .color(active.get())
                })
        }),
    )
    .style(move |s| s.hover(|s| s.cursor(CursorStyle::Pointer)));
    container(view).style(move |s| {
        let line_height = line_height.get();
        s.absolute()
            .height(line_height)
            .justify_center()
            .items_center()
            .margin_top(folding_display_item.y as f32)
    })
}

// fn editor_gutter_code_lens(
//     window_tab_data: WindowTabData,
//     doc: DocSignal,
//     screen_lines: ReadSignal<ScreenLines>,
//     viewport: ReadSignal<Rect>,
//     icon_padding: f32,
// ) -> impl View {
//     let config = window_tab_data.common.config;
//
//     dyn_stack(
//         move || {
//             let doc = doc.get();
//             doc.code_lens.get()
//         },
//         move |(line, _)| (*line, doc.with_untracked(|doc| doc.rev())),
//         move |(line, lens)| {
//             editor_gutter_code_lens_view(
//                 window_tab_data.clone(),
//                 line,
//                 lens,
//                 screen_lines,
//                 viewport,
//                 icon_padding,
//             )
//         },
//     )
//         .style(move |s| {
//             let config = config.get();
//             let width = config.ui.icon_size() as f32 + icon_padding * 2.0;
//             s.absolute()
//                 .width(width)
//                 .height_full()
//                 .margin_left(width - 8.0)
//         })
//         .debug_name("CodeLens Stack")
// }

fn editor_gutter_folding_range(
    window_tab_data: WindowWorkspaceData,
    e_data: EditorData,
) -> impl View {
    let config = window_tab_data.common.config;
    dyn_stack(
        move || e_data.folding_display_item.get(),
        move |item| *item,
        move |item| {
            editor_gutter_folding_view(window_tab_data.clone(), item).on_click_stop(
                {
                    let lines = e_data.doc_signal().with_untracked(|x| x.lines);
                    move |_| {
                        lines.update(|x| {
                            if let Err(err) = x.update_folding_ranges(item.into()) {
                                error!("{:?}", err);
                            }
                        });
                    }
                },
            )
        },
    )
    .style(move |s| {
        let icon_size = config.signal(|config| config.ui.icon_size.signal()).get();
        let width = icon_size as f32;
        s.width(width).height_full().margin_left(width / -2.0)
    })
    .debug_name("Folding Range Stack")
}

fn editor_breadcrumbs(
    workspace: Arc<LapceWorkspace>,
    e_data: EditorData,
    config: WithLapceConfig,
) -> impl View {
    let doc = e_data.doc_signal();
    let doc_path = create_memo(move |_| {
        let doc = doc.get();
        let content = doc.content.get();
        if let DocContent::History(history) = &content {
            Some(history.path.clone())
        } else {
            content.path().cloned()
        }
    });
    container(
        scroll(
            stack((
                {
                    let workspace = workspace.clone();
                    dyn_stack(
                        move || {
                            let full_path = doc_path.get().unwrap_or_default();
                            let mut path = full_path;
                            if let Some(workspace_path) = workspace.clone().path() {
                                path = path
                                    .strip_prefix(workspace_path)
                                    .unwrap_or(&path)
                                    .to_path_buf();
                            }
                            path.ancestors()
                                .filter_map(|path| {
                                    Some(
                                        path.file_name()?
                                            .to_string_lossy()
                                            .into_owned(),
                                    )
                                })
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .enumerate()
                        },
                        |(i, section)| (*i, section.to_string()),
                        move |(i, section)| {
                            stack((
                                svg(move || {
                                    config.with_ui_svg(
                                        LapceIcons::BREADCRUMB_SEPARATOR,
                                    )
                                })
                                .style(move |s| {
                                    let (active, icon_size) =
                                        config.signal(|config| {
                                            (
                                                config.color(
                                                    LapceColor::LAPCE_ICON_ACTIVE,
                                                ),
                                                config.ui.icon_size.signal(),
                                            )
                                        });

                                    let size = icon_size.get() as f32;
                                    s.apply_if(i == 0, |s| s.hide())
                                        .size(size, size)
                                        .color(active.get())
                                }),
                                label(move || section.clone())
                                    .style(move |s| s.selectable(false)),
                            ))
                            .style(|s| s.items_center())
                        },
                    )
                    .style(|s| s.padding_horiz(10.0))
                },
                label(move || {
                    let doc = doc.get();
                    if let DocContent::History(history) = doc.content.get() {
                        format!("({})", history.version)
                    } else {
                        "".to_string()
                    }
                })
                .style(move |s| {
                    let doc = doc.get();
                    let is_history = doc.content.with_untracked(|content| {
                        matches!(content, DocContent::History(_))
                    });

                    s.padding_right(10.0).apply_if(!is_history, |s| s.hide())
                }),
            ))
            .style(|s| s.items_center()),
        )
        .scroll_to(move || {
            doc.track();
            Some(Point::new(3000.0, 0.0))
        })
        .scroll_style(|s| s.hide_bars(true))
        .style(move |s| {
            s.absolute()
                .size_pct(100.0, 100.0)
                .border_bottom(1.0)
                .border_color(config.with_color(LapceColor::LAPCE_BORDER))
                .items_center()
        }),
    )
    .style(move |s| {
        let (show_bread_crumbs, line_height) = config.signal(|config| {
            (
                config.editor.show_bread_crumbs.signal(),
                config.editor.line_height.signal(),
            )
        });
        s.items_center()
            .width_pct(100.0)
            .height(line_height.get() as f32)
            .apply_if(doc_path.get().is_none(), |s| s.hide())
            .apply_if(!show_bread_crumbs.get(), |s| s.hide())
    })
    .debug_name("Editor BreadCrumbs")
}

pub fn count_rect(
    changes: &[DiffResult],
    index: usize,
    right_editor: &EditorData,
) -> Result<()> {
    if let Some(change) = changes.get(index) {
        // log::error!("{change:?}");
        let line_index = match change {
            DiffResult::Empty { lines } => {
                right_editor.visual_lines.with_untracked(|x| {
                    for (index, line) in x.iter().enumerate() {
                        if let LineTy::DiffEmpty { change_line_start } = line.line_ty
                            && change_line_start == lines.start
                        {
                            return Ok(index);
                        }
                    }
                    bail!("count_rect Empty {lines:?} {x:?}");
                })
            },
            DiffResult::Changed { lines } => {
                right_editor.visual_lines.with_untracked(|x| {
                    for (index, line) in x.iter().enumerate() {
                        if let LineTy::OriginText {
                            line_range_inclusive,
                            ..
                        } = &line.line_ty
                            && line_range_inclusive.contains(&lines.start)
                        {
                            return Ok(index);
                        }
                    }
                    bail!("count_rect Changed {lines:?} {x:?}");
                })
            },
        }?;
        let line_height = right_editor.line_height(0);
        let y = ((line_index.max(3) - 3) * line_height) as f64;
        let y1 = ((line_index + 3) * line_height) as f64;
        let rect = Rect::new(0.0, y, 0.0, y1);
        log::debug!("\n\nindex={index} rect={rect:?} len={} \n\n", changes.len());
        right_editor.ensure_visible.set(rect);
        Ok(())
    } else {
        bail!("changes len {} index {}", changes.len(), index);
    }
}

pub fn editor_diff_header(
    config: WithLapceConfig,
    right_editor: EditorData,
) -> impl View {
    let index = create_rw_signal(0usize);
    let right_editor_svg = right_editor.clone();
    let view = h_stack((
        common_svg(config, None, LapceIcons::FOLD_UP)
            .style(|x| x.padding_horiz(15.0))
            .on_click_stop(move |_| {
                if let EditorViewKind::Diff { changes, .. } =
                    right_editor_svg.kind_read().get_untracked()
                {
                    let Some(index) = index.try_update(|x| {
                        if *x == 0 {
                            *x = changes.len() - 1;
                        } else {
                            x.sub_assign(1);
                        }
                        *x
                    }) else {
                        return;
                    };
                    if let Err(err) = count_rect(&changes, index, &right_editor_svg)
                    {
                        error!("{err}");
                    }
                }
            }),
        common_svg(config, None, LapceIcons::FOLD_DOWN).on_click_stop(move |_| {
            if let EditorViewKind::Diff { changes, .. } =
                right_editor.kind_read().get_untracked()
            {
                let Some(index) = index.try_update(|x| {
                    x.add_assign(1);
                    if *x >= changes.len() {
                        *x = 0;
                    }
                    *x
                }) else {
                    return;
                };
                if let Err(err) = count_rect(&changes, index, &right_editor) {
                    error!("{err}");
                }
            }
        }),
    ));
    view.style(|x| x.height(30.))
}

fn editor_content(
    editor: EditorData,
    debug_breakline: Memo<Option<(usize, PathBuf)>>,
    is_active: impl Fn(bool) -> bool + 'static + Copy,
) -> impl View {
    let (cursor, scroll_delta, scroll_to, window_origin, scope) = (
        editor.cursor().read_only(),
        editor.scroll_delta.read_only(),
        editor.scroll_to,
        editor.window_origin(),
        editor.scope,
    );

    // let config = editor.common.config;

    let current_scroll = scope.create_rw_signal(Rect::ZERO);

    {
        let editor = editor.clone();
        scope.create_effect(move |_| {
            editor.doc_signal().track();
            let cursor = cursor.get();
            let offset = cursor.offset();
            let offset_line_from_top = editor
                .common
                .offset_line_from_top
                .try_update(|x| x.take())
                .flatten();
            let line_height = editor
                .common
                .config
                .signal(|x| x.editor.line_height.signal())
                .get() as f64;

            let Ok(mut origin_point) =
                editor.doc_signal().with(|x| x.lines).with_untracked(|x| {
                    x.cursor_position_of_buffer_offset(offset, cursor.affinity)
                })
            else {
                editor.ensure_visible.set(current_scroll.get_untracked());
                return;
            };

            let ensure_visiable = if let Some(height) = offset_line_from_top {
                // from jump
                // let height = offset_line_from_top.unwrap_or(5) as f64 *
                // line_height;
                let scroll = current_scroll.get_untracked();
                let line_height_2 = line_height * 2.0;
                // avoid to at boundary
                let height = height
                    .min(scroll.height() - line_height_2)
                    .max(line_height_2);
                let backup_point = origin_point;
                let rect = if scroll != Rect::ZERO {
                    let y0 = (origin_point.y - height).max(0.0);
                    let y1 = y0 + scroll.height();
                    // avoid to be hidden
                    // if y1 - origin_point.y < line_height_2 {
                    //     y1 += line_height_2;
                    //     y0 = (y0 - line_height_2).max(0.0);
                    // }
                    Rect::new(origin_point.x, y0, origin_point.x, y1)
                } else {
                    log::error!("ensure_visible viewport is zero",);
                    origin_point.y += height;
                    Rect::from_origin_size(
                        origin_point,
                        (line_height_2, line_height_2),
                    )
                };
                log::debug!(
                    "offset_line_from_top viewport={scroll:?} target={rect:?} \
                     backup_point={backup_point:?} offset={offset} \
                     offset_line_from_top={offset_line_from_top:?} height={height} ",
                );
                rect
            } else {
                // from click maybe
                // let viewport = editor.viewport.get_untracked().size();
                // log::warn!("viewport={viewport:?}");
                //
                // Rect::from_center_size(
                //     origin_point,
                //     (viewport.width, viewport.height),
                // )
                Rect::from_center_size(origin_point, (2.0, line_height * 2.0))
            };
            log::debug!(
                "ensure_visible offset={offset} \
                 offset_line_from_top={offset_line_from_top:?} origin_point.y={} \
                 ensure_visiable={}-{}",
                origin_point.y,
                ensure_visiable.y0,
                ensure_visiable.y1
            );
            editor.ensure_visible.set(ensure_visiable);
        });
    }

    let editor_scroll = editor.clone();
    let editor_ensure = editor.clone();

    scroll({
        let editor_content_view =
            editor_view(editor.clone(), debug_breakline, is_active, "editor").style(
                move |s| {
                    s.absolute()
                        .margin_left(1.0)
                        .min_size_full()
                        .cursor(CursorStyle::Text)
                },
            );
        let id = editor_content_view.id();
        editor.editor_view_id.set(Some(id));

        let editor_down = editor.clone();
        let editor_move = editor.clone();
        let editor_up = editor.clone();
        let editor_left = editor.clone();

        editor_content_view
            // .on_event_cont(EventListener::FocusGained, move |_| {
            //     editor.editor_view_focused.notify();
            // })
            // .on_event_cont(EventListener::FocusLost, move |_| {
            //     editor2.editor_view_focus_lost.notify();
            // })
            // 不能on_event_stop。否则会导致diff的左侧光标无法渲染
            .on_event_cont(EventListener::PointerDown, move |event| {
                if let Event::PointerDown(pointer_event) = event {
                    id.request_active();
                    editor_down.pointer_down(pointer_event);
                }
            })
            .on_event_stop(EventListener::PointerMove, move |event| {
                if let Event::PointerMove(pointer_event) = event {
                    editor_move.pointer_move(pointer_event);
                }
            })
            .on_event_stop(EventListener::PointerUp, move |event| {
                if let Event::PointerUp(pointer_event) = event {
                    editor_up.pointer_up(pointer_event);
                }
            })
            .on_event_stop(EventListener::PointerLeave, move |event| {
                if let Event::PointerLeave = event {
                    editor_left.pointer_leave();
                }
            })
            .keyboard_navigable()
    })
    .on_move(move |point| {
        window_origin.set(point);
    })
    .on_resize(|_size| {
        // log::info!("on_resize rect={size:?}");
    })
    .on_scroll(move |rect| {
        log::debug!("on_scroll rect{rect:?}");
        if rect.y0 != current_scroll.get_untracked().y0 {
            editor_scroll.cancel_completion();
            editor_scroll.cancel_inline_completion();
        }
        editor_scroll.viewport.set(rect);
        editor_scroll.common.hover.active.set(false);
        current_scroll.set(rect);
    })
    .scroll_to(move || {
        scroll_to.get().map(|s| {
            // log::info!("scroll_to {:?}", s);
            s.to_point()
        })
    })
    .scroll_delta(move || scroll_delta.get())
    .ensure_visible(move || {
        let ensure_visible = editor_ensure.ensure_visible.get();
        let current_scroll = current_scroll.get_untracked();
        log::debug!("ensure_visible rect{ensure_visible:?} {current_scroll:?}");
        ensure_visible
    })
    .style(move |s| {
        s.size_full().set(PropagatePointerWheel, false)
        // .border_left(1.0)
        // .border_color(config.with_color(LapceColor::LAPCE_BORDER))
        // .border_right(1.0)
    })
    .keyboard_navigable()
    .debug_name("Editor Content")
}

fn search_editor_view(
    // find_editor: EditorData,
    // find_focus: RwSignal<bool>,
    // is_active: impl Fn(bool) -> bool + 'static + Copy,
    // replace_focus: RwSignal<bool>,
    common: Rc<CommonData>,
    find_str: RwSignal<String>,
    find_view_id: RwSignal<Option<ViewId>>,
    window_tab_data: WindowWorkspaceData,
) -> impl View {
    let config = common.config;

    let case_matching = common.find.case_matching;
    let whole_word = common.find.whole_words;
    let is_regex = common.find.is_regex;
    // let visual = common.find.visual;

    // let focus_trace = common.scope.create_trigger();

    let find_view = text_input(find_str)
        .keyboard_navigable()
        .on_event_stop(EventListener::KeyDown, move |event| {
            if let Event::KeyDown(_key_event) = event {
                window_tab_data.key_down(_key_event);
            }
        })
        .style(|s| s.width_pct(100.0))
        .debug_name("find_view_input");

    find_view_id.set(Some(find_view.id()));

    stack((
        find_view,
        clickable_icon(
            || LapceIcons::SEARCH_CASE_SENSITIVE,
            move || {
                let new = match case_matching.get_untracked() {
                    CaseMatching::Exact => CaseMatching::CaseInsensitive,
                    CaseMatching::CaseInsensitive => CaseMatching::Exact,
                };
                case_matching.set(new);
            },
            move || case_matching.get() == CaseMatching::Exact,
            || false,
            || "Case Sensitive",
            config,
        )
        .style(|s| s.padding_vert(1.0)),
        clickable_icon(
            || LapceIcons::SEARCH_WHOLE_WORD,
            move || {
                whole_word.update(|whole_word| {
                    *whole_word = !*whole_word;
                });
            },
            move || whole_word.get(),
            || false,
            || "Whole Word",
            config,
        )
        .style(|s| s.padding_left(6.0)),
        clickable_icon(
            || LapceIcons::SEARCH_REGEX,
            move || {
                is_regex.update(|is_regex| {
                    *is_regex = !*is_regex;
                });
            },
            move || is_regex.get(),
            || false,
            || "Use Regex",
            config,
        )
        .style(|s| s.padding_horiz(6.0)),
    ))
    .style(move |s| {
        let (border_color, bg) = config.signal(|config| {
            (
                config.color(LapceColor::LAPCE_BORDER),
                config.color(LapceColor::EDITOR_BACKGROUND),
            )
        });
        s.width(200.0)
            .height(25.0)
            .items_center()
            .border(1.0)
            .border_radius(6.0)
            .border_color(border_color.get())
            .background(bg.get())
    })
}

fn replace_editor_view(
    // replace_editor: EditorData,
    // replace_active: RwSignal<bool>,
    // replace_focus: RwSignal<bool>,
    // is_active: impl Fn(bool) -> bool + 'static + Copy,
    // find_focus: RwSignal<bool>,
    window_tab_data: WindowWorkspaceData,
    common: Rc<CommonData>,
    replace_str: RwSignal<String>,
) -> impl View {
    // let config = replace_editor.common.config;
    let config = common.config;
    // let visual = replace_editor.common.find.visual;

    let replace_view = text_input(replace_str)
        .keyboard_navigable()
        .on_event_stop(EventListener::KeyDown, move |event| {
            if let Event::KeyDown(_key_event) = event {
                window_tab_data.key_down(_key_event);
            }
        })
        .style(|s| s.width_pct(100.0))
        .debug_name("replace_str_view_input");

    stack((
        replace_view,
        empty().style(move |s| {
            let size = config.with_icon_size() as f32 + 10.0;
            s.size(0.0, size).padding_vert(4.0)
        }),
    ))
    .style(move |s| {
        let (border_color, bg) = config.signal(|config| {
            (
                config.color(LapceColor::LAPCE_BORDER),
                config.color(LapceColor::EDITOR_BACKGROUND),
            )
        });
        s.width(200.0)
            .height(25.0)
            .items_center()
            .border(1.0)
            .border_radius(6.0)
            .border_color(border_color.get())
            .background(bg.get())
    })
}

fn find_view(
    editor: EditorData,
    replace_active: RwSignal<bool>,
    common: Rc<CommonData>,
    find_str: RwSignal<String>,
    find_view_id: RwSignal<Option<ViewId>>,
    replace_str: RwSignal<String>,
    window_tab_data: WindowWorkspaceData,
) -> impl View {
    // let common = find_editor.common.clone();
    let config = common.config;
    let find_visual = common.find.visual;
    let focus = common.focus;
    let window_tab_data_replace = window_tab_data.clone();

    let find_pos = create_memo({
        let editor = editor.clone();
        move |_| {
            let visual = find_visual.get();
            if !visual {
                return (0, 0);
            }
            let cursor = editor.cursor();
            let offset = cursor.with(|cursor| cursor.offset());
            let occurrences = editor.doc_signal().get().find_result.occurrences;
            occurrences.with(|occurrences| {
                for (i, region) in occurrences.regions().iter().enumerate() {
                    if offset <= region.max() {
                        return (i + 1, occurrences.regions().len());
                    }
                }
                (occurrences.regions().len(), occurrences.regions().len())
            })
        }
    });

    let editor_back = editor.clone();
    let editor_forward = editor.clone();

    container(
        stack((
            stack((
                clickable_icon(
                    move || {
                        if replace_active.get() {
                            LapceIcons::ITEM_OPENED
                        } else {
                            LapceIcons::ITEM_CLOSED
                        }
                    },
                    move || {
                        replace_active.update(|active| *active = !*active);
                    },
                    move || false,
                    || false,
                    || "Toggle Replace",
                    config,
                )
                .style(|s| s.padding_horiz(6.0)),
                search_editor_view(
                    common.clone(),
                    find_str,
                    find_view_id,
                    window_tab_data,
                ),
                label(move || {
                    let (current, all) = find_pos.get();
                    if all == 0 {
                        "No Results".to_string()
                    } else {
                        format!("{current} of {all}")
                    }
                })
                .style(|s| s.margin_left(6.0).min_width(70.0)),
                clickable_icon(
                    || LapceIcons::SEARCH_BACKWARD,
                    move || {
                        editor_back.search_backward(Modifiers::empty());
                    },
                    move || false,
                    || false,
                    || "Previous Match",
                    config,
                )
                .style(|s| s.padding_left(6.0)),
                clickable_icon(
                    || LapceIcons::SEARCH_FORWARD,
                    move || {
                        editor_forward.search_forward(Modifiers::empty());
                    },
                    move || false,
                    || false,
                    || "Next Match",
                    config,
                )
                .style(|s| s.padding_left(6.0)),
                clickable_icon(
                    || LapceIcons::CLOSE,
                    {
                        let editor = editor.clone();
                        move || {
                            editor.clear_search();
                        }
                    },
                    move || false,
                    || false,
                    || "Close",
                    config,
                )
                .style(|s| s.padding_horiz(6.0)),
            ))
            .style(|s| s.items_center()),
            stack((
                empty().style(move |s| {
                    let width = config.with_icon_size() as f32 + 10.0 + 6.0 * 2.0;
                    s.width(width)
                }),
                replace_editor_view(
                    window_tab_data_replace,
                    common.clone(),
                    replace_str,
                ),
                clickable_icon(
                    || LapceIcons::SEARCH_REPLACE,
                    {
                        let editor = editor.clone();
                        move || {
                            let text = replace_str.get_untracked();
                            editor.replace_next(&text);
                        }
                    },
                    move || false,
                    || false,
                    || "Replace Next",
                    config,
                )
                .style(|s| s.padding_left(6.0)),
                clickable_icon(
                    || LapceIcons::SEARCH_REPLACE_ALL,
                    {
                        let editor = editor.clone();
                        move || {
                            let text = replace_str.get_untracked();
                            editor.replace_all(&text);
                        }
                    },
                    move || false,
                    || false,
                    || "Replace All",
                    config,
                )
                .style(|s| s.padding_left(6.0)),
            ))
            .style(move |s| {
                s.items_center()
                    .margin_top(4.0)
                    .apply_if(!replace_active.get(), |s| s.hide())
            }),
        ))
        .style(move |s| {
            let (border_color, bg) = config.signal(|config| {
                (
                    config.color(LapceColor::LAPCE_BORDER),
                    config.color(LapceColor::PANEL_BACKGROUND),
                )
            });
            s.margin_right(50.0)
                .background(bg.get())
                .border_radius(6.0)
                .border(1.0)
                .border_color(border_color.get())
                .padding_vert(4.0)
                .cursor(CursorStyle::Default)
                .flex_col()
        })
        .on_event_stop(EventListener::PointerDown, move |_| {
            // Shift the editor tab focus to the editor the find search is attached
            // to So that if you have two tabs open side-by-side (and
            // thus two find views), clicking on one will shift the focus
            // to the editor it's attached to
            if let Some(editor_tab_id) = editor.editor_tab_id.get_untracked() {
                editor
                    .common
                    .internal_command
                    .send(InternalCommand::FocusEditorTab { editor_tab_id });
            }
            focus.set(Focus::Workbench);
            // Request focus on the app view, as our current method of dispatching
            // pointer events is from the app_view to the actual editor.
            // That's also why this stops the pointer event is stopped
            // here, as otherwise our default handling would make it go through to
            // the editor.
            common
                .window_common
                .app_view_id
                .get_untracked()
                .request_focus();
        }),
    )
    .style(move |s| {
        s.absolute()
            .margin_top(-1.0)
            .width_pct(100.0)
            .justify_end()
            .apply_if(!find_visual.get(), |s| s.hide())
    })
}

/// Iterator over (len, color, modified) for each change in the diff
fn changes_color_iter(
    changes: &im::Vector<DiffLines>,
    added: Color,
    modified_color: Color,
    removed: Color,
) -> impl Iterator<Item = (usize, Option<Color>, bool)> + '_ {
    let mut last_change = None;
    changes.iter().map(move |change| {
        let len = match change {
            DiffLines::Left(_range) => 0,
            DiffLines::Both(info) => info.right.len(),
            DiffLines::Right(range) => range.len(),
        };
        let mut modified = false;
        let color = match change {
            DiffLines::Left(_range) => Some(removed),
            DiffLines::Right(_range) => {
                if let Some(DiffLines::Left(_)) = last_change.as_ref() {
                    modified = true;
                }
                if modified {
                    Some(modified_color)
                } else {
                    Some(added)
                }
            },
            _ => None,
        };

        last_change = Some(change.clone());

        (len, color, modified)
    })
}

// TODO: both of the changes color functions could easily return iterators

/// Get the position and coloring information for over the entire current
/// [`ScreenLines`] Returns `(y, height_idx, removed, color)`
pub fn changes_colors_screen(
    editor: &EditorData,
    changes: im::Vector<DiffLines>,
    added: Color,
    modified_color: Color,
    removed: Color,
) -> Result<Vec<(f64, usize, bool, Color)>> {
    let Some((min, max)) = editor.screen_lines.with_untracked(|x| x.line_interval())
    else {
        return Ok(vec![]);
    };

    let mut line = 0;
    let mut colors = Vec::new();

    for (len, color, modified) in
        changes_color_iter(&changes, added, modified_color, removed)
    {
        let _pre_line = line;

        line += len;
        if line < min {
            continue;
        }

        if let Some(_color) = color
            && modified
        {
            colors.pop();
        }

        if line > max {
            break;
        }
    }

    Ok(colors)
}

// TODO: limit the visual line that changes are considered past to some
// reasonable number TODO(minor): This could be a `changes_colors_range` with
// some minor changes, but it isn't needed
/// Get the position and coloring information for over the entire current
/// [`ScreenLines`] Returns `(y, height_idx, removed, color)`
pub fn changes_colors_all(
    _config: &LapceConfig,
    _changes: im::Vector<DiffLines>,
) -> Vec<(f64, usize, bool, Color)> {
    // let line_height = config.editor.line_height();
    //
    // let mut line = 0;
    // let colors = Vec::new();
    // let mut vline_iter = ed.iter_vlines(false, VLine(0)).peekable();
    //
    // for (len, color, modified) in changes_color_iter(&changes, config) {
    //     let pre_line = line;
    //
    //     line += len;
    //
    //     // Skip over all vlines that are before the current line
    //     vline_iter
    //         .by_ref()
    //         .peeking_take_while(|info| info.rvline.line < pre_line)
    //         .count();
    //
    //     if let Some(color) = color {
    //         if modified {
    //             colors.pop();
    //         }
    //
    //         // Find the info with a line == pre_line
    //         let Some(info) = vline_iter.peek() else {
    //             continue;
    //         };
    //
    //         let y = info.vline.get() * line_height;
    //         let end_line = info.rvline.line + len;
    //         let height = vline_iter
    //             .by_ref()
    //             .peeking_take_while(|info| info.rvline.line < end_line)
    //             .count();
    //         let removed = len == 0;
    //
    //         colors.push((y as f64, height, removed, color));
    //     }
    //
    //     if vline_iter.peek().is_none() {
    //         break;
    //     }
    // }

    // colors

    Vec::new()
}

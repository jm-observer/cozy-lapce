use std::{path::PathBuf, sync::Arc, time::SystemTime};

use alacritty_terminal::{
    grid::Dimensions,
    index::Side,
    selection::{Selection, SelectionType},
    term::{RenderableContent, cell::Flags, test::TermSize},
};
use doc::lines::{mode::Mode, register::Clipboard, text::SystemClipboard};
use floem::{
    Renderer, View, ViewId,
    context::{EventCx, PaintCx},
    event::{Event, EventPropagation},
    peniko::{
        Color,
        kurbo::{Point, Rect, Size},
    },
    pointer::PointerInputEvent,
    prelude::SignalUpdate,
    reactive::{Scope, SignalGet, SignalTrack, SignalWith, create_effect},
    text::{Attrs, AttrsList, FamilyOwned, TextLayout, Weight},
};
use lapce_core::{panel::PanelKind, workspace::LapceWorkspace};
use lapce_rpc::{proxy::ProxyRpcHandler, terminal::TermId};
use lsp_types::Position;
use regex::Regex;

use super::panel::TerminalPanelData;
use crate::{
    command::InternalCommand,
    common::call_back::find_file_call_back,
    config::{LapceConfig, WithLapceConfig, color::LapceColor},
    editor::location::{EditorLocation, EditorPosition},
    listener::Listener,
    terminal::data::TerminalData,
    window_workspace::Focus,
};

/// Threshold used for double_click/triple_click.
const CLICK_THRESHOLD: u128 = 400;

enum TerminalViewState {
    // Config,
    Focus(bool), // Raw(Arc<RwLock<RawTerminal>>),
}

struct TerminalLineContent<'a> {
    y:         f64,
    bg:        Vec<(usize, usize, Color)>,
    underline: Vec<(usize, usize, Color, f64)>,
    chars:     Vec<(char, Attrs<'a>, f64, f64)>,
    cursor:    Option<(char, f64)>,
}

pub struct TerminalView {
    id:                    ViewId,
    scope:                 Scope,
    term_id:               TermId,
    // mode: ReadSignal<Mode>,
    size:                  Size,
    is_focused:            bool,
    config:                WithLapceConfig,
    // run_config: ReadSignal<Option<RunDebugProcess>>,
    proxy:                 ProxyRpcHandler,
    // launch_error: RwSignal<Option<String>>,
    internal_command:      Listener<InternalCommand>,
    workspace:             Arc<LapceWorkspace>,
    hyper_regs:            Vec<Regex>,
    previous_mouse_action: MouseAction,
    current_mouse_action:  MouseAction,
    terminal_data:         TerminalData,
}

#[allow(clippy::too_many_arguments)]
pub fn terminal_view(
    term_id: TermId,
    // data: RwSignal<TerminalSignalData>,
    // mode: ReadSignal<Mode>,
    // run_config: ReadSignal<Option<RunDebugProcess>>,
    terminal_panel_data: TerminalPanelData,
    // launch_error: RwSignal<Option<String>>,
    internal_command: Listener<InternalCommand>,
    workspace: Arc<LapceWorkspace>,
    terminal: TerminalData,
) -> TerminalView {
    let id = ViewId::new();
    terminal.data.update(|x| x.view_id = Some(id));
    // let raw_data = terminal.data;
    // create_effect(move |_| {
    //     let raw = raw_data.with(|x| x.raw.clone());
    //     id.update_state(TerminalViewState::Raw(raw));
    // });
    let launch_error_data = terminal.data;

    create_effect(move |_| {
        launch_error_data.track();
        id.request_paint();
    });

    let config = terminal_panel_data.common.config;
    // create_effect(move |_| {
    //     config.with(|_c| {});
    //     id.update_state(TerminalViewState::Config);
    // });

    let proxy = terminal_panel_data.common.proxy.clone();

    create_effect(move |last| {
        let focus = terminal_panel_data.common.focus.get();

        let mut is_focused = false;
        if let Focus::Panel(PanelKind::Terminal) = focus {
            let tab = terminal_panel_data.active_tab_tracked();
            if let Some(terminal) = tab {
                is_focused = terminal.term_id == term_id;
            }
        }

        if last != Some(is_focused) {
            id.update_state(TerminalViewState::Focus(is_focused));
        }

        is_focused
    });

    // for rust
    let reg =
        regex::Regex::new(r"([:\.\w\\/-]+\.(rs|toml)?):(\d+)(:(\d+))?").unwrap();
    // let raw = raw_data.with_untracked(|x| x.raw.clone());

    TerminalView {
        scope: terminal.scope,
        terminal_data: terminal,
        id,
        term_id,
        config,
        proxy: proxy.proxy_rpc,
        size: Size::ZERO,
        is_focused: false,
        internal_command,
        workspace,
        hyper_regs: vec![reg],
        previous_mouse_action: Default::default(),
        current_mouse_action: Default::default(),
    }
}

impl TerminalView {
    fn char_size(&self) -> Size {
        let (font_family, font_size) = self.config.with_untracked(|config| {
            (
                config.terminal_font_family().to_string(),
                config.terminal_font_size(),
            )
        });
        let family: Vec<FamilyOwned> =
            FamilyOwned::parse_list(&font_family).collect();
        let attrs = Attrs::new().family(&family).font_size(font_size as f32);
        let attrs_list = AttrsList::new(attrs);
        let text_layout = TextLayout::new_with_text("W", attrs_list);
        text_layout.size()
    }

    fn terminal_size(&self) -> (usize, usize) {
        let line_height = self
            .config
            .with_untracked(|config| config.terminal_line_height() as f64);
        let char_width = self.char_size().width;
        let width = (self.size.width / char_width).floor() as usize;
        let height = (self.size.height / line_height).floor() as usize;
        (width.max(1), height.max(1))
    }

    fn click(&self, pos: Point) -> Option<()> {
        let raw = self.terminal_data.data.with_untracked(|x| x.raw.clone());
        let raw = raw.read();
        let position = self.get_terminal_point(pos);
        let start_point = raw.term.semantic_search_left(position);
        let end_point = raw.term.semantic_search_right(position);
        let mut selection =
            Selection::new(SelectionType::Simple, start_point, Side::Left);
        selection.update(end_point, Side::Right);
        selection.include_all();
        if let Some(selection) = selection.to_range(&raw.term) {
            let content = raw.term.bounds_to_string(selection.start, selection.end);

            if let Some(rs) =
                self.hyper_regs.iter().find_map(|x| x.captures(&content))
            {
                if let (Some(path), Some(line), col) =
                    (rs.get(1), rs.get(3), rs.get(5))
                {
                    let mut file: PathBuf = path.as_str().into();
                    let line = line.as_str().parse::<u32>().ok()?;
                    let col = col
                        .and_then(|x| x.as_str().parse::<u32>().ok())
                        .unwrap_or(0);

                    if !file.exists() {
                        log::info!("{file:?} is not exists");
                        file = self.workspace.path()?.join(file);
                    }
                    self.internal_command.send(InternalCommand::JumpToLocation {
                        location: EditorLocation {
                            path:               file,
                            position:           Some(EditorPosition::Position(
                                Position::new(
                                    line.saturating_sub(1),
                                    col.saturating_sub(1),
                                ),
                            )),
                            scroll_offset:      None,
                            ignore_unconfirmed: false,
                            same_editor_tab:    false,
                        },
                    });
                }
                return Some(());
            } else {
                self.proxy.find_file_from_log(
                    content,
                    find_file_call_back(self.scope, self.internal_command),
                );
            }
        }
        None
    }

    fn update_mouse_action_by_down(&mut self, mouse: &PointerInputEvent) {
        let mut next_action = MouseAction::None;
        match self.current_mouse_action {
            MouseAction::None
            | MouseAction::LeftDouble { .. }
            | MouseAction::LeftSelect { .. }
            | MouseAction::RightOnce { .. } => {
                if mouse.button.is_primary() {
                    next_action = MouseAction::LeftDown { pos: mouse.pos };
                } else if mouse.button.is_secondary() {
                    next_action = MouseAction::RightDown { pos: mouse.pos };
                }
            },
            MouseAction::LeftOnce { pos, time } => {
                let during_mills =
                    time.elapsed().map(|x| x.as_millis()).unwrap_or(u128::MAX);
                match (
                    mouse.button.is_primary(),
                    mouse.pos == pos,
                    during_mills < CLICK_THRESHOLD,
                ) {
                    (true, true, true) => {
                        next_action = MouseAction::LeftOnceAndDown { pos, time };
                    },
                    (true, _, _) => {
                        next_action = MouseAction::LeftDown { pos: mouse.pos };
                    },
                    _ => {},
                }
            },
            MouseAction::LeftOnceAndDown { .. }
            | MouseAction::LeftDown { .. }
            | MouseAction::RightDown { .. } => {},
        }
        self.current_mouse_action = next_action;
    }

    fn update_mouse_action_by_up(&mut self, mouse: &PointerInputEvent) {
        let mut next_action = MouseAction::None;
        match self.current_mouse_action {
            MouseAction::None => {},
            MouseAction::LeftDown { pos } => {
                match (mouse.button.is_primary(), mouse.pos == pos) {
                    (true, true) => {
                        next_action = MouseAction::LeftOnce {
                            pos,
                            time: SystemTime::now(),
                        }
                    },
                    (true, false) => {
                        next_action = MouseAction::LeftSelect {
                            start_pos: pos,
                            end_pos:   mouse.pos,
                        };
                    },
                    _ => {},
                }
            },
            MouseAction::LeftOnce { .. } => {},
            MouseAction::LeftSelect { .. } => {},
            MouseAction::LeftOnceAndDown { pos, time } => {
                let during_mills =
                    time.elapsed().map(|x| x.as_millis()).unwrap_or(u128::MAX);
                match (
                    mouse.button.is_primary(),
                    mouse.pos == pos,
                    during_mills < CLICK_THRESHOLD,
                ) {
                    (true, true, true) => {
                        next_action = MouseAction::LeftDouble { pos };
                    },
                    (true, true, false) => {
                        next_action = MouseAction::LeftOnce {
                            pos:  mouse.pos,
                            time: SystemTime::now(),
                        };
                    },
                    (true, false, _) => {
                        next_action = MouseAction::LeftSelect {
                            start_pos: pos,
                            end_pos:   mouse.pos,
                        };
                    },
                    _ => {},
                }
            },
            MouseAction::LeftDouble { .. } => {},
            MouseAction::RightDown { pos } => {
                if mouse.button.is_secondary() && mouse.pos == pos {
                    next_action = MouseAction::RightOnce { pos };
                }
            },
            MouseAction::RightOnce { .. } => {},
        }
        self.previous_mouse_action = self.current_mouse_action;
        self.current_mouse_action = next_action;
    }

    fn get_terminal_point(&self, pos: Point) -> alacritty_terminal::index::Point {
        let raw = self.terminal_data.data.with_untracked(|x| x.raw.clone());
        let raw = raw.read();
        let col = (pos.x / self.char_size().width) as usize;
        let line_no = pos.y as i32
            / (self
                .config
                .with(|config| config.terminal_line_height() as i32))
            - raw.term.grid().display_offset() as i32;
        alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(line_no),
            alacritty_terminal::index::Column(col),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_content(
        &self,
        cx: &mut PaintCx,
        content: RenderableContent,
        line_height: f64,
        char_size: Size,
        config: &LapceConfig,
        cursor: Color,
        error: Color,
        caret: Color,
        terminal_font_family: &str,
        terminal_font_size: usize,
        terminal_bg: Color,
    ) {
        let family: Vec<FamilyOwned> =
            FamilyOwned::parse_list(terminal_font_family).collect();
        let attrs = Attrs::new()
            .family(&family)
            .font_size(terminal_font_size as f32);

        let char_width = char_size.width;

        let cursor_point = &content.cursor.point;

        let mut line_content = TerminalLineContent {
            y:         0.0,
            bg:        Vec::new(),
            underline: Vec::new(),
            chars:     Vec::new(),
            cursor:    None,
        };
        for item in content.display_iter {
            let point = item.point;
            let cell = item.cell;
            let inverse = cell.flags.contains(Flags::INVERSE);

            let x = point.column.0 as f64 * char_width;
            let y =
                (point.line.0 as f64 + content.display_offset as f64) * line_height;
            let char_y = y + (line_height - char_size.height) / 2.0;
            if y != line_content.y {
                self.paint_line_content(
                    cx,
                    &line_content,
                    line_height,
                    char_width,
                    cursor,
                    error,
                    caret,
                );
                line_content.y = y;
                line_content.bg.clear();
                line_content.underline.clear();
                line_content.chars.clear();
                line_content.cursor = None;
            }

            let mut bg = config.terminal_get_color(&cell.bg, content.colors);
            let mut fg = config.terminal_get_color(&cell.fg, content.colors);
            if cell.flags.contains(Flags::DIM)
                || cell.flags.contains(Flags::DIM_BOLD)
            {
                fg = fg.multiply_alpha(0.66);
            }

            if inverse {
                std::mem::swap(&mut fg, &mut bg);
            }

            if terminal_bg != bg {
                let mut extend = false;
                if let Some((_, end, color)) = line_content.bg.last_mut()
                    && color == &bg
                    && *end == point.column.0
                {
                    *end += 1;
                    extend = true;
                }
                if !extend {
                    line_content
                        .bg
                        .push((point.column.0, point.column.0 + 1, bg));
                }
            }

            if cursor_point == &point {
                line_content.cursor = Some((cell.c, x));
            }

            let bold = cell.flags.contains(Flags::BOLD)
                || cell.flags.contains(Flags::DIM_BOLD);

            if &point == cursor_point && self.is_focused {
                fg = terminal_bg;
            }

            if cell.c != ' ' && cell.c != '\t' {
                let mut attrs = attrs.color(fg);
                if bold {
                    attrs = attrs.weight(Weight::BOLD);
                }
                line_content.chars.push((cell.c, attrs, x, char_y));
            }
        }
        self.paint_line_content(
            cx,
            &line_content,
            line_height,
            char_width,
            cursor,
            error,
            caret,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_line_content(
        &self,
        cx: &mut PaintCx,
        line_content: &TerminalLineContent,
        line_height: f64,
        char_width: f64,
        cursor: Color,
        error: Color,
        caret: Color,
    ) {
        for (start, end, bg) in &line_content.bg {
            let rect = Size::new(
                char_width * (end.saturating_sub(*start) as f64),
                line_height,
            )
            .to_rect()
            .with_origin(Point::new(*start as f64 * char_width, line_content.y));
            cx.fill(&rect, bg, 0.0);
        }

        for (start, end, fg, y) in &line_content.underline {
            let rect =
                Size::new(char_width * (end.saturating_sub(*start) as f64), 1.0)
                    .to_rect()
                    .with_origin(Point::new(*start as f64 * char_width, y - 1.0));
            cx.fill(&rect, fg, 0.0);
        }

        if let Some((_, x)) = line_content.cursor {
            let rect = Size::new(2.0, line_height)
                .to_rect()
                .with_origin(Point::new(x, line_content.y));

            let (mode, stopped) = self.terminal_data.data.with_untracked(|x| {
                (
                    x.mode,
                    x.run_debug.as_ref().map(|r| r.stopped).unwrap_or(false),
                )
            });
            let cursor_color = if mode == Mode::Terminal {
                if stopped { error } else { cursor }
            } else {
                caret
            };
            let hide_cursor = self
                .terminal_data
                .common
                .window_common
                .hide_cursor
                .get_untracked();
            // info!("hide_cursor {}, self.is_focused={}", hide_cursor,
            // self.is_focused);
            if self.is_focused && !hide_cursor {
                cx.fill(&rect, cursor_color, 0.0);
                // } else {
                //     cx.stroke(&rect, cursor_color, &Stroke::new(1.0));
            }
        }

        for (char, attr, x, y) in &line_content.chars {
            let text_layout =
                TextLayout::new_with_text(&char.to_string(), AttrsList::new(*attr));
            cx.draw_text_with_layout(text_layout.layout_runs(), Point::new(*x, *y));
        }
    }
}

impl View for TerminalView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn event_before_children(
        &mut self,
        _cx: &mut EventCx,
        event: &Event,
    ) -> EventPropagation {
        let raw = self.terminal_data.data.with_untracked(|x| x.raw.clone());
        match event {
            Event::PointerDown(e) => {
                self.update_mouse_action_by_down(e);
            },
            Event::PointerUp(e) => {
                self.update_mouse_action_by_up(e);
                let mut clear_selection = false;
                match self.current_mouse_action {
                    MouseAction::LeftOnce { pos, .. } => {
                        clear_selection = true;
                        if e.modifiers.control() && self.click(pos).is_some() {
                            return EventPropagation::Stop;
                        }
                    },
                    MouseAction::LeftSelect { start_pos, end_pos } => {
                        let mut selection = Selection::new(
                            SelectionType::Simple,
                            self.get_terminal_point(start_pos),
                            Side::Left,
                        );
                        selection
                            .update(self.get_terminal_point(end_pos), Side::Right);
                        selection.include_all();
                        raw.write().term.selection = Some(selection);
                        _cx.app_state_mut().request_paint(self.id);
                    },
                    MouseAction::LeftDouble { pos } => {
                        let position = self.get_terminal_point(pos);
                        let mut raw = raw.write();
                        let start_point = raw.term.semantic_search_left(position);
                        let end_point = raw.term.semantic_search_right(position);

                        let mut selection = Selection::new(
                            SelectionType::Simple,
                            start_point,
                            Side::Left,
                        );
                        selection.update(end_point, Side::Right);
                        selection.include_all();
                        raw.term.selection = Some(selection);
                        _cx.app_state_mut().request_paint(self.id);
                    },
                    MouseAction::RightOnce { pos } => {
                        let position = self.get_terminal_point(pos);
                        let raw = raw.read();
                        if let Some(selection) = &raw
                            .term
                            .selection
                            .as_ref()
                            .and_then(|x| x.to_range(&raw.term))
                        {
                            if selection.contains(position) {
                                let mut clipboard = SystemClipboard::new();
                                let content = raw.term.bounds_to_string(
                                    selection.start,
                                    selection.end,
                                );
                                if !content.is_empty() {
                                    clipboard.put_string(content);
                                }
                            }
                            clear_selection = true;
                        }
                    },
                    _ => {
                        clear_selection = true;
                    },
                }
                if clear_selection {
                    raw.write().term.selection = None;
                    _cx.app_state_mut().request_paint(self.id);
                }
            },
            _ => {},
        }
        EventPropagation::Continue
    }

    fn update(
        &mut self,
        cx: &mut floem::context::UpdateCx,
        state: Box<dyn std::any::Any>,
    ) {
        if let Ok(state) = state.downcast() {
            match *state {
                // TerminalViewState::Config => {},
                TerminalViewState::Focus(is_focused) => {
                    self.is_focused = is_focused;
                }, /* TerminalViewState::Raw(raw) => {
                    *     self.raw = raw;
                    * }, */
            }
            cx.app_state_mut().request_paint(self.id);
        }
    }

    fn layout(
        &mut self,
        cx: &mut floem::context::LayoutCx,
    ) -> floem::taffy::prelude::NodeId {
        cx.layout_node(self.id, false, |_cx| Vec::new())
    }

    fn compute_layout(
        &mut self,
        _cx: &mut floem::context::ComputeLayoutCx,
    ) -> Option<Rect> {
        let raw = self.terminal_data.data.with_untracked(|x| x.raw.clone());
        let layout = self.id.get_layout().unwrap_or_default();
        let size = layout.size;
        let size = Size::new(size.width as f64, size.height as f64);
        if size.is_zero_area() {
            return None;
        }
        if size != self.size {
            self.size = size;
            let (width, height) = self.terminal_size();
            let term_size = TermSize::new(width, height);
            raw.write().term.resize(term_size);
            self.proxy.terminal_resize(self.term_id, width, height);
            log::debug!("{:?} {}-{}", self.term_id, width, height);
        }

        None
    }

    fn paint(&mut self, cx: &mut floem::context::PaintCx) {
        let config = self.config.get();

        let (mode, launch_error, raw) = self
            .terminal_data
            .data
            .with_untracked(|x| (x.mode, x.launch_error.clone(), x.raw.clone()));
        let line_height = config.terminal_line_height() as f64;
        let font_family = config.terminal_font_family();
        let font_size = config.terminal_font_size();
        let char_size = self.char_size();
        let char_width = char_size.width;

        let family: Vec<FamilyOwned> =
            FamilyOwned::parse_list(font_family).collect();
        let attrs = Attrs::new().family(&family).font_size(font_size as f32);

        if let Some(error) = launch_error {
            let text_layout = TextLayout::new_with_text(
                &format!("Terminal failed to launch. Error: {error}"),
                AttrsList::new(
                    attrs.color(config.color(LapceColor::EDITOR_FOREGROUND)),
                ),
            );
            cx.draw_text_with_layout(
                text_layout.layout_runs(),
                Point::new(6.0, 0.0 + (line_height - char_size.height) / 2.0),
            );
            return;
        }

        let raw = raw.read();
        let term = &raw.term;
        let content = term.renderable_content();

        // let mut search = RegexSearch::new("[\\w\\\\?]+\\.rs:\\d+:\\d+").unwrap();
        // self.hyper_matches = visible_regex_match_iter(term, &mut
        // search).collect();

        if let Some(selection) = content.selection.as_ref() {
            let start_line = selection.start.line.0 + content.display_offset as i32;
            let start_line = if start_line < 0 {
                0
            } else {
                start_line as usize
            };
            let start_col = selection.start.column.0;

            let end_line = selection.end.line.0 + content.display_offset as i32;
            let end_line = if end_line < 0 { 0 } else { end_line as usize };
            let end_col = selection.end.column.0;

            for line in start_line..end_line + 1 {
                let left_col = if selection.is_block || line == start_line {
                    start_col
                } else {
                    0
                };
                let right_col = if selection.is_block || line == end_line {
                    end_col + 1
                } else {
                    term.last_column().0
                };
                let x0 = left_col as f64 * char_width;
                let x1 = right_col as f64 * char_width;
                let y0 = line as f64 * line_height;
                let y1 = y0 + line_height;
                cx.fill(
                    &Rect::new(x0, y0, x1, y1),
                    config.color(LapceColor::EDITOR_SELECTION),
                    0.0,
                );
            }
        } else if mode != Mode::Terminal {
            let y = (content.cursor.point.line.0 as f64
                + content.display_offset as f64)
                * line_height;
            cx.fill(
                &Rect::new(0.0, y, self.size.width, y + line_height),
                config.color(LapceColor::EDITOR_CURRENT_LINE),
                0.0,
            );
        }

        let (
            cursor,
            error,
            caret,
            terminal_font_family,
            terminal_font_size,
            terminal_bg,
        ) = (
            config.color(LapceColor::EDITOR_CARET),
            config.color(LapceColor::TERMINAL_CURSOR),
            config.color(LapceColor::LAPCE_ERROR),
            config.terminal_font_family(),
            config.terminal_font_size(),
            config.color(LapceColor::TERMINAL_BACKGROUND),
        );

        self.paint_content(
            cx,
            content,
            line_height,
            char_size,
            &config,
            cursor,
            error,
            caret,
            terminal_font_family,
            terminal_font_size,
            terminal_bg,
        );
    }
}

#[derive(Debug, Default, Copy, Clone)]
enum MouseAction {
    #[default]
    None,
    LeftDown {
        pos: Point,
    },
    LeftOnce {
        pos:  Point,
        time: SystemTime,
    },
    LeftSelect {
        start_pos: Point,
        end_pos:   Point,
    },
    LeftOnceAndDown {
        pos:  Point,
        time: SystemTime,
    },
    LeftDouble {
        pos: Point,
    },
    RightDown {
        pos: Point,
    },
    RightOnce {
        pos: Point,
    },
}

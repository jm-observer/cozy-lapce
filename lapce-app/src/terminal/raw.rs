use std::rc::Rc;

use alacritty_terminal::{
    Term,
    event::EventListener,
    grid::Dimensions,
    index::{Column, Direction, Line, Point},
    term::{
        cell::{Flags, LineLength},
        search::{Match, RegexIter, RegexSearch},
        test::TermSize,
    },
    vte::ansi,
};
use lapce_rpc::terminal::TermId;

use crate::window_workspace::CommonData;

pub struct EventProxy {
    term_id: TermId,
    raw_id:  u64,
    common:  Rc<CommonData>,
}

impl EventListener for EventProxy {
    fn send_event(&self, event: alacritty_terminal::event::Event) {
        match event {
            alacritty_terminal::event::Event::PtyWrite(s) => {
                self.common.proxy.proxy_rpc.terminal_write(
                    self.term_id,
                    self.raw_id,
                    s,
                );
            },
            alacritty_terminal::event::Event::MouseCursorDirty => {
                self.common.proxy.core_rpc.terminal_paint();
            },
            alacritty_terminal::event::Event::Title(s) => {
                self.common
                    .proxy
                    .core_rpc
                    .terminal_set_title(self.term_id, s);
            },
            _ => (),
        }
    }
}

pub struct RawTerminal {
    pub raw_id:       u64,
    pub parser:       ansi::Processor,
    pub term:         Term<EventProxy>,
    pub scroll_delta: f64,
}

impl RawTerminal {
    pub fn new(term_id: TermId, raw_id: u64, common: Rc<CommonData>) -> Self {
        let config = alacritty_terminal::term::Config {
            semantic_escape_chars: ",│`|\"' ()[]{}<>\t".to_string(),
            ..Default::default()
        };
        let event_proxy = EventProxy {
            raw_id,
            term_id,
            common,
        };

        let size = TermSize::new(50, 30);
        let term = Term::new(config, &size, event_proxy);
        let parser = ansi::Processor::new();

        Self {
            raw_id,
            parser,
            term,
            scroll_delta: 0.0,
        }
    }

    pub fn update_content(&mut self, content: &Vec<u8>) {
        for byte in content {
            self.parser.advance(&mut self.term, *byte);
        }
    }

    pub fn output(&self, line_num: usize) -> Vec<String> {
        let grid = self.term.grid();
        let mut lines = Vec::with_capacity(line_num);
        let mut rows = Vec::new();
        for line in (grid.topmost_line().0..=grid.bottommost_line().0)
            .map(Line)
            .rev()
        {
            let row_cell = &grid[line];
            if row_cell[Column(row_cell.len() - 1)]
                .flags
                .contains(Flags::WRAPLINE)
            {
                rows.push(row_cell);
            } else {
                if !rows.is_empty() {
                    let mut new_line = Vec::new();
                    std::mem::swap(&mut rows, &mut new_line);
                    let line_str: String = new_line
                        .into_iter()
                        .rev()
                        .flat_map(|x| {
                            x.into_iter().take(x.line_length().0).map(|x| x.c)
                        })
                        .collect();
                    if line_str.trim().is_empty() {
                        continue;
                    }
                    lines.push(line_str);
                    if lines.len() >= line_num {
                        break;
                    }
                }
                rows.push(row_cell);
            }
        }
        lines
    }
}

pub fn visible_regex_match_iter<'a, EventProxy>(
    term: &'a Term<EventProxy>,
    regex: &'a mut RegexSearch,
) -> impl Iterator<Item = Match> + 'a {
    let viewport_start = Line(-(term.grid().display_offset() as i32));
    let viewport_end = viewport_start + term.bottommost_line();
    let mut start = term.line_search_left(Point::new(viewport_start, Column(0)));
    let mut end = term.line_search_right(Point::new(viewport_end, Column(0)));
    start.line = start.line.max(viewport_start - MAX_SEARCH_LINES);
    end.line = end.line.min(viewport_end + MAX_SEARCH_LINES);

    RegexIter::new(start, end, Direction::Right, term, regex)
        .skip_while(move |rm| rm.end().line < viewport_start)
        .take_while(move |rm| rm.start().line <= viewport_end)
}
/// todo:should be improved
pub const MAX_SEARCH_LINES: usize = 100;

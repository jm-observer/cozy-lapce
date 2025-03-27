use std::{
    borrow::Cow,
    fmt::{Debug, Formatter},
    iter::{Filter, Peekable},
    ops::Range,
    path::PathBuf,
    slice::Iter,
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
    },
};

use anyhow::{Result, anyhow, bail};
use floem::{
    context::StyleCx,
    kurbo::{Point, Rect, Size},
    peniko::{Brush, Color},
    reactive::{
        ReadSignal, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, batch,
    },
    text::{Attrs, AttrsList, FONT_SYSTEM, FamilyOwned, LineHeightValue, Wrap},
};
use itertools::Itertools;
use lapce_xi_rope::{
    Interval, Rope, RopeDelta, Transformer,
    spans::{SpanIter, Spans, SpansBuilder},
};
use layout::{TextLayout, TextLayoutLine};
use line::OriginFoldedLine;
use log::{debug, error, info, warn};
use lsp_types::{
    DiagnosticSeverity, DocumentHighlight, InlayHint, InlayHintLabel, Location,
    Position,
};
use phantom_text::{
    PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine,
};
use signal::Signals;
use smallvec::SmallVec;
use style::NewLineStyle;

use crate::{
    DiagnosticData, EditorViewKind,
    config::EditorConfig,
    lines::{
        action::UpdateFolding,
        buffer::{Buffer, InvalLines, rope_text::RopeText},
        command::EditCommand,
        cursor::{ColPosition, Cursor, CursorAffinity, CursorMode},
        diff::{DiffResult, advance, consume_line, is_changed, is_diff, is_empty},
        edit::{Action, EditConf, EditType},
        encoding::{offset_utf8_to_utf16, offset_utf16_to_utf8},
        fold::{
            FoldedRanges, FoldingDisplayItem, FoldingRanges, FoldingRangesLine,
            MergeFoldingRangesLine,
        },
        indent::IndentStyle,
        line::{LineTy, OriginLine, VisualLine},
        line_ending::LineEnding,
        mode::{Mode, MotionMode},
        phantom_text::Text,
        register::Register,
        screen_lines::{ScreenLines, VisualLineInfo, VisualOriginText},
        selection::Selection,
        style::EditorStyle,
        text::{PreeditData, SystemClipboard},
        util::get_document_highlight,
        word::WordCursor,
    },
    syntax::{BracketParser, Syntax, edit::SyntaxEdit},
};

pub mod action;
pub mod buffer;
pub mod char_buffer;
pub mod chars;
pub mod command;
pub mod cursor;
pub mod delta_compute;
pub mod diff;
pub mod edit;
pub mod editor_command;
pub mod encoding;
pub mod fold;
pub mod indent;
pub mod layout;
pub mod line;
pub mod line_ending;
pub mod mode;
pub mod movement;
pub mod paragraph;
pub mod phantom_text;
pub mod register;
pub mod screen_lines;
pub mod selection;
pub mod signal;
pub mod soft_tab;
pub mod style;
pub mod text;
pub mod util;
pub mod word;

// /// Minimum width that we'll allow the view to be wrapped at.
// const MIN_WRAPPED_WIDTH: f32 = 100.0;

#[derive(Clone)]
pub struct LinesOfOriginOffset {
    pub origin_offset:             usize,
    pub origin_line:               OriginLine,
    pub origin_folded_line:        OriginFoldedLine,
    // 在折叠行的偏移值
    pub origin_folded_line_offest: usize, /* pub visual_line:
                                           * VisualLine,
                                           * 在视觉行的偏移值
                                           * pub visual_line_offest:        usize */
}

#[derive(Clone, Copy)]
pub struct DocLinesManager {
    lines: RwSignal<DocLines>,
}

impl DocLinesManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cx: Scope,
        diagnostics: DiagnosticData,
        syntax: Syntax,
        parser: BracketParser,
        viewport: Rect,
        editor_style: EditorStyle,
        config: EditorConfig,
        buffer: Buffer,
        // kind: RwSignal<EditorViewKind>,
        path: Option<PathBuf>,
    ) -> Self {
        Self {
            lines: cx.create_rw_signal(DocLines::new(
                cx,
                diagnostics,
                syntax,
                parser,
                viewport,
                editor_style,
                config,
                buffer,
                // kind,
                path,
            )),
        }
    }

    pub fn with_untracked<O>(&self, f: impl FnOnce(&DocLines) -> O) -> O {
        self.lines.with_untracked(f)
    }

    // 不允许这样！也许会出现渲染死循环的问题！！！
    // pub fn with<O>(&self, f: impl FnOnce(&DocLines) -> O) -> O
    // {
    //     self.lines.with(f)
    // }

    // pub fn get(&self) -> DocLines {
    //     self.lines.get()
    // }

    pub fn update(&self, f: impl FnOnce(&mut DocLines)) {
        // not remove `batch`!
        batch(|| {
            self.lines.update(f);
        });
    }

    pub fn try_update<O>(&self, f: impl FnOnce(&mut DocLines) -> O) -> Option<O> {
        // not remove `batch`!
        batch(|| self.lines.try_update(f))
    }
}

#[derive(Clone)]
pub struct DocLines {
    // pub origin_lines:        Vec<OriginLine>,
    // pub origin_folded_lines: Vec<OriginFoldedLine>,

    // pub visual_lines:        Vec<VisualLine>,
    // pub font_sizes: Rc<EditorFontSizes>,
    // font_size_cache_id: FontSizeCacheId,
    // wrap: ResolvedWrap,
    // pub layout_event: Listener<LayoutEvent>,
    // max_width:               f64,

    // editor: Editor
    pub inlay_hints:     Option<Spans<InlayHint>>,
    pub completion_lens: Option<String>,
    pub completion_pos:  (usize, usize),
    pub folding_ranges:  FoldingRanges,
    // pub buffer: Buffer,
    pub diagnostics:     DiagnosticData,

    /// Current inline completion text, if any.
    /// This will be displayed even on views that are not focused.
    /// (line, col)
    pub inline_completion: Option<(String, usize, usize)>,
    pub preedit:           PreeditData,
    // tree-sitter
    pub syntax:            Syntax,
    // lsp 来自lsp的语义样式.string是指代码的类别，如macro、function
    pub semantic_styles:   Option<(Option<String>, Spans<String>)>,
    pub parser:            BracketParser,
    // /// 用于存储每行的前景色样式。如keyword的颜色
    // pub line_styles: HashMap<usize, Vec<NewLineStyle>>,
    pub editor_style:      EditorStyle,
    viewport_size:         Size,
    pub config:            EditorConfig,
    // pub buffer: Buffer,
    // pub buffer_rev: u64,
    // pub kind: RwSignal<EditorViewKind>,
    pub(crate) signals:    Signals,
    style_from_lsp:        bool,
    // folding_items: Vec<FoldingDisplayItem>,
    path:                  Option<PathBuf>,
    document_highlight:    Option<Vec<DocumentHighlight>>,
}

impl DocLines {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cx: Scope,
        diagnostics: DiagnosticData,
        syntax: Syntax,
        parser: BracketParser,
        viewport: Rect,
        editor_style: EditorStyle,
        config: EditorConfig,
        buffer: Buffer,
        // kind: RwSignal<EditorViewKind>,
        path: Option<PathBuf>,
    ) -> Self {
        let last_line = buffer.last_line() + 1;
        let signals = Signals::new(cx, &editor_style, buffer, (last_line, 0.0));

        // log::info!("{}", serde_json::to_string(&config).unwrap());

        Self {
            path,
            signals,
            // layout_event: Listener::new_empty(cx), //
            // font_size_cache_id: id,
            viewport_size: viewport.size(),
            config,
            editor_style,
            // origin_lines: vec![],
            // origin_folded_lines: vec![],
            // visual_lines: vec![],
            // max_width: 0.0,
            inlay_hints: None,
            completion_pos: (0, 0),
            folding_ranges: Default::default(),
            // buffer: Buffer::new(""),
            diagnostics,
            completion_lens: None,
            inline_completion: None,
            preedit: PreeditData::new(cx),
            syntax,
            semantic_styles: None,
            parser,
            // line_styles: Default::default(),
            // kind,
            style_from_lsp: false,
            // folding_items: Default::default(),
            document_highlight: None,
        }
    }

    // pub fn update_cache_id(&mut self) {
    //     let current_id = self.font_sizes.cache_id();
    //     if current_id != self.font_size_cache_id {
    //         self.font_size_cache_id = current_id;
    //         self.update()
    //     }
    // }

    // pub fn update_font_sizes(&mut self, font_sizes:
    // Rc<EditorFontSizes>) {     self.font_sizes = font_sizes;
    //     self.update()
    // }

    // fn clear(&mut self) {
    //     self.config.line_height = 0;
    // }

    fn update_parser(&mut self) -> Result<()> {
        let buffer = self.signals.buffer.val(); // 提前保存，结束不可变借用
        let styles_exist = self.syntax.styles.is_some(); // 提前判断，不再借用 self.syntax

        let parser = &mut self.parser; // 现在安全地进行可变借用
        if styles_exist {
            parser.update_code(buffer, Some(&self.syntax))?;
        } else {
            parser.update_code(buffer, None)?;
        }
        Ok(())
    }

    // fn update_lines_old(&mut self) {
    //     self.clear();
    //
    //     let last_line = self.buffer.last_line();
    //     let semantic_styles = self.init_semantic_styes();
    //     // self.update_parser(buffer);
    //     let mut current_line = 0;
    //     let mut origin_folded_line_index = 0;
    //     let mut visual_line_index = 0;
    //     self.config.line_height = self.config.line_height;
    //
    //     let font_size = self.config.font_size;
    //     let family = Cow::Owned(
    //         FamilyOwned::parse_list(&self.config.font_family).
    // collect(),     );
    //     let attrs = Attrs::new()
    //         .color(self.editor_style.ed_text_color())
    //         .family(&family)
    //         .font_size(font_size as f32)
    //         .line_height(LineHeightValue::Px(self.config.line_height as
    // f32));     // let mut duration = Duration::from_secs(0);
    //     while current_line <= last_line {
    //         let start_offset =
    // self.buffer.offset_of_line(current_line);         let
    // end_offset = self.buffer.offset_of_line(current_line + 1);
    //         // let time = std::time::SystemTime::now();
    //         let text_layout = self.new_text_layout(
    //             current_line,
    //             start_offset,
    //             end_offset,
    //             font_size,
    //             attrs, &semantic_styles,
    //         );
    //         // duration += time.elapsed().unwrap();
    //         let origin_line_start = text_layout.phantom_text.line;
    //         let origin_line_end =
    // text_layout.phantom_text.last_line;
    //
    //         let width = text_layout.text.size().width;
    //         if width > self.max_width {
    //             self.max_width = width;
    //         }
    //
    //         for origin_line in origin_line_start..=origin_line_end
    // {             self.origin_lines.push(OriginLine {
    //                 line_index: origin_line,
    //                 start_offset,
    //                 phantom: Default::default(),
    //                 fg_styles: vec![],
    //             });
    //         }
    //
    //         let origin_interval = Interval {
    //             start:
    // self.buffer.offset_of_line(origin_line_start),
    // end: self.buffer.offset_of_line(origin_line_end + 1),
    //         };
    //
    //         let mut visual_offset_start = 0;
    //         let mut visual_offset_end;
    //
    //         // [visual_offset_start..visual_offset_end)
    //         for (origin_folded_line_sub_index, layout) in
    //             text_layout.text.line_layout().iter().enumerate()
    //         {
    //             if layout.glyphs.is_empty() {
    //                 self.visual_lines.push(VisualLine {
    //                     line_index: visual_line_index,
    //                     origin_interval: Interval::new(
    //                         origin_interval.end,
    //                         origin_interval.end,
    //                     ),
    //                     visual_interval: Interval::new(
    //                         visual_offset_start,
    //                         visual_offset_start,
    //                     ),
    //                     origin_line: origin_line_start,
    //                     origin_folded_line:
    // origin_folded_line_index,
    // origin_folded_line_sub_index: 0,
    // text_layout: text_layout.clone(),                 });
    //                 continue;
    //             }
    //             visual_offset_end = visual_offset_start +
    // layout.glyphs.len() - 1;             let offset_info =
    // text_layout                 .phantom_text
    //
    // .cursor_position_of_final_col(visual_offset_start);
    //             let origin_interval_start =
    //                 self.buffer.offset_of_line(offset_info.0) +
    // offset_info.1;             let offset_info = text_layout
    //                 .phantom_text
    //
    // .cursor_position_of_final_col(visual_offset_end);
    //
    //             let origin_interval_end =
    //                 self.buffer.offset_of_line(offset_info.0) +
    // offset_info.1;             let origin_interval = Interval {
    //                 start: origin_interval_start,
    //                 end: origin_interval_end + 1,
    //             };
    //
    //             self.visual_lines.push(VisualLine {
    //                 line_index: visual_line_index,
    //                 origin_interval,
    //                 origin_line: origin_line_start,
    //                 origin_folded_line: origin_folded_line_index,
    //                 origin_folded_line_sub_index,
    //                 text_layout: text_layout.clone(),
    //                 visual_interval: Interval::new(
    //                     visual_offset_start,
    //                     visual_offset_end + 1,
    //                 ),
    //             });
    //
    //             visual_offset_start = visual_offset_end;
    //             visual_line_index += 1;
    //         }
    //
    //         self.origin_folded_lines.push(OriginFoldedLine {
    //             line_index: origin_folded_line_index,
    //             origin_line_start,
    //             origin_line_end,
    //             origin_interval,
    //             text_layout,
    //         });
    //
    //         current_line = origin_line_end + 1;
    //         origin_folded_line_index += 1;
    //     }
    //     self.on_update_lines();
    // }

    // fn update_lines_2(&mut self, (_start_delta, _end_delta):
    // (Option<LineDelta>, Option<LineDelta>)) {     self.clear();
    //     self.origin_lines.clear();
    //     self.origin_folded_lines.clear();
    //     self.visual_lines.clear();
    //     let last_line = self.buffer().last_line();
    //     let mut current_line = 0;
    //     let mut origin_folded_line_index = 0;
    //     let mut visual_line_index = 0;
    //     self.config.line_height = self.config.line_height;
    //     let font_size = self.config.font_size;
    //     let family = Cow::Owned(
    //         FamilyOwned::parse_list(&self.config.font_family).
    // collect(),     );
    //     let attrs = Attrs::new()
    //         .color(self.editor_style.ed_text_color())
    //         .family(&family)
    //         .font_size(font_size as f32)
    //         .line_height(LineHeightValue::Px(self.config.line_height as
    // f32));     // let mut duration = Duration::from_secs(0);
    //
    //     let all_origin_lines = self.init_all_origin_line((&None,
    // &None));     while current_line <= last_line {
    //         let Some((text_layout, semantic_styles,
    // diagnostic_styles)) = self.new_text_layout_2(
    // current_line,             &all_origin_lines,
    //             font_size,
    //             attrs,
    //         ) else {
    //             // todo
    //             break;
    //         };
    //         // duration += time.elapsed().unwrap();
    //         let origin_line_start = text_layout.phantom_text.line;
    //         let origin_line_end =
    // text_layout.phantom_text.last_line;
    //
    //         let width = text_layout.text.size().width;
    //         if width > self.max_width {
    //             self.max_width = width;
    //         }
    //
    //         let origin_interval = Interval {
    //             start:
    // self.buffer().offset_of_line(origin_line_start),
    //             end: self.buffer().offset_of_line(origin_line_end +
    // 1),         };
    //
    //         let mut visual_offset_start = 0;
    //         let mut visual_offset_end;
    //
    //         // [visual_offset_start..visual_offset_end)
    //         for (origin_folded_line_sub_index, layout) in
    //             text_layout.text.line_layout().iter().enumerate()
    //         {
    //             if layout.glyphs.is_empty() {
    //                 self.visual_lines.push(VisualLine {
    //                     line_index: visual_line_index,
    //                     origin_interval: Interval::new(
    //                         origin_interval.end,
    //                         origin_interval.end,
    //                     ),
    //                     visual_interval: Interval::new(
    //                         visual_offset_start,
    //                         visual_offset_start,
    //                     ),
    //                     origin_line: origin_line_start,
    //                     origin_folded_line:
    // origin_folded_line_index,
    // origin_folded_line_sub_index: 0,                     //
    // text_layout: text_layout.clone(),                 });
    //                 continue;
    //             }
    //             visual_offset_end = visual_offset_start +
    // layout.glyphs.len() - 1;             let offset_info =
    // text_layout                 .phantom_text
    //
    // .cursor_position_of_final_col(visual_offset_start);
    //             let origin_interval_start =
    //                 self.buffer().offset_of_line(offset_info.0) +
    // offset_info.1;             let offset_info = text_layout
    //                 .phantom_text
    //
    // .cursor_position_of_final_col(visual_offset_end);
    //
    //             let origin_interval_end =
    //                 self.buffer().offset_of_line(offset_info.0) +
    // offset_info.1;             let origin_interval = Interval {
    //                 start: origin_interval_start,
    //                 end: origin_interval_end + 1,
    //             };
    //
    //             self.visual_lines.push(VisualLine {
    //                 line_index: visual_line_index,
    //                 origin_interval,
    //                 origin_line: origin_line_start,
    //                 origin_folded_line: origin_folded_line_index,
    //                 origin_folded_line_sub_index,
    //                 // text_layout: text_layout.clone(),
    //                 visual_interval: Interval::new(
    //                     visual_offset_start,
    //                     visual_offset_end + 1,
    //                 ),
    //             });
    //
    //             visual_offset_start = visual_offset_end;
    //             visual_line_index += 1;
    //         }
    //
    //         self.origin_folded_lines.push(OriginFoldedLine {
    //             line_index: origin_folded_line_index,
    //             origin_line_start,
    //             origin_line_end,
    //             origin_interval,
    //             text_layout,
    //             semantic_styles,
    //             diagnostic_styles,
    //         });
    //
    //         current_line = origin_line_end + 1;
    //         origin_folded_line_index += 1;
    //     }
    //     self.origin_lines = all_origin_lines;
    //     self.on_update_lines();
    // }

    // fn update_lines(
    //     &mut self,
    //     (start_delta, end_delta): (Option<LineDelta>, Option<LineDelta>)
    // ) -> Result<()> {
    //     self.clear();
    //     self.visual_lines.clear();
    //     self.config.line_height = self.config.line_height;
    //     let last_line = self.signals.buffer.val().last_line();
    //     let font_size = self.config.font_size;
    //     let family =
    //         Cow::Owned(FamilyOwned::parse_list(&self.config.font_family).
    // collect());     let attrs = Attrs::new()
    //         .color(self.editor_style.ed_text_color())
    //         .family(&family)
    //         .font_size(font_size as f32)
    //         .line_height(LineHeightValue::Px(self.config.line_height as f32));
    //     // let mut duration = Duration::from_secs(0);
    //
    //     let all_origin_lines =
    //         self.init_all_origin_line((&start_delta, &end_delta))?;
    //
    //     let mut origin_folded_lines = if let Some(LineDelta {
    //         start_line,
    //         end_line,
    //         ..
    //     }) = start_delta
    //     {
    //         self.origin_folded_lines
    //             .iter()
    //             .filter_map(|folded| {
    //                 if start_line <= folded.origin_line_start
    //                     && folded.origin_line_end < end_line
    //                 {
    //                     Some(folded.clone())
    //                 } else {
    //                     None
    //                 }
    //             })
    //             .collect()
    //     } else {
    //         Vec::new()
    //     };
    //     {
    //         let mut origin_folded_line_index = 0;
    //
    //         let mut current_line = if let Some(line) = origin_folded_lines.last()
    // {             line.origin_line_end + 1
    //         } else {
    //             0
    //         };
    //         while current_line <= last_line {
    //             let (text_layout, semantic_styles, diagnostic_styles) = self
    //                 .new_text_layout_2(
    //                     current_line,
    //                     &all_origin_lines,
    //                     font_size,
    //                     attrs
    //                 )?;
    //             // duration += time.elapsed().unwrap();
    //             let origin_line_start = text_layout.phantom_text.line;
    //             let origin_line_end = text_layout.phantom_text.last_line;
    //
    //             let width = text_layout.text.size().width;
    //             if width > self.max_width {
    //                 self.max_width = width;
    //             }
    //
    //             let origin_interval = Interval {
    //                 start: self.buffer().offset_of_line(origin_line_start)?,
    //                 end:   self.buffer().offset_of_line(origin_line_end + 1)?
    //             };
    //
    //             origin_folded_lines.push(OriginFoldedLine {
    //                 line_index: origin_folded_line_index,
    //                 origin_line_start,
    //                 origin_line_end,
    //                 origin_interval,
    //                 text_layout,
    //                 semantic_styles,
    //                 diagnostic_styles
    //             });
    //
    //             current_line = origin_line_end + 1;
    //             origin_folded_line_index += 1;
    //         }
    //     }
    //     {
    //         let mut visual_line_index = 0;
    //         // while let Some(line) = origin_line_iter.next() {
    //         for line in origin_folded_lines.iter() {
    //             // duration += time.elapsed().unwrap();
    //             let text_layout = &line.text_layout;
    //             let origin_line_start = text_layout.phantom_text.line;
    //             let origin_line_end = text_layout.phantom_text.last_line;
    //             let origin_folded_line_index = line.line_index;
    //
    //             let origin_interval = Interval {
    //                 start: self.buffer().offset_of_line(origin_line_start)?,
    //                 end:   self.buffer().offset_of_line(origin_line_end + 1)?
    //             };
    //
    //             let mut visual_offset_start = 0;
    //             let mut visual_offset_end;
    //
    //             // [visual_offset_start..visual_offset_end)
    //             for (origin_folded_line_sub_index, layout) in
    //                 text_layout.text.line_layout().iter().enumerate()
    //             {
    //                 if layout.glyphs.is_empty() {
    //                     self.visual_lines.push(VisualLine {
    //                         line_index:                   visual_line_index,
    //                         origin_interval:              Interval::new(
    //                             origin_interval.end,
    //                             origin_interval.end
    //                         ),
    //                         visual_interval:              Interval::new(
    //                             visual_offset_start,
    //                             visual_offset_start
    //                         ),
    //                         origin_line:                  origin_line_start,
    //                         origin_folded_line:
    // origin_folded_line_index,
    // origin_folded_line_sub_index: 0 /* text_layout:
    //                                                          * text_layout.
    //                                                          * clone(), */
    //                     });
    //                     continue;
    //                 }
    //                 visual_offset_end =
    //                     visual_offset_start + layout.glyphs.len() - 1;
    //                 let offset_info = text_layout
    //                     .phantom_text
    //                     .cursor_position_of_final_col(visual_offset_start);
    //                 let origin_interval_start =
    //                     self.buffer().offset_of_line(offset_info.0)? +
    // offset_info.1;                 let offset_info = text_layout
    //                     .phantom_text
    //                     .cursor_position_of_final_col(visual_offset_end);
    //
    //                 let origin_interval_end =
    //                     self.buffer().offset_of_line(offset_info.0)? +
    // offset_info.1;                 let origin_interval = Interval {
    //                     start: origin_interval_start,
    //                     end:   origin_interval_end + 1
    //                 };
    //
    //                 self.visual_lines.push(VisualLine {
    //                     line_index: visual_line_index,
    //                     origin_interval,
    //                     origin_line: origin_line_start,
    //                     origin_folded_line: origin_folded_line_index,
    //                     origin_folded_line_sub_index,
    //                     // text_layout: text_layout.clone(),
    //                     visual_interval: Interval::new(
    //                         visual_offset_start,
    //                         visual_offset_end + 1
    //                     )
    //                 });
    //
    //                 visual_offset_start = visual_offset_end;
    //                 visual_line_index += 1;
    //             }
    //         }
    //     }
    //
    //     self.origin_lines = all_origin_lines;
    //     self.origin_folded_lines = origin_folded_lines;
    //     self.on_update_lines();
    //     Ok(())
    // }

    // fn init_origin_line(
    //     &self,
    //     current_line: usize,
    //     semantic_styles: Option<&mut Peekable<SpanIter<String>>>,
    //     inlay_hints: Option<&mut Peekable<SpanIter<InlayHint>>>,
    //     folded_ranges: FoldedRanges
    // ) -> Result<OriginLine> {
    //     let start_offset = self.buffer().offset_of_line(current_line)?;
    //     let end_offset = self.buffer().offset_of_line(current_line + 1)?;
    //     // let mut fg_styles = Vec::new();
    //     // 用于存储该行的最高诊断级别。最后决定该行的背景色
    //     // let mut max_severity: Option<DiagnosticSeverity> = None;
    //     // fg_styles.extend(self.get_line_diagnostic_styles(
    //     //     start_offset,
    //     //     end_offset,
    //     //     &mut max_severity,
    //     //     0,
    //     // ));
    //
    //     let phantom_text = self.phantom_text(
    //         current_line,
    //         folded_ranges,
    //         inlay_hints,
    //         start_offset,
    //         end_offset
    //     )?;
    //     let semantic_styles = semantic_styles
    //         .map(|x| {
    //             let mut styles = vec![];
    //             loop {
    //                 if let Some((Interval { start, .. }, _)) = x.peek() {
    //                     if end_offset <= *start {
    //                         break;
    //                     }
    //                 } else {
    //                     break;
    //                 }
    //                 if let Some((Interval { start, end }, fg_color)) = x.next() {
    //                     if start_offset <= start && end < end_offset {
    //                         let Some(color) =
    //                             self.config.syntax_style_color(fg_color)
    //                         else {
    //                             continue;
    //                         };
    //                         styles.push(NewLineStyle {
    //                             origin_line: current_line,
    //                             origin_line_offset_start: start - start_offset,
    //                             len: end - start,
    //                             start_of_buffer: start,
    //                             end_of_buffer: end,
    //                             fg_color: color /* folded_line_offset_start:
    //                                              * start - line_start,
    //                                              * folded_line_offset_end: end -
    //                                              * line_start */
    //                         });
    //                     }
    //                 }
    //             }
    //             styles
    //         })
    //         .unwrap_or_default();
    //     // let semantic_styles =
    //     //     self.get_line_semantic_styles(current_line, start_offset,
    // end_offset);     let diagnostic_styles =
    // self.get_line_diagnostic_styles_2(         current_line,
    //         start_offset,
    //         end_offset
    //     );
    //     Ok(OriginLine {
    //         line_index: current_line,
    //         start_offset,
    //         len: end_offset - start_offset,
    //         phantom: phantom_text,
    //         semantic_styles,
    //         diagnostic_styles
    //     })
    // }

    // fn get_line_semantic_styles(
    //     &self,
    //     origin_line: usize,
    //     line_start: usize,
    //     line_end: usize
    // ) -> Vec<NewLineStyle> {
    //     self._get_line_semantic_styles(origin_line, line_start, line_end)
    //         .unwrap_or_default()
    // }
    //
    // fn _get_line_semantic_styles(
    //     &self,
    //     origin_line: usize,
    //     line_start: usize,
    //     line_end: usize
    // ) -> Option<Vec<NewLineStyle>> {
    //     Some(
    //         if self.style_from_lsp {
    //             &self.semantic_styles.as_ref()?.1
    //         } else {
    //             self.syntax.styles.as_ref()?
    //         }
    //         .iter()
    //         .filter_map(|(Interval { start, end }, fg_color)| {
    //             if line_start <= start && end < line_end {
    //                 let color = self.config.syntax_style_color(fg_color)?;
    //                 Some(NewLineStyle {
    //                     origin_line,
    //                     origin_line_offset_start: start - line_start,
    //                     len: end - start,
    //                     start_of_buffer: start,
    //                     end_of_buffer: end,
    //                     fg_color: color,
    //                     // folded_line_offset_start: start - line_start,
    //                     // folded_line_offset_end: end - line_start
    //                 })
    //             } else {
    //                 None
    //             }
    //         })
    //         .collect()
    //     )
    // }

    // /// ~~视觉~~行的text_layout信息
    // fn _text_layout_of_visual_line(&self, line: usize) -> Option<&TextLayoutLine>
    // {     Some(
    //         &self
    //             .origin_folded_lines
    //             .get(self.visual_lines.get(line)?.origin_folded_line)?
    //             .text_layout
    //     )
    // }

    // pub fn text_layout_of_visual_line(
    //     &self,
    //     line: usize
    // ) -> Result<&TextLayoutLine> {
    //     Ok(
    //         &self
    //             .origin_folded_lines
    //             .get(line).ok_or(anyhow!("text layout empty)"))?.text_layout
    //     )
    //     //
    //     // self._text_layout_of_visual_line(line)
    //     //     .ok_or(anyhow!("text layout empty)"))
    // }

    // // 原始行的第一个视觉行。原始行可能会有多个视觉行
    // pub fn start_visual_line_of_origin_line(
    //     &self,
    //     origin_line: usize
    // ) -> Result<&VisualLine> {
    //     let folded_line = self.folded_line_of_origin_line(origin_line)?;
    //     self.start_visual_line_of_folded_line(folded_line.line_index)
    // }
    //
    // pub fn start_visual_line_of_folded_line(
    //     &self,
    //     origin_folded_line: usize
    // ) -> Result<&VisualLine> {
    //     for visual_line in &self.visual_lines {
    //         if visual_line.origin_folded_line == origin_folded_line {
    //             return Ok(visual_line);
    //         }
    //     }
    //     bail!(
    //         "start_visual_line_of_folded_line \
    //          origin_folded_line={origin_folded_line}"
    //     )
    // }

    // pub fn folded_line_of_origin_line(
    //     &self,
    //     origin_line: usize
    // ) -> Result<&OriginFoldedLine> {
    //     for folded_line in &self.origin_folded_lines {
    //         if folded_line.origin_line_start <= origin_line
    //             && origin_line <= folded_line.origin_line_end
    //         {
    //             return Ok(folded_line);
    //         }
    //     }
    //     bail!("folded_line_of_origin_line origin_line={origin_line}")
    // }

    #[inline]
    pub fn folded_line_of_buffer_offset(
        &self,
        buffer_offset: usize,
    ) -> Result<OriginFoldedLine> {
        let line = self.buffer().line_of_offset(buffer_offset);
        self.init_folded_line_layout_alone(line)
    }

    // pub fn folded_line_of_visual_line(
    //     &self,
    //     vl: &VisualLine
    // ) -> Result<&OriginFoldedLine> {
    //     for folded_line in &self.origin_folded_lines {
    //         if folded_line.line_index == vl.origin_folded_line {
    //             return Ok(folded_line);
    //         }
    //     }
    //     bail!("folded_line_of_visual_line {vl:?}")
    // }

    // 不支持编辑器折叠
    // pub fn visual_line_of_folded_line_and_sub_index(
    //     &self,
    //     origin_folded_line: usize,
    //     sub_index: usize
    // ) -> Result<&VisualLine> {
    //     for visual_line in &self.visual_lines {
    //         if visual_line.origin_folded_line == origin_folded_line
    //             && visual_line.origin_folded_line_sub_index == sub_index
    //         {
    //             return Ok(visual_line);
    //         }
    //     }
    //     bail!(
    //         "visual_line_of_folded_line_and_sub_index \
    //          origin_folded_line={origin_folded_line} sub_index={sub_index}"
    //     )
    // }

    // 不支持编辑器折叠
    // pub fn last_visual_line(&self) -> &VisualLine {
    //     &self.visual_lines[self.visual_lines.len() - 1]
    // }

    /// the buffer offset at the click position
    pub fn buffer_offset_of_click(
        &self,
        _mode: &CursorMode,
        point: Point,
    ) -> Result<(usize, bool, CursorAffinity)> {
        let mut is_inside = true;
        let info = match self.origin_folded_line_of_point(point.y)? {
            None => {
                is_inside = false;
                let last_line = self.buffer().last_line();
                self.init_folded_line_layout_alone(last_line)?
            },
            Some(rs) => rs,
        };
        let hit_point = info.hit_point(Point::new(point.x, 0.0));
        let visual_char_offset = hit_point.index;

        if hit_point.is_inside && is_inside {
            for x in info.text() {
                match x {
                    Text::Phantom { text } => {
                        if text.final_col <= visual_char_offset
                            && visual_char_offset < text.next_final_col()
                        {
                            // 在虚拟文本的后半部分，则光标置于虚拟文本之后
                            return Ok(
                                if hit_point.index
                                    > text.final_col + text.text.len() / 2
                                {
                                    (
                                        info.origin_interval.start
                                            + text.origin_merge_col,
                                        true,
                                        CursorAffinity::Forward,
                                    )
                                } else {
                                    (
                                        info.origin_interval.start
                                            + text.origin_merge_col,
                                        true,
                                        CursorAffinity::Backward,
                                    )
                                },
                            );
                        } else if visual_char_offset == text.next_final_col() {
                            return Ok((
                                info.origin_interval.start + text.origin_merge_col,
                                true,
                                CursorAffinity::Forward,
                            ));
                        }
                    },
                    Text::OriginText { text } => {
                        if text.final_col.contains(visual_char_offset)
                            || info.last_line
                                && text.final_col.end == visual_char_offset
                        {
                            return Ok((
                                visual_char_offset - text.final_col.start
                                    + text.origin_merge_col_start()
                                    + info.origin_interval.start,
                                true,
                                CursorAffinity::Backward,
                            ));
                        }
                    },
                    Text::EmptyLine { .. } => unreachable!(),
                }
            }
            error!(
                "path {:?}, point={:?}, index={}",
                self.path, point, hit_point.index
            );
            unreachable!();
        } else {
            let Some(text) = info.text().last() else {
                unreachable!()
            };
            // last of line
            Ok(match text {
                Text::Phantom { text } => (
                    text.origin_merge_col + info.origin_interval.start,
                    false,
                    CursorAffinity::Forward,
                ),
                Text::OriginText { .. } => {
                    // 该行只有 "\r\n"，因此return '\r' CursorAffinity::Backward
                    if info.len_without_rn() == 0 {
                        (info.offset_of_line(), false, CursorAffinity::Backward)
                    } else {
                        // 该返回\r的前一个字符，CursorAffinity::Forward
                        let line_ending_len = info.len() - info.len_without_rn();
                        if line_ending_len == 0 {
                            (
                                info.origin_interval.end,
                                false,
                                CursorAffinity::Backward,
                            )
                        } else {
                            (
                                info.origin_interval.end - line_ending_len,
                                false,
                                CursorAffinity::Backward,
                            )
                        }
                    }
                    // (text.merge_col.end +
                    // text_layout.phantom_text.offset_of_line - 1, false,
                    // CursorAffinity::Forward)
                },
                Text::EmptyLine { text } => {
                    (text.offset_of_line, false, CursorAffinity::Backward)
                },
            })
        }
    }

    pub(crate) fn origin_folded_line_of_point(
        &self,
        point_y: f64,
    ) -> Result<Option<OriginFoldedLine>> {
        let origin_folded_line_index =
            (point_y / self.config.line_height as f64).floor() as usize;
        self.init_folded_line_layout_alone_by_index(origin_folded_line_index)
    }

    pub fn result_of_left_click(&mut self, mut point: Point) -> Result<ClickResult> {
        let Some(info) = self.origin_folded_line_of_point(point.y)? else {
            return Ok(ClickResult::NoHintOrNothing);
        };
        // let y = text_layout
        //     .get_layout_y(0)
        //     .unwrap_or(0.0);
        point.y = 0.0;
        let hit_point = info.hit_point(point);
        if !hit_point.is_inside {
            return Ok(ClickResult::NoHintOrNothing);
        }
        Ok(
            if let Text::Phantom { text: phantom } =
                info.text_of_final_col(hit_point.index)
            {
                let phantom_offset = hit_point.index - phantom.final_col;
                if let PhantomTextKind::InlayHint = phantom.kind {
                    let line = phantom.line as u32;
                    let index = phantom.col as u32;
                    if let Some(hints) = &self.inlay_hints {
                        if let Some(location) = hints.iter().find_map(|(_, hint)| {
                            if hint.position.line == line
                                && hint.position.character == index
                            {
                                if let InlayHintLabel::LabelParts(parts) =
                                    &hint.label
                                {
                                    let mut start = 0;
                                    for part in parts {
                                        let end = start + part.value.len();
                                        if start <= phantom_offset
                                            && phantom_offset < end
                                        {
                                            return part.location.clone();
                                        }
                                        start = end;
                                    }
                                }
                            }
                            None
                        }) {
                            return Ok(ClickResult::MatchHint(location));
                        }
                    }
                } else if let PhantomTextKind::LineFoldedRang {
                    start_position,
                    ..
                } = phantom.kind
                {
                    self.update_folding_ranges(start_position.into())?;
                    return Ok(ClickResult::MatchFolded);
                }
                ClickResult::MatchWithoutLocation
            } else {
                ClickResult::NoHintOrNothing
            },
        )
    }

    // /// 原始位移字符所在的行信息（折叠行、原始行、视觉行）
    // pub fn lines_of_origin_offset(
    //     &self,
    //     buffer_offset: usize
    // ) -> Result<LinesOfOriginOffset> {
    //     // 位于的原始行，以及在原始行的起始offset
    //     let origin_line = self.buffer().line_of_offset(buffer_offset);
    //     let origin_line = self
    //         .origin_lines
    //         .get(origin_line)
    //         .ok_or(anyhow!("origin_line is empty"))?
    //         .clone();
    //     let offset = buffer_offset - origin_line.start_offset;
    //     let folded_line =
    // self.folded_line_of_origin_line(origin_line.line_index)?;
    //     let origin_folded_line_offset = folded_line
    //         .text_layout
    //         .phantom_text
    //         .final_col_of_col(origin_line.line_index, offset, false);
    //     let folded_line_layout = folded_line.text_layout.text.line_layout();
    //     let mut visual_line_offset = origin_folded_line_offset;
    //     for sub_line in folded_line_layout.iter() {
    //         if visual_line_offset < sub_line.glyphs.len() {
    //             break;
    //         } else {
    //             visual_line_offset -= sub_line.glyphs.len();
    //         }
    //     }
    //     // let visual_line = self.visual_line_of_folded_line_and_sub_index(
    //     //     folded_line.line_index,
    //     //     sub_line_index
    //     // )?;
    //     Ok(LinesOfOriginOffset {
    //         origin_offset: 0,
    //         origin_line,
    //         origin_folded_line: folded_line.clone(),
    //         origin_folded_line_offest: 0,
    //         // visual_line: visual_line.clone(),
    //         // visual_line_offest: 0
    //     })
    // }

    // /// 视觉行的偏移位置，对应的上一行的偏移位置（原始文本）和是否为最后一个字符
    // ///
    // /// return (OriginFoldedLine, final col)
    // pub fn previous_visual_line(
    //     &self,
    //     visual_line_index: usize,
    //     line_offset: usize,
    //     _affinity: CursorAffinity
    // ) -> Option<(&OriginFoldedLine, usize, usize)> {
    //     let line = self
    //         .origin_folded_lines
    //         .get(visual_line_index)?;
    //     let (_origin_line, _origin_col, final_col, _offset_buffer, _) =
    //         line.text_layout
    //         .phantom_text
    //         .cursor_position_of_final_col(line_offset);
    //     // let last_char = line.is_last_char(offset_line,
    // self.buffer().line_ending());
    //
    //     Some((
    //         line,
    //         final_col, _offset_buffer
    //     ))
    // }

    // /// 视觉行的偏移位置，对应的上一行的偏移位置（原始文本）和是否为最后一个字符
    // ///
    // /// return (&OriginFoldedLine, final col, offset of buffer)
    // pub fn next_visual_line(
    //     &self,
    //     visual_line_index: usize,
    //     final_cal: usize,
    //     _affinity: CursorAffinity
    // ) -> Option<(&OriginFoldedLine, usize, usize)> {
    //     let next_line = self.origin_folded_lines.get(visual_line_index + 1)?;
    //     let (_origin_line, _offset_line, final_col, _offset_buffer, _) =
    // next_line         .text_layout
    //         .phantom_text
    //         .cursor_position_of_final_col(final_cal);
    //     // let last_char = next_line.is_last_char(offset_line,
    // self.buffer().line_ending());     Some((
    //         next_line,
    //         final_col,
    //         _offset_buffer
    //     ))
    // }

    /// 原始位移字符所在的合并行的偏移位置和是否是最后一个字符，point
    ///
    /// return (OriginFoldedLine, final col, last char, origin_line,
    /// start_offset_of_origin_line
    pub fn folded_line_and_final_col_of_offset(
        &self,
        buffer_offset: usize,
        affinity: CursorAffinity,
    ) -> Result<(
        OriginFoldedLine,
        usize, //bool, usize, usize
    )> {
        // // 位于的原始行，以及在原始行的起始offset
        // let (origin_line, start_offset_of_origin_line) = {
        //     let origin_line = self.buffer().line_of_offset(offset);
        //     let origin_line_start_offset =
        //         self.buffer().offset_of_line(origin_line)?;
        //     (origin_line, origin_line_start_offset)
        // };
        // let offset_of_col = offset - start_offset_of_origin_line;
        let line = self.buffer().line_of_offset(buffer_offset);
        let folded_line = self.init_folded_line_layout_alone(line)?;
        let merge_offset = buffer_offset - folded_line.origin_interval.start;
        let final_col =
            folded_line.cursor_final_col_of_merge_col(merge_offset, affinity)?;

        // let final_col = folded_line
        //     .final_offset_of_line_and_offset(origin_line, offset_of_col,
        // affinity); let last_char = folded_line.is_last_char(final_col);

        Ok((
            // visual_line.clone(),
            // offset_of_visual,
            folded_line,
            final_col, // last_char, origin_line, start_offset_of_origin_line,
        ))
    }

    // /// 原始位移字符所在的视觉行，以及视觉行的偏移位置，
    // /// 合并行的偏移位置和是否是最后一个字符，point
    // pub fn visual_info_of_cursor_offset(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity
    // ) -> Result<Option<(usize, bool, &OriginFoldedLine)>> {
    //     // 位于的原始行，以及在原始行的起始offset
    //     let (origin_line, offset_of_origin_line) = {
    //         let origin_line = self.buffer().line_of_offset(offset);
    //         let origin_line_start_offset =
    //             self.buffer().offset_of_line(origin_line)?;
    //         (origin_line, origin_line_start_offset)
    //     };
    //     let offset = offset - offset_of_origin_line;
    //     let folded_line = self.folded_line_of_origin_line(origin_line)?;
    //
    //     let Some(offset_of_folded) = folded_line.visual_offset_of_cursor_offset(
    //         origin_line,
    //         offset,
    //         affinity
    //     ) else {
    //         return Ok(None);
    //     };
    //     // let visual_line = self.visual_line_of_folded_line_and_sub_index(
    //     //     folded_line.line_index,
    //     //     sub_line_index
    //     // )?;
    //     let last_char = folded_line.is_last_char(offset_of_folded);
    //
    //     Ok(Some((offset_of_folded, last_char, folded_line)))
    // }

    // pub fn visual_lines(
    //     &mut self,
    //     start: usize,
    //     end: usize,
    //     view_kind: EditorViewKind,
    //     line_height: usize,
    //     y0: f64,
    //     base: Rect
    // ) -> ScreenLines {
    //     let mut max_width = 0.0;
    //
    //     let mut highlights = self
    //         .document_highlight
    //         .clone()
    //         .unwrap_or_default()
    //         .into_iter()
    //         .peekable();
    //
    //     // todo other color
    //     let document_highlight_color = self.document_highlight();
    //     match view_kind {
    //         EditorViewKind::Normal => {
    //             let start = start.min(self.origin_folded_lines.len() - 1);
    //             let end = end.min(self.origin_folded_lines.len() - 1);
    //             let mut visual_lines = Vec::with_capacity(end - start + 1);
    //
    //             for index in start..=end {
    //                 let line = &mut self.origin_folded_lines[index];
    //                 let highlight = get_document_highlight(
    //                     &mut highlights,
    //                     line.origin_line_start as u32,
    //                     line.origin_line_end as u32
    //                 );
    //                 line.init_layout();
    //                 line.extra_style();
    //                 line.init_document_highlight(
    //                     highlight,
    //                     document_highlight_color,
    //                     line_height
    //                 );
    //                 let size_width = line.size_width().width;
    //                 if size_width > max_width {
    //                     max_width = size_width;
    //                 }
    //                 let folded_line_y = line.line_index * line_height;
    //                 let visual_line_info = VisualLineInfo::OriginText {
    //                     text: VisualOriginText {
    //                         folded_line_y: folded_line_y as f64 - y0,
    //                         folded_line:   line.clone(),
    //                         is_diff:       false
    //                     }
    //                 };
    //                 visual_lines.push(visual_line_info);
    //             }
    //             self.signals.max_width.update_if_not_equal(max_width);
    //             ScreenLines {
    //                 visual_lines,
    //                 diff_sections: None,
    //                 base,
    //                 line_height: line_height as f64,
    //                 buffer_len: self.buffer().len()
    //             }
    //         },
    //         EditorViewKind::Diff { changes, .. } => {
    //             // let changes = diff.changes();
    //             let mut empty_lines = changes
    //                 .iter()
    //                 .filter(is_empty as fn(&&DiffResult) -> bool)
    //                 .peekable();
    //             let mut change_lines = changes
    //                 .iter()
    //                 .filter(is_changed as fn(&&DiffResult) -> bool)
    //                 .peekable();
    //             let len = end - start;
    //             // 合并后，起始行
    //             let mut start_line =
    //                 consume_lines_until_enough(&mut empty_lines, start);
    //             let mut empty_count = 0;
    //             let mut origin_line_index = start_line;
    //             let mut visual_lines = Vec::with_capacity(end - start + 1);
    //             //
    //             for folded_line_index in start_line..start_line + len {
    //                 // 未合并的原始行
    //                 if consume_line(&mut empty_lines, start_line + empty_count) {
    //                     let folded_line_y = folded_line_index * line_height;
    //                     let visual_line_info = VisualLineInfo::DiffDelete {
    //                         folded_line_y: folded_line_y as f64 - y0
    //                     };
    //                     visual_lines.push(visual_line_info);
    //                     empty_count += 1;
    //                 } else if let Some(line) =
    //                     &mut self.origin_folded_lines.get_mut(origin_line_index)
    //                 {
    //                     let is_diff =
    //                         is_diff(&mut change_lines, line.origin_line_start);
    //                     start_line = line.origin_line_end + 1;
    //                     origin_line_index += 1;
    //
    //                     let highlight = get_document_highlight(
    //                         &mut highlights,
    //                         line.origin_line_start as u32,
    //                         line.origin_line_end as u32
    //                     );
    //                     line.init_layout();
    //                     line.extra_style();
    //                     line.init_document_highlight(
    //                         highlight,
    //                         document_highlight_color,
    //                         line_height
    //                     );
    //                     let size_width = line.size_width().width;
    //                     if size_width > max_width {
    //                         max_width = size_width;
    //                     }
    //                     let folded_line_y = folded_line_index * line_height;
    //                     let visual_line_info = VisualLineInfo::OriginText {
    //                         text: VisualOriginText {
    //                             folded_line_y: folded_line_y as f64 - y0,
    //                             folded_line: line.clone(),
    //                             is_diff
    //                         }
    //                     };
    //                     visual_lines.push(visual_line_info);
    //                     advance(&mut empty_lines, start_line + empty_count);
    //                     empty_count = 0;
    //                 }
    //             }
    //             self.signals.max_width.update_if_not_equal(max_width);
    //             ScreenLines {
    //                 visual_lines,
    //                 diff_sections: None,
    //                 base,
    //                 line_height: line_height as f64,
    //                 buffer_len: self.buffer().len()
    //             }
    //         }
    //     }
    // }

    pub fn compute_screen_lines_new(
        &mut self,
        base: Rect,
        view_kind: EditorViewKind,
    ) -> Result<(Arc<ScreenLines>, Vec<FoldingDisplayItem>, Vec<VisualLine>)> {
        info!("_compute_screen_lines base={base:?} kind={view_kind:?}");
        let line_height = self.config.line_height;
        let (y0, y1) = (base.y0, base.y1);
        let min_val = (y0 / line_height as f64).floor() as usize;
        let max_val = (y1 / line_height as f64).floor() as usize;

        let (folded, changes, is_normal) = match view_kind {
            EditorViewKind::Normal => {
                (self.folding_ranges.get_all_folded_range(), vec![], true)
            },
            EditorViewKind::Diff { changes, .. } => {
                (FoldedRanges(vec![]), changes, false)
            },
        };
        debug!("{folded:?}");
        let folded_line_count = folded.folded_line_count();
        let mut folded_lines = FoldingRangesLine::new(&folded.0);

        let empty_line_len = changes.iter().fold(0, |x, y| {
            if let DiffResult::Empty { lines } = y {
                x + lines.len()
            } else {
                x
            }
        });
        let mut empty_lines = changes
            .iter()
            .filter(is_empty as fn(&&DiffResult) -> bool)
            .peekable();
        let mut change_lines = changes
            .iter()
            .filter(is_changed as fn(&&DiffResult) -> bool)
            .peekable();
        let last_line =
            self.buffer().last_line() + empty_line_len - folded_line_count;
        let min_val = min_val.min(last_line);
        let max_val = max_val.min(last_line);
        let visual_lines = self.generate_visual_lines(
            last_line,
            &mut empty_lines,
            &mut folded_lines,
        );

        let screen_lines = self._compute_screen_lines_new(
            &visual_lines[min_val..=max_val],
            self.config.line_height,
            y0,
            base,
            &mut change_lines,
            &mut FoldingRangesLine::new(&folded.0),
        )?;

        let display_items = if is_normal {
            self.folding_ranges.to_display_items(&screen_lines)
        } else {
            vec![]
        };

        self.signals.trigger();
        Ok((Arc::new(screen_lines), display_items, visual_lines))
    }

    pub fn _compute_screen_lines_new(
        &mut self,
        lines: &[VisualLine],
        line_height: usize,
        y0: f64,
        base: Rect,
        change_lines: &mut Peekable<
            Filter<Iter<DiffResult>, fn(&&DiffResult) -> bool>,
        >,
        folded_lines: &mut FoldingRangesLine,
    ) -> Result<ScreenLines> {
        let mut max_width = 0.0;

        let mut highlights = self
            .document_highlight
            .clone()
            .unwrap_or_default()
            .into_iter()
            .peekable();

        // todo other color
        let document_highlight_color = self.document_highlight();
        let mut visual_lines = Vec::with_capacity(lines.len());
        let buffer = self.buffer();
        let line_ending: &'static str = buffer.line_ending().get_chars();
        let last_line = buffer.last_line();
        let preedit_phantom = util::preedit_phantom_2(
            &self.preedit,
            buffer,
            Some(self.config.editor_foreground),
        );
        let family =
            Cow::Owned(FamilyOwned::parse_list(&self.config.font_family).collect());
        let attrs = self.init_attrs_with_color(&family);

        let mut semantic_styles = if self.style_from_lsp {
            self.semantic_styles.as_ref().map(|x| x.1.iter().peekable())
        } else {
            self.syntax.styles.as_ref().map(|x| x.iter().peekable())
        };
        let mut inlay_hints = self
            .config
            .enable_inlay_hints
            .then_some(())
            .and(self.inlay_hints.as_ref())
            .map(|x| x.iter().peekable());

        for line in lines {
            match &line.line_ty {
                LineTy::DiffEmpty { .. } => {
                    let folded_line_y = line.line_index * line_height;
                    let visual_line_info = VisualLineInfo::DiffDelete {
                        folded_line_y: folded_line_y as f64 - y0,
                    };
                    visual_lines.push(visual_line_info);
                },
                LineTy::OriginText {
                    line_range_inclusive: line_number,
                    origin_folded_line_index,
                } => {
                    let mut folded_line = self.init_folded_line_2(
                        *line_number.start(),
                        attrs,
                        *origin_folded_line_index,
                        line_ending,
                        last_line,
                        &mut semantic_styles,
                        &mut inlay_hints,
                        folded_lines,
                        &preedit_phantom,
                    )?;
                    let is_diff = is_diff(change_lines, *line_number.start());

                    let highlight = get_document_highlight(
                        &mut highlights,
                        folded_line.origin_line_start as u32,
                        folded_line.origin_line_end as u32,
                    );
                    folded_line.init_layout();
                    folded_line.init_extra_style();
                    folded_line.init_document_highlight(
                        highlight,
                        document_highlight_color,
                        line_height,
                    );
                    let size_width = folded_line.size_width().width;
                    if size_width > max_width {
                        max_width = size_width;
                    }
                    let folded_line_y = line.line_index * line_height;
                    let visual_line_info = VisualLineInfo::OriginText {
                        text: VisualOriginText {
                            folded_line_y: folded_line_y as f64 - y0,
                            folded_line: folded_line.clone(),
                            is_diff,
                        },
                    };
                    visual_lines.push(visual_line_info);
                },
            }
        }
        self.signals.max_width.update_if_not_equal(max_width);
        Ok(ScreenLines {
            visual_lines,
            diff_sections: None,
            base,
            line_height: line_height as f64,
            buffer_len: self.buffer().len(),
        })
    }

    pub fn init_folded_line_layout_alone(
        &self,
        mut current_origin_line: usize,
    ) -> Result<OriginFoldedLine> {
        let binding = self.folding_ranges.get_all_folded_range();
        debug!("{binding:?}");
        let folded_index = MergeFoldingRangesLine::new(
            &self.folding_ranges.get_all_folded_folded_range().0,
        )
        .get_origin_folded_line_index(current_origin_line);
        if let Some(folded_range) = MergeFoldingRangesLine::new(
            &self.folding_ranges.get_all_folded_folded_range().0,
        )
        .get_folded_range_by_line(current_origin_line as u32)
        {
            current_origin_line = *folded_range.start();
        }

        let mut folded_lines = FoldingRangesLine::new(&binding.0);

        let buffer = self.buffer();
        let line_ending: &'static str = buffer.line_ending().get_chars();
        let last_line = buffer.last_line();
        let preedit_phantom = util::preedit_phantom_2(
            &self.preedit,
            buffer,
            Some(self.config.editor_foreground),
        );
        let family =
            Cow::Owned(FamilyOwned::parse_list(&self.config.font_family).collect());
        let font_size = self.config.font_size;
        let attrs = Attrs::new()
            .family(&family)
            .font_size(font_size as f32)
            .line_height(LineHeightValue::Px(self.config.line_height as f32));

        let mut semantic_styles = if self.style_from_lsp {
            self.semantic_styles.as_ref().map(|x| x.1.iter().peekable())
        } else {
            self.syntax.styles.as_ref().map(|x| x.iter().peekable())
        };
        let mut inlay_hints = self
            .config
            .enable_inlay_hints
            .then_some(())
            .and(self.inlay_hints.as_ref())
            .map(|x| x.iter().peekable());

        let folded_line = self.init_folded_line_2(
            current_origin_line,
            attrs,
            folded_index,
            line_ending,
            last_line,
            &mut semantic_styles,
            &mut inlay_hints,
            &mut folded_lines,
            &preedit_phantom,
        )?;
        folded_line.init_layout();
        Ok(folded_line)
    }

    pub fn init_folded_line_layout_alone_by_index(
        &self,
        origin_folded_index: usize,
    ) -> Result<Option<OriginFoldedLine>> {
        let buffer = self.buffer();
        let line_ending: &'static str = buffer.line_ending().get_chars();
        let last_line = buffer.last_line();

        let binding = self.folding_ranges.get_all_folded_range();
        let Some(line_num) = MergeFoldingRangesLine::new(
            &self.folding_ranges.get_all_folded_folded_range().0,
        )
        .get_line_num(origin_folded_index, last_line) else {
            return Ok(None);
        };
        let mut folded_lines = FoldingRangesLine::new(&binding.0);

        let preedit_phantom = util::preedit_phantom_2(
            &self.preedit,
            buffer,
            Some(self.config.editor_foreground),
        );
        let family =
            Cow::Owned(FamilyOwned::parse_list(&self.config.font_family).collect());
        let font_size = self.config.font_size;
        let attrs = Attrs::new()
            .family(&family)
            .font_size(font_size as f32)
            .line_height(LineHeightValue::Px(self.config.line_height as f32));

        let mut semantic_styles = if self.style_from_lsp {
            self.semantic_styles.as_ref().map(|x| x.1.iter().peekable())
        } else {
            self.syntax.styles.as_ref().map(|x| x.iter().peekable())
        };
        let mut inlay_hints = self
            .config
            .enable_inlay_hints
            .then_some(())
            .and(self.inlay_hints.as_ref())
            .map(|x| x.iter().peekable());

        let folded_line = self.init_folded_line_2(
            line_num,
            attrs,
            origin_folded_index,
            line_ending,
            last_line,
            &mut semantic_styles,
            &mut inlay_hints,
            &mut folded_lines,
            &preedit_phantom,
        )?;
        folded_line.init_layout();
        Ok(Some(folded_line))
    }

    fn init_folded_line_2(
        &self,
        current_origin_line: usize,
        attrs: Attrs,
        origin_folded_line_index: usize,
        line_ending: &'static str,
        last_line: usize,
        all_semantic_styles: &mut Option<Peekable<SpanIter<String>>>,
        all_inlay_hints: &mut Option<Peekable<SpanIter<InlayHint>>>,
        folded_ranges: &mut FoldingRangesLine,
        preedit_phantom: &Option<PhantomText>,
    ) -> Result<OriginFoldedLine> {
        let text_layout = self.new_text_layout_3(
            current_origin_line,
            attrs,
            line_ending,
            last_line,
            all_semantic_styles,
            all_inlay_hints,
            folded_ranges,
            preedit_phantom,
        )?;
        // duration += time.elapsed().unwrap();
        let origin_line_start = text_layout.phantom_text.line;
        let origin_line_end = text_layout.phantom_text.last_line;

        let origin_interval = Interval {
            start: self.buffer().offset_of_line(origin_line_start)?,
            end:   self.buffer().offset_of_line(origin_line_end + 1)?,
        };

        let last_line =
            origin_line_start <= last_line && last_line <= origin_line_end;

        Ok(OriginFoldedLine {
            line_index: origin_folded_line_index,
            origin_line_start,
            origin_line_end,
            origin_interval,
            text_layout,
            last_line,
        })
    }

    pub fn generate_visual_lines(
        &mut self,
        last_line: usize,
        empty_lines: &mut Peekable<
            Filter<Iter<DiffResult>, fn(&&DiffResult) -> bool>,
        >,
        folded_lines: &mut FoldingRangesLine,
    ) -> Vec<VisualLine> {
        // 合并后，起始行
        let mut empty_count = 0;
        let mut origin_folded_line_index = 0;
        let mut origin_line_num = 0;
        let mut visual_lines = Vec::with_capacity(last_line);
        for visual_line_index in 0..=last_line {
            if let Some(range) =
                consume_line(empty_lines, origin_line_num + empty_count)
            {
                visual_lines.push(VisualLine {
                    line_index: visual_line_index,
                    line_ty:    LineTy::DiffEmpty {
                        change_line_start: range.start,
                    },
                });
                empty_count += 1;
                continue;
            } else {
                advance(empty_lines, origin_line_num + empty_count);
            }
            if let Some(range) =
                folded_lines.get_folded_range_by_line(origin_line_num as u32)
            {
                origin_line_num = *range.end() + 1;
                visual_lines.push(VisualLine {
                    line_index: visual_line_index,
                    line_ty:    LineTy::OriginText {
                        line_range_inclusive: range,
                        origin_folded_line_index,
                    },
                });
            } else {
                visual_lines.push(VisualLine {
                    line_index: visual_line_index,
                    line_ty:    LineTy::OriginText {
                        line_range_inclusive: origin_line_num..=origin_line_num,
                        origin_folded_line_index,
                    },
                });
                origin_line_num += 1;
            }
            origin_folded_line_index += 1;
            empty_count = 0;
        }
        visual_lines
    }

    // fn phantom_text(
    //     &self,
    //     line: usize,
    //     folded_ranges: FoldedRanges,
    //     inlay_hints: Option<&mut Peekable<SpanIter<InlayHint>>>,
    //     start_offset: usize,
    //     end_offset: usize
    // ) -> Result<PhantomTextLine> {
    //     let buffer = self.buffer();
    //     let origin_text_len = end_offset - start_offset;
    //     let mut text = inlay_hints
    //         .map(|x| {
    //             let mut styles =
    //                 SmallVec::<[crate::lines::phantom_text::PhantomText;
    // 6]>::new();             loop {
    //                 if let Some((Interval { start, .. }, _)) = x.peek() {
    //                     if end_offset <= *start {
    //                         break;
    //                     }
    //                 } else {
    //                     break;
    //                 }
    //                 if let Some((Interval { start, end }, inlay_hint)) = x.next()
    // {                     if start_offset <= start && end < end_offset {
    //                         let (_, col) = match buffer.offset_to_line_col(start)
    // {                             Ok(rs) => rs,
    //                             Err(err) => {
    //                                 error!("{err:?}");
    //                                 return SmallVec::new();
    //                             }
    //                         };
    //                         let mut text = match &inlay_hint.label {
    //                             InlayHintLabel::String(label) =>
    // label.to_string(),
    // InlayHintLabel::LabelParts(parts) => {
    // parts.iter().map(|p| &p.value).join("")                             },
    //                         };
    //                         match (text.starts_with(':'), text.ends_with(':')) {
    //                             (true, true) => {
    //                                 text.push(' ');
    //                             },
    //                             (true, false) => {
    //                                 text.push(' ');
    //                             },
    //                             (false, true) => {
    //                                 text = format!(" {} ", text);
    //                             },
    //                             (false, false) => {
    //                                 text = format!(" {}", text);
    //                             }
    //                         }
    //                         styles.push(PhantomText {
    //                             kind: PhantomTextKind::InlayHint,
    //                             col,
    //                             text,
    //                             fg: Some(self.config.inlay_hint_fg),
    //                             // font_family:
    //                             // Some(self.config.inlay_hint_font_family()),
    //                             font_size:
    // Some(self.config.inlay_hint_font_size()),                             bg:
    // Some(self.config.inlay_hint_bg),                             under_line:
    // None,                             final_col: col,
    //                             line,
    //                             visual_merge_col: col,
    //                             origin_merge_col: col
    //                         })
    //                     }
    //                 }
    //             }
    //             styles
    //         })
    //         .unwrap_or_default();
    //
    //     // lsp返回的字符包括换行符，现在长度不考虑，后续会有问题
    //     // let line_ending =
    //     // self.buffer.line_ending().get_chars().len();
    //     // if origin_text_len >= line_ending {
    //     //     origin_text_len -= line_ending;
    //     // }
    //     // if line == 10 {
    //     //     info!("start_offset={start_offset}
    //     // end_offset={end_offset}
    //     // origin_text_len={origin_text_len}"); }
    //
    //     // let folded_ranges =
    //     //     self.folding_ranges.get_folded_range_by_line(line as u32);
    //
    //     // If hints are enabled, and the hints field is filled, then
    //     // get the hints for this line and convert them into
    //     // PhantomText instances
    //     // let hints = self
    //     //     .config
    //     //     .enable_inlay_hints
    //     //     .then_some(())
    //     //     .and(self.inlay_hints.as_ref())
    //     //     .map(|hints| hints.iter_chunks(start_offset..end_offset))
    //     //     .into_iter()
    //     //     .flatten()
    //     //     .filter(|(interval, hint)| {
    //     //         interval.start >= start_offset
    //     //             && interval.start < end_offset
    //     //             && !folded_ranges.contain_position(hint.position)
    //     //     })
    //     //     .filter_map(|(interval, inlay_hint)| {
    //     //         let (_, col) = match buffer.offset_to_line_col(interval.start)
    // {     //             Ok(rs) => rs,
    //     //             Err(err) => {
    //     //                 error!("{err:?}");
    //     //                 return None;
    //     //             }
    //     //         };
    //     //         let mut text = match &inlay_hint.label {
    //     //             InlayHintLabel::String(label) => label.to_string(),
    //     //             InlayHintLabel::LabelParts(parts) => {
    //     //                 parts.iter().map(|p| &p.value).join("")
    //     //             },
    //     //         };
    //     //         match (text.starts_with(':'), text.ends_with(':')) {
    //     //             (true, true) => {
    //     //                 text.push(' ');
    //     //             },
    //     //             (true, false) => {
    //     //                 text.push(' ');
    //     //             },
    //     //             (false, true) => {
    //     //                 text = format!(" {} ", text);
    //     //             },
    //     //             (false, false) => {
    //     //                 text = format!(" {}", text);
    //     //             }
    //     //         }
    //     //         Some(PhantomText {
    //     //             kind: PhantomTextKind::InlayHint,
    //     //             col,
    //     //             text,
    //     //             fg: Some(self.config.inlay_hint_fg),
    //     //             // font_family:
    //     //             // Some(self.config.inlay_hint_font_family()),
    //     //             font_size: Some(self.config.inlay_hint_font_size()),
    //     //             bg: Some(self.config.inlay_hint_bg),
    //     //             under_line: None,
    //     //             final_col: col,
    //     //             line,
    //     //             visual_merge_col: col,
    //     //             origin_merge_col: col,
    //     //         })
    //     //     });
    //     // You're quite unlikely to have more than six hints on a
    //     // single line this later has the diagnostics added
    //     // onto it, but that's still likely to be below six
    //     // overall.
    //     // let mut text: SmallVec<[PhantomText; 6]> = hints.collect();
    //
    //     // If error lens is enabled, and the diagnostics field is
    //     // filled, then get the diagnostics that end on this
    //     // line which have a severity worse than HINT and convert them
    //     // into PhantomText instances
    //
    //     // 会与折叠冲突，因此暂时去掉
    //     // let mut diag_text: SmallVec<[PhantomText; 6]> = self.config
    //     //     .enable_error_lens
    //     //     .then_some(())
    //     //     .map(|_|
    //     // self.diagnostics.diagnostics_span.get_untracked())
    //     //     .map(|diags| {
    //     //         diags
    //     //             .iter_chunks(start_offset..end_offset)
    //     //             .filter_map(|(iv, diag)| {
    //     //                 let end = iv.end();
    //     //                 let end_line =
    //     // self.buffer.line_of_offset(end);                 if
    //     // end_line == line                     &&
    //     // diag.severity < Some(DiagnosticSeverity::HINT)
    //     //                     &&
    //     // !folded_ranges.contain_position(diag.range.start)
    //     //                     &&
    //     // !folded_ranges.contain_position(diag.range.end)
    //     //                 {
    //     //                     let fg = {
    //     //                         let severity = diag
    //     //                             .severity
    //     //
    //     // .unwrap_or(DiagnosticSeverity::WARNING);
    //     //
    //     // self.config.color_of_error_lens(severity)
    //     //                     };
    //     //
    //     //                     let text = if
    //     // self.config.only_render_error_styling {
    //     // "".to_string()                     } else if
    //     // self.config.error_lens_multiline {
    //     // format!("    {}", diag.message)
    //     // } else {                         format!("    {}",
    //     // diag.message.lines().join(" "))
    //     // };                     Some(PhantomText {
    //     //                         kind: PhantomTextKind::Diagnostic,
    //     //                         col: end_offset - start_offset,
    //     //                         affinity:
    //     // Some(CursorAffinity::Backward),
    //     // text,                         fg: Some(fg),
    //     //                         font_size: Some(
    //     //
    //     // self.config.error_lens_font_size(),
    //     // ),                         bg: None,
    //     //                         under_line: None,
    //     //                         final_col: end_offset -
    //     // start_offset,                         line,
    //     //                         merge_col: end_offset -
    //     // start_offset,                     })
    //     //                 } else {
    //     //                     None
    //     //                 }
    //     //             })
    //     //             .collect::<SmallVec<[PhantomText; 6]>>()
    //     //     })
    //     //     .unwrap_or_default();
    //     //
    //     // text.append(&mut diag_text);
    //
    //     let (completion_line, completion_col) = self.completion_pos;
    //     let completion_text = self.config
    //     .enable_completion_lens
    //     .then_some(())
    //     .and(self.completion_lens.as_ref())
    //     // TODO: We're probably missing on various useful completion things to
    // include here!     .filter(|_| {
    //         line == completion_line
    //             && !folded_ranges.contain_position(Position {
    //             line: completion_line as u32,
    //             character: completion_col as u32,
    //         })
    //     })
    //     .map(|completion| PhantomText {
    //         kind: PhantomTextKind::Completion,
    //         col: completion_col,
    //         text: completion.clone(),
    //         fg: Some(self.config.completion_lens_foreground),
    //         font_size: Some(self.config.completion_lens_font_size()),
    //         // font_family:
    // Some(self.config.editor.completion_lens_font_family()),         bg: None,
    //         under_line: None,
    //         final_col: completion_col,
    //         line,
    //         visual_merge_col: completion_col,
    //         // TODO: italics?
    //         origin_merge_col: completion_col,
    //     });
    //     if let Some(completion_text) = completion_text {
    //         text.push(completion_text);
    //     }
    //
    //     // TODO: don't display completion lens and inline completion
    //     // at the same time and/or merge them so that they can
    //     // be shifted between like multiple inline completions
    //     // can
    //     // let (inline_completion_line, inline_completion_col) =
    //     //     self.inline_completion_pos;
    //     let inline_completion_text = self
    //         .config
    //         .enable_inline_completion
    //         .then_some(())
    //         .and(self.inline_completion.as_ref())
    //         .filter(|(_, inline_completion_line, inline_completion_col)| {
    //             line == *inline_completion_line
    //                 && !folded_ranges.contain_position(Position {
    //                     line:      *inline_completion_line as u32,
    //                     character: *inline_completion_col as u32
    //                 })
    //         })
    //         .map(|(completion, _, inline_completion_col)| {
    //             PhantomText {
    //                 kind: PhantomTextKind::Completion,
    //                 col: *inline_completion_col,
    //                 text: completion.clone(),
    //                 fg: Some(self.config.completion_lens_foreground),
    //                 font_size: Some(self.config.completion_lens_font_size()),
    //                 // font_family:
    //                 // Some(self.config.
    //                 // completion_lens_font_family()),
    //                 bg: None,
    //                 under_line: None,
    //                 final_col: *inline_completion_col,
    //                 line,
    //                 visual_merge_col: *inline_completion_col, // TODO: italics?
    //                 origin_merge_col: *inline_completion_col
    //             }
    //         });
    //     if let Some(inline_completion_text) = inline_completion_text {
    //         text.push(inline_completion_text);
    //     }
    //
    //     // todo filter by folded?
    //     if let Some(preedit) = util::preedit_phantom(
    //         &self.preedit,
    //         buffer,
    //         Some(self.config.editor_foreground),
    //         line
    //     ) {
    //         text.push(preedit)
    //     }
    //
    //     let fg = self.config.inlay_hint_fg;
    //     let font_size = self.config.inlay_hint_font_size();
    //     let bg = self.config.inlay_hint_bg;
    //     text.extend(
    //         folded_ranges.into_phantom_text(buffer, line, font_size, fg, bg)
    //     );
    //
    //     Ok(PhantomTextLine::new(
    //         line,
    //         origin_text_len,
    //         start_offset,
    //         text
    //     ))
    // }

    fn phantom_text_2(
        &self,
        line: usize,
        inlay_hints: Option<&mut Peekable<SpanIter<InlayHint>>>,
        start_offset: usize,
        end_offset: usize,
        folded_ranges: &mut FoldingRangesLine,
        preedit_phantom: &Option<PhantomText>,
    ) -> Result<PhantomTextLine> {
        let buffer = self.buffer();
        let origin_text_len = end_offset - start_offset;
        let mut text = inlay_hints
            .map(|x| {
                let mut styles =
                    SmallVec::<[crate::lines::phantom_text::PhantomText; 6]>::new();
                loop {
                    if let Some((Interval { start, .. }, _)) = x.peek() {
                        if end_offset <= *start {
                            break;
                        }
                    } else {
                        break;
                    }
                    if let Some((Interval { start, end }, inlay_hint)) = x.next() {
                        if start_offset <= start && end < end_offset {
                            let (_, col) = match buffer.offset_to_line_col(start) {
                                Ok(rs) => rs,
                                Err(err) => {
                                    error!("{err:?}");
                                    return SmallVec::new();
                                },
                            };
                            let mut text = match &inlay_hint.label {
                                InlayHintLabel::String(label) => label.to_string(),
                                InlayHintLabel::LabelParts(parts) => {
                                    parts.iter().map(|p| &p.value).join("")
                                },
                            };
                            match (text.starts_with(':'), text.ends_with(':')) {
                                (true, true) => {
                                    text.push(' ');
                                },
                                (true, false) => {
                                    text.push(' ');
                                },
                                (false, true) => {
                                    text = format!(" {} ", text);
                                },
                                (false, false) => {
                                    text = format!(" {}", text);
                                },
                            }
                            styles.push(PhantomText {
                                kind: PhantomTextKind::InlayHint,
                                col,
                                text,
                                fg: Some(self.config.inlay_hint_fg),
                                // font_family:
                                // Some(self.config.inlay_hint_font_family()),
                                font_size: Some(self.config.inlay_hint_font_size()),
                                bg: Some(self.config.inlay_hint_bg),
                                under_line: None,
                                final_col: col,
                                line,
                                visual_merge_col: col,
                                origin_merge_col: col,
                            })
                        }
                    }
                }
                styles
            })
            .unwrap_or_default();

        let (completion_line, completion_col) = self.completion_pos;
        let completion_text = self.config
            .enable_completion_lens
            .then_some(())
            .and(self.completion_lens.as_ref())
            // TODO: We're probably missing on various useful completion things to include here!
            .filter(|_| {
                line == completion_line
                    && !folded_ranges.contain_position(Position {
                    line: completion_line as u32,
                    character: completion_col as u32,
                })
            })
            .map(|completion| PhantomText {
                kind: PhantomTextKind::Completion,
                col: completion_col,
                text: completion.clone(),
                fg: Some(self.config.completion_lens_foreground),
                font_size: Some(self.config.completion_lens_font_size()),
                // font_family: Some(self.config.editor.completion_lens_font_family()),
                bg: None,
                under_line: None,
                final_col: completion_col,
                line,
                visual_merge_col: completion_col,
                // TODO: italics?
                origin_merge_col: completion_col,
            });
        if let Some(completion_text) = completion_text {
            text.push(completion_text);
        }
        let inline_completion_text = self
            .config
            .enable_inline_completion
            .then_some(())
            .and(self.inline_completion.as_ref())
            .filter(|(_, inline_completion_line, inline_completion_col)| {
                line == *inline_completion_line
                    && !folded_ranges.contain_position(Position {
                        line:      *inline_completion_line as u32,
                        character: *inline_completion_col as u32,
                    })
            })
            .map(|(completion, _, inline_completion_col)| {
                PhantomText {
                    kind: PhantomTextKind::Completion,
                    col: *inline_completion_col,
                    text: completion.clone(),
                    fg: Some(self.config.completion_lens_foreground),
                    font_size: Some(self.config.completion_lens_font_size()),
                    bg: None,
                    under_line: None,
                    final_col: *inline_completion_col,
                    line,
                    visual_merge_col: *inline_completion_col, // TODO: italics?
                    origin_merge_col: *inline_completion_col,
                }
            });
        if let Some(inline_completion_text) = inline_completion_text {
            text.push(inline_completion_text);
        }

        if let Some(preedit) = preedit_phantom.as_ref() {
            if preedit.line == line {
                text.push(preedit.clone());
            }
        }

        let fg = self.config.inlay_hint_fg;
        let font_size = self.config.inlay_hint_font_size();
        let bg = self.config.inlay_hint_bg;

        // may be one more phantom_text?
        text.append(&mut folded_ranges.phantom_text(
            line as u32,
            buffer,
            font_size,
            fg,
            bg,
        )?);

        PhantomTextLine::new(line, origin_text_len, start_offset, text)
    }

    // #[allow(clippy::too_many_arguments)]
    // fn new_text_layout_2(
    //     &self,
    //     line: usize,
    //     origins: &[OriginLine],
    //     attrs: Attrs,
    //     line_ending: &'static str,
    //     last_line: usize
    // ) -> Result<TextLayoutLine> {
    //     let origin_line =
    //         origins.get(line).ok_or(anyhow!("origins {line} empty"))?;
    //
    //     let mut line_content = String::new();
    //
    //     {
    //         let line_content_original = self.buffer().line_content(line)?;
    //         util::push_strip_suffix(&line_content_original, &mut line_content);
    //     }
    //
    //     let mut collapsed_line_col = origin_line.phantom.folded_line();
    //     let mut phantom_text = PhantomTextMultiLine::new(
    //         origin_line.phantom.clone(),
    //         line == last_line
    //     );
    //
    //     let mut attrs_list = AttrsList::new(attrs);
    //     // let mut font_system = FONT_SYSTEM.lock();
    //     let mut semantic_styles = origin_line.semantic_styles(0);
    //     let mut diagnostic_styles = origin_line.diagnostic_styles(0);
    //
    //     while let Some(collapsed_line) = collapsed_line_col.take() {
    //         {
    //             util::push_strip_suffix(
    //                 self.buffer().line_content(collapsed_line)?.as_ref(),
    //                 &mut line_content
    //             );
    //         }
    //         let offset_col = phantom_text.origin_text_len;
    //         let next_origin_line = origins
    //             .get(collapsed_line)
    //             .ok_or(anyhow!("origins {line} empty"))?;
    //         let next_phantom_text = next_origin_line.phantom.clone();
    //         collapsed_line_col = next_phantom_text.folded_line();
    //         semantic_styles.extend(next_origin_line.semantic_styles(offset_col));
    //         diagnostic_styles.extend(next_origin_line.
    // diagnostic_styles(offset_col));         let is_last_line =
    // next_phantom_text.line == last_line;         phantom_text.
    // merge(next_phantom_text, is_last_line);     }
    //
    //     let phantom_color = self.editor_style.phantom_color();
    //     phantom_text.add_phantom_style(
    //         &mut attrs_list,
    //         attrs.font_size(attrs.font_size - 1.0),
    //         phantom_color
    //     );
    //     let final_line_content = phantom_text.final_line_content(&line_content);
    //     self.apply_semantic_styles_2(
    //         &phantom_text,
    //         &semantic_styles,
    //         &mut attrs_list,
    //         attrs
    //     );
    //     let text_layout = TextLayout::new_without_init(
    //         line,
    //         &final_line_content,
    //         attrs_list,
    //         None,
    //         Wrap::WordOrGlyph,
    //         line_ending
    //     );
    //     // drop(font_system);
    //     // match self.editor_style.wrap_method() {
    //     //     WrapMethod::None => {},
    //     //     WrapMethod::EditorWidth => {
    //     //         text_layout.set_wrap(Wrap::WordOrGlyph);
    //     //         text_layout.set_size(self.viewport_size.width as f32,
    // f32::MAX);     //     },
    //     //     WrapMethod::WrapWidth { width } => {
    //     //         text_layout.set_wrap(Wrap::WordOrGlyph);
    //     //         text_layout.set_size(width, f32::MAX);
    //     //     },
    //     //     // TODO:
    //     //     WrapMethod::WrapColumn { .. } => {}
    //     // }
    //     let indent = 0.0;
    //     let layout_line = TextLayoutLine::new(
    //         text_layout.into(),
    //         None,
    //         indent,
    //         phantom_text,
    //         semantic_styles,
    //         diagnostic_styles
    //     );
    //     Ok(layout_line)
    // }

    fn init_origin_line_2(
        &self,
        current_line: usize,
        semantic_styles: Option<&mut Peekable<SpanIter<String>>>,
        inlay_hints: Option<&mut Peekable<SpanIter<InlayHint>>>,
        folded_ranges: &mut FoldingRangesLine,
        preedit_phantom: &Option<PhantomText>,
    ) -> Result<OriginLine> {
        let start_offset = self.buffer().offset_of_line(current_line)?;
        let end_offset = self.buffer().offset_of_line(current_line + 1)?;
        // let mut fg_styles = Vec::new();
        // 用于存储该行的最高诊断级别。最后决定该行的背景色
        // let mut max_severity: Option<DiagnosticSeverity> = None;
        // fg_styles.extend(self.get_line_diagnostic_styles(
        //     start_offset,
        //     end_offset,
        //     &mut max_severity,
        //     0,
        // ));

        let phantom_text = self.phantom_text_2(
            current_line,
            inlay_hints,
            start_offset,
            end_offset,
            folded_ranges,
            preedit_phantom,
        )?;
        let semantic_styles = semantic_styles
            .map(|x| {
                let mut styles = vec![];
                loop {
                    if let Some((Interval { start, .. }, _)) = x.peek() {
                        if end_offset <= *start {
                            break;
                        }
                    } else {
                        break;
                    }
                    if let Some((Interval { start, end }, fg_color)) = x.next() {
                        if start_offset <= start && end < end_offset {
                            let Some(color) =
                                self.config.syntax_style_color(fg_color)
                            else {
                                continue;
                            };
                            styles.push(NewLineStyle {
                                origin_line: current_line,
                                origin_line_offset_start: start - start_offset,
                                len: end - start,
                                start_of_buffer: start,
                                end_of_buffer: end,
                                fg_color: color, /* folded_line_offset_start:
                                                  * start - line_start,
                                                  * folded_line_offset_end: end -
                                                  * line_start */
                            });
                        }
                    }
                }
                styles
            })
            .unwrap_or_default();
        // let semantic_styles =
        //     self.get_line_semantic_styles(current_line, start_offset, end_offset);
        let diagnostic_styles = self.get_line_diagnostic_styles_2(
            current_line,
            start_offset,
            end_offset,
        );
        Ok(OriginLine {
            line_index: current_line,
            start_offset,
            len: end_offset - start_offset,
            phantom: phantom_text,
            semantic_styles,
            diagnostic_styles,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn new_text_layout_3(
        &self,
        line: usize,
        attrs: Attrs,
        line_ending: &'static str,
        last_line: usize,
        all_semantic_styles: &mut Option<Peekable<SpanIter<String>>>,
        all_inlay_hints: &mut Option<Peekable<SpanIter<InlayHint>>>,
        folded_ranges: &mut FoldingRangesLine,
        preedit_phantom: &Option<PhantomText>,
    ) -> Result<TextLayoutLine> {
        let origin_line = self.init_origin_line_2(
            line,
            all_semantic_styles.as_mut(),
            all_inlay_hints.as_mut(),
            folded_ranges,
            preedit_phantom,
        )?;
        // origins.get(line).ok_or(anyhow!("origins {line} empty"))?;

        let mut line_content = String::new();

        {
            let line_content_original = self.buffer().line_content(line)?;
            util::push_strip_suffix(&line_content_original, &mut line_content);
        }

        let mut collapsed_line_col = origin_line.phantom.folded_line();
        let mut phantom_text = PhantomTextMultiLine::new(
            origin_line.phantom.clone(),
            line == last_line,
        );

        let mut attrs_list = AttrsList::new(attrs);
        // let mut font_system = FONT_SYSTEM.lock();
        let mut semantic_styles = origin_line.semantic_styles(0);
        let mut diagnostic_styles = origin_line.diagnostic_styles(0);

        while let Some(collapsed_line) = collapsed_line_col.take() {
            {
                util::push_strip_suffix(
                    self.buffer().line_content(collapsed_line)?.as_ref(),
                    &mut line_content,
                );
            }
            let offset_col = phantom_text.origin_text_len;
            let next_origin_line = self.init_origin_line_2(
                collapsed_line,
                all_semantic_styles.as_mut(),
                all_inlay_hints.as_mut(),
                folded_ranges,
                preedit_phantom,
            )?;

            semantic_styles.extend(next_origin_line.semantic_styles(offset_col));
            diagnostic_styles.extend(next_origin_line.diagnostic_styles(offset_col));
            let next_phantom_text = next_origin_line.phantom;
            collapsed_line_col = next_phantom_text.folded_line();
            let is_last_line = next_phantom_text.line == last_line;
            phantom_text.merge(next_phantom_text, is_last_line);
        }

        let phantom_color = self.editor_style.phantom_color();
        phantom_text.add_phantom_style(
            &mut attrs_list,
            attrs.font_size(attrs.font_size - 1.0),
            phantom_color,
        );
        let final_line_content = phantom_text.final_line_content(&line_content);
        self.apply_semantic_styles_2(
            &phantom_text,
            &semantic_styles,
            &mut attrs_list,
            attrs,
        );
        let text_layout = TextLayout::new_without_init(
            line,
            &final_line_content,
            attrs_list,
            None,
            Wrap::WordOrGlyph,
            line_ending,
        );
        // drop(font_system);
        // match self.editor_style.wrap_method() {
        //     WrapMethod::None => {},
        //     WrapMethod::EditorWidth => {
        //         text_layout.set_wrap(Wrap::WordOrGlyph);
        //         text_layout.set_size(self.viewport_size.width as f32, f32::MAX);
        //     },
        //     WrapMethod::WrapWidth { width } => {
        //         text_layout.set_wrap(Wrap::WordOrGlyph);
        //         text_layout.set_size(width, f32::MAX);
        //     },
        //     // TODO:
        //     WrapMethod::WrapColumn { .. } => {}
        // }
        let indent = 0.0;
        let layout_line = TextLayoutLine::new(
            text_layout.into(),
            None,
            indent,
            phantom_text,
            semantic_styles,
            diagnostic_styles,
        );
        Ok(layout_line)
    }

    // pub fn update_folding_item(&mut self, item: FoldingDisplayItem)
    // {     match item.ty {
    //         FoldingDisplayType::UnfoldStart |
    // FoldingDisplayType::Folded => {
    // self.folding_ranges.0.iter_mut().find_map(|range| {
    //                 if range.start == item.position {
    //                     range.status.click();
    //                     Some(())
    //                 } else {
    //                     None
    //                 }
    //             });
    //         }
    //         FoldingDisplayType::UnfoldEnd => {
    //             self.folding_ranges.0.iter_mut().find_map(|range| {
    //                 if range.end == item.position {
    //                     range.status.click();
    //                     Some(())
    //                 } else {
    //                     None
    //                 }
    //             });
    //         }
    //     }
    //     self.update_lines();
    // }

    fn trigger_signals(&mut self) {
        self.signals.trigger();
    }

    pub fn trigger_signals_force(&mut self) {
        self.signals.trigger_force();
    }

    // pub fn update_folding_ranges(&mut self, new: Vec<FoldingRange>)
    // {     self.folding_ranges.update_ranges(new);
    //     self.update_lines();
    // }

    fn update_completion_lens(&mut self, delta: &RopeDelta) -> Result<()> {
        let Some(completion) = &mut self.completion_lens else {
            return Ok(());
        };
        let (line, col) = self.completion_pos;
        let offset = self.signals.buffer.val().offset_of_line_col(line, col)?;
        if delta.as_simple_insert().is_some() {
            let (iv, new_len) = delta.summary();
            if iv.start() == iv.end()
                && iv.start() == offset
                && new_len <= completion.len()
            {
                // Remove the # of newly inserted characters
                // These aren't necessarily the same as the characters
                // literally in the text, but the
                // completion will be updated when the completion
                // widget receives the update event,
                // and it will fix this if needed.
                // TODO: this could be smarter and use the insert's
                // content
                self.completion_lens = Some(completion[new_len..].to_string());
            }
        }

        // Shift the position by the rope delta
        let mut transformer = Transformer::new(delta);

        let new_offset = transformer.transform(offset, true);
        let new_pos = self.buffer().offset_to_line_col(new_offset)?;
        self.completion_pos = new_pos;
        Ok(())
    }

    /// init by lsp
    fn init_diagnostics_with_buffer(&self) -> Result<()> {
        let len = self.buffer().len();
        let diagnostics = self.diagnostics.diagnostics.get_untracked();
        let mut span = SpansBuilder::new(len);
        for diag in diagnostics.into_iter() {
            let start = self.buffer().offset_of_position(&diag.range.start)?;
            let end = self.buffer().offset_of_position(&diag.range.end)?;
            // warn!("start={start} end={end} {:?}", diag);
            span.add_span(Interval::new(start, end), diag);
        }
        let span = span.build();
        self.diagnostics.diagnostics_span.set(span);
        Ok(())
    }

    fn update_diagnostics(&mut self, delta: &RopeDelta) {
        if self
            .diagnostics
            .diagnostics
            .with_untracked(|d| d.is_empty())
        {
            return;
        }

        self.diagnostics.diagnostics_span.update(|diagnostics| {
            diagnostics.apply_shape(delta);
        });
    }

    // /// 语义的样式和方括号的样式
    // fn line_semantic_styles(
    //     &self,
    //     line: usize,
    // ) -> Option<Vec<(usize, usize, Color)>> {
    //     let mut styles: Vec<(usize, usize, Color)> =
    //         self.line_style(line)?;
    //     if let Some(bracket_styles) =
    // self.parser.bracket_pos.get(&line) {         let mut
    // bracket_styles = bracket_styles             .iter()
    //             .filter_map(|bracket_style| {
    //                 if let Some(fg_color) =
    // bracket_style.fg_color.as_ref() {                     if
    // let Some(fg_color) = self.config.syntax_style_color(fg_color) {
    //                         return Some((
    //                             bracket_style.start,
    //                             bracket_style.end,
    //                             fg_color,
    //                         ));
    //                     }
    //                 }
    //                 None
    //             })
    //             .collect();
    //         styles.append(&mut bracket_styles);
    //     }
    //     Some(styles)
    // }

    // // 文本样式，前景色
    // fn line_style(
    //     &self,
    //     line: usize,
    // ) -> Option<Vec<(usize, usize, Color)>> {
    //     // let styles = self.styles();
    //     let styles = self.line_styles.get(&line)?;
    //     Some(
    //         styles
    //             .iter()
    //             .filter_map(|x| {
    //                 if let Some(fg) = &x.fg_color {
    //                     if let Some(color) =
    // self.config.syntax_style_color(fg) {
    // return Some((
    // x.origin_line_offset_start,
    // x.origin_line_offset_end,
    // color,                         ));
    //                     }
    //                 }
    //                 None
    //             })
    //             .collect(),
    //     )
    // }

    // fn indent_line(
    //     &self,
    //     line: usize,
    //     line_content: &str,
    // ) -> usize {
    //     if line_content.trim().is_empty() {
    //         let offset = self.buffer.offset_of_line(line);
    //         if let Some(offset) = self.syntax.parent_offset(offset)
    // {             return self.buffer.line_of_offset(offset);
    //         }
    //     }
    //     line
    // }

    // pub fn _compute_screen_lines(
    //     &mut self,
    //     base: Rect,
    //     view_kind: EditorViewKind
    // ) -> (ScreenLines, Vec<FoldingDisplayItem>) {
    //     info!("_compute_screen_lines base={base:?} kind={view_kind:?}");
    //     // TODO: this should probably be a get since we need to depend
    //     // on line-height let doc_lines =
    //     // doc.doc_lines.get_untracked();
    //     // let view_kind = self.kind.get_untracked();
    //     // let base = self.screen_lines().base;
    //
    //     let line_height = self.config.line_height;
    //     let (y0, y1) = (base.y0, base.y1);
    //     // Get the start and end (visual) lines that are visible in
    //     // the viewport
    //     let min_val = (y0 / line_height as f64).floor() as usize;
    //     let max_val = (y1 / line_height as f64).floor() as usize;
    //     let screen_lines =
    //         self.visual_lines(min_val, max_val, view_kind, line_height, y0,
    // base);     let display_items =
    // self.folding_ranges.to_display_items(&screen_lines);     self.signals.
    // trigger();     (screen_lines, display_items)
    // }

    // pub fn viewport(&self) -> Rect {
    //     self.screen_lines().base
    // }

    pub fn log(&self) {
        info!(
            "DocLines viewport={:?} buffer.rev={} buffer.len()=[{}] \
             style_from_lsp={} is_pristine={} line_height={}",
            self.viewport_size,
            self.buffer().rev(),
            self.buffer().text().len(),
            self.style_from_lsp,
            self.buffer().is_pristine(),
            self.config.line_height
        );
        // info!("{:?}", self.config);
        // for origin_lines in &self.origin_lines {
        //     info!("{:?}", origin_lines);
        // }
        // self._log_folded_lines();
        // self._log_visual_lines();
        // self._log_screen_lines();
        // info!("folding_items");
        // for item in self.signals.folding_items.val() {
        //     info!("{:?}", item);
        // }
        // self._log_folding_ranges();
    }

    pub fn _log_folding_ranges(&self) {
        info!("folding_ranges");
        for range in &self.folding_ranges.0 {
            info!("{:?}", range);
        }
    }

    // pub fn _log_folded_lines(&self) {
    //     for origin_folded_line in &self.origin_folded_lines {
    //         info!("{:?}", origin_folded_line);
    //     }
    // }

    // pub fn _log_screen_lines(&self) {
    //     info!("screen_lines");
    //     info!("base={:?}", self.screen_lines().base);
    //     for visual_line in &self.screen_lines().visual_lines {
    //         info!("{:?}", visual_line);
    //     }
    // }

    // pub fn _log_visual_lines(&self) {
    //     for visual_line in &self.visual_lines {
    //         info!("{:?}", visual_line);
    //     }
    // }

    fn apply_semantic_styles_2(
        &self,
        phantom_text: &PhantomTextMultiLine,
        semantic_styles: &[NewLineStyle],
        attrs_list: &mut AttrsList,
        attrs: Attrs,
    ) {
        for NewLineStyle {
            fg_color,
            start_of_buffer,
            end_of_buffer,
            ..
        } in semantic_styles.iter()
        {
            let origin_line_offset_end =
                *end_of_buffer - phantom_text.offset_of_line;
            match (
                phantom_text.final_col_of_origin_merge_col(
                    *start_of_buffer - phantom_text.offset_of_line,
                ),
                phantom_text.final_col_of_origin_merge_col(origin_line_offset_end),
            ) {
                (Ok(Some(start)), Ok(Some(end))) => {
                    attrs_list.add_span(start..end, attrs.color(*fg_color));
                },
                // (Err(err), _) => {
                //     error!("{}: {}", err.to_string(), *origin_line_offset_start);
                //     continue
                // }
                // (_, Err(err)) => {
                //     error!("{}: {}", err.to_string(), origin_line_offset_end);
                //     continue
                // }
                _ => {
                    // maybe be folded
                    continue;
                },
            }
            // // for (start, end, color) in styles.into_iter() {
            // let (Some(start), Some(end)) = (
            //     phantom_text.final_col_of_merge_col(*
            // origin_line_offset_start),     phantom_text.
            // final_col_of_merge_col(*origin_line_offset_start + *len)
            // ) else {
            //     continue;
            // };
            // attrs_list.add_span(start..end, attrs.color(*fg_color));
        }
    }

    // fn apply_diagnostic_styles(
    //     &self,
    //     layout_line: &mut TextLayoutLine,
    //     line_styles: Vec<(usize, usize, Color)>,
    //     // _max_severity: Option<DiagnosticSeverity>,
    // ) {
    //     let layout = &layout_line.text;
    //     let phantom_text = &layout_line.phantom_text;
    //
    //     // 暂不考虑
    //     for (start, end, color) in line_styles {
    //         // warn!("line={} start={start}, end={end},
    // color={color:?}", phantom_text.line);         //
    // col_at(end)可以为空，因为end是不包含的         let
    // (Some(start), Some(end)) = (phantom_text.col_at(start),
    // phantom_text.col_at(end.max(1) - 1)) else {
    // warn!("line={} start={start}, end={end}, color={color:?} col_at
    // empty", phantom_text.line);             continue;
    //         };
    //         let styles =
    //             util::extra_styles_for_range(layout, start, end +
    // 1, None, None, Some(color));         layout_line.
    // extra_style.extend(styles);     }
    //
    //     // 不要背景色，因此暂时comment
    //     // Add the styling for the diagnostic severity, if
    // applicable     // if let Some(max_severity) = max_severity
    // {     //     let size = layout_line.text.size();
    //     //     let x1 = if !config.error_lens_end_of_line {
    //     //         let error_end_x = size.width;
    //     //         Some(error_end_x.max(size.width))
    //     //     } else {
    //     //         None
    //     //     };
    //     //
    //     //     // TODO(minor): Should we show the background only
    // on wrapped lines that have the     //     // diagnostic
    // actually on that line?     //     // That would make it
    // more obvious where it is from and matches other editors.
    //     //     layout_line.extra_style.push(LineExtraStyle {
    //     //         x: 0.0,
    //     //         y: 0.0,
    //     //         width: x1,
    //     //         height: size.height,
    //     //         bg_color:
    // Some(self.config.color_of_error_lens(max_severity)),     //
    // under_line: None,     //         wave_line: None,
    //     //     });
    //     // }
    // }

    // /// return (line,start, end, color)
    // pub fn get_line_diagnostic_styles(
    //     &self,
    //     start_offset: usize,
    //     end_offset: usize,
    //     max_severity: &mut Option<DiagnosticSeverity>,
    //     line_offset: usize,
    // ) -> Vec<(usize, usize, Color)> {
    //     self.config
    //         .enable_error_lens
    //         .then_some(())
    //         .map(|_| {
    //             self.diagnostics.diagnostics_span.with_untracked(|diags| {
    //                 diags
    //                     .iter_chunks(start_offset..end_offset)
    //                     .filter_map(|(iv, diag)| {
    //                         let start = iv.start();
    //                         let end = iv.end();
    //                         let severity = diag.severity?;
    //                         // warn!("start_offset={start_offset}
    //                         // end_offset={end_offset}
    //                         // interval={iv:?}");
    //                         if start <= end_offset
    //                             && start_offset <= end
    //                             && severity < DiagnosticSeverity::HINT
    //                         {
    //                             match (severity, *max_severity) {
    //                                 (severity, Some(max)) => {
    //                                     if severity < max {
    //                                         *max_severity = Some(severity);
    //                                     }
    //                                 },
    //                                 (severity, None) => {
    //                                     *max_severity = Some(severity);
    //                                 },
    //                             }
    //                             let color =
    //                                 self.config.color_of_diagnostic(severity)?;
    //                             Some((
    //                                 start + line_offset - start_offset,
    //                                 end + line_offset - start_offset,
    //                                 color,
    //                             ))
    //                         } else {
    //                             None
    //                         }
    //                     })
    //                     .collect()
    //             })
    //         })
    //         .unwrap_or_default()
    // }

    /// return (line,start, end, color)
    fn get_line_diagnostic_styles_2(
        &self,
        origin_line: usize,
        start_offset: usize,
        end_offset: usize, /* max_severity: &mut
                            * Option<DiagnosticSeverity>, */
    ) -> Vec<NewLineStyle> {
        self.config
            .enable_error_lens
            .then_some(())
            .map(|_| {
                self.diagnostics.diagnostics_span.with_untracked(|diags| {
                    diags
                        .iter_chunks(start_offset..end_offset)
                        .filter_map(|(iv, diag)| {
                            let start = iv.start();
                            let end = iv.end();
                            let severity = diag.severity?;
                            // ?
                            if start_offset <= start
                                && end <= end_offset
                                && severity < DiagnosticSeverity::HINT
                            {
                                let color =
                                    self.config.color_of_diagnostic(severity)?;
                                Some(NewLineStyle {
                                    origin_line,
                                    origin_line_offset_start: start_offset,
                                    len: end - start,
                                    start_of_buffer: start,
                                    end_of_buffer: end,
                                    fg_color: color, /* folded_line_offset_start:
                                                      * start -
                                                      * start_offset,
                                                      * folded_line_offset_end:
                                                      * end
                                                      * - start_offset */
                                })
                            } else {
                                None
                            }
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    fn update_inlay_hints(&mut self, delta: &RopeDelta) {
        if let Some(hints) = self.inlay_hints.as_mut() {
            hints.apply_shape(delta);
        }
    }

    pub fn move_right(
        &self,
        buffer_offset: usize,
        affinity: CursorAffinity,
    ) -> Result<Option<(usize, CursorAffinity)>> {
        // if matches!(affinity, CursorAffinity::Backward) {
        //     return Ok(Some((buffer_offset, CursorAffinity::Forward)));
        // }
        if buffer_offset == self.buffer().len() {
            // last line is empty
            return Ok(None);
        }

        let folded_line = self.folded_line_of_buffer_offset(buffer_offset)?;
        debug!("{folded_line:?}");
        let origin_merge_col = buffer_offset - folded_line.origin_interval.start;

        let mut iter = folded_line.text().iter();
        // find text_of_merge_col
        while let Some(text) = iter.next() {
            match text {
                Text::Phantom { text } => {
                    if text.origin_merge_col <= origin_merge_col
                        && origin_merge_col <= text.next_origin_merge_col()
                    {
                        if matches!(affinity, CursorAffinity::Backward) {
                            return Ok(Some((
                                buffer_offset,
                                CursorAffinity::Forward,
                            )));
                        } else {
                            // next merge col
                            while let Some(text) = iter.next() {
                                if let Text::OriginText { text } = text {
                                    if folded_line.is_last_char(text.final_col.start)
                                    {
                                        break;
                                    } else {
                                        return Ok(Some((
                                            text.origin_merge_col_start()
                                                + folded_line.offset_of_line()
                                                + 1,
                                            CursorAffinity::Backward,
                                        )));
                                    }
                                }
                            }
                            // next line
                            return Ok(Some((
                                folded_line.origin_interval.end,
                                CursorAffinity::Backward,
                            )));
                        }
                    }
                },
                Text::OriginText { text } => {
                    if text.origin_merge_col_contains(
                        origin_merge_col,
                        folded_line.last_line,
                    ) {
                        let final_col = text.final_col.start
                            + (origin_merge_col - text.origin_merge_col_start());
                        if folded_line.is_last_char(final_col) {
                            // 换行
                            return Ok(Some((
                                folded_line.origin_interval.end,
                                CursorAffinity::Backward,
                            )));
                        } else {
                            return Ok(Some((
                                buffer_offset + 1,
                                CursorAffinity::Backward,
                            )));
                        }
                    }
                },
                Text::EmptyLine { .. } => {
                    unreachable!()
                },
            }
        }
        Err(anyhow!(
            "move_right buffer_offset={buffer_offset}, affinity={affinity:?} error"
        ))
    }

    pub fn move_left(
        &self,
        buffer_offset: usize,
        affinity: CursorAffinity,
    ) -> Result<Option<(usize, CursorAffinity)>> {
        if buffer_offset == 0 {
            return Ok(None);
        }

        let folded_line = self.folded_line_of_buffer_offset(buffer_offset)?;
        let origin_merge_col = buffer_offset - folded_line.origin_interval.start;

        // let mut iter = folded_line.text().iter();
        let mut previous_text: Option<&Text> = None;
        // find text_of_merge_col
        for text in folded_line.text() {
            match text {
                Text::Phantom { text: phantom_text } => {
                    if phantom_text.origin_merge_col <= origin_merge_col
                        && origin_merge_col <= phantom_text.next_origin_merge_col()
                    {
                        if matches!(affinity, CursorAffinity::Forward) {
                            return Ok(Some((
                                buffer_offset,
                                CursorAffinity::Backward,
                            )));
                        } else if let Some(previous_text) = previous_text.take() {
                            match previous_text {
                                Text::Phantom { text: previous } => {
                                    return Ok(Some((
                                        previous.origin_merge_col
                                            + folded_line.origin_interval.start,
                                        CursorAffinity::Backward,
                                    )));
                                },
                                Text::OriginText { text: previous } => {
                                    return Ok(Some((
                                        previous.origin_merge_col_end()
                                            + folded_line.origin_interval.start
                                            - 1,
                                        CursorAffinity::Backward,
                                    )));
                                },
                                _ => {
                                    bail!("unreachable")
                                },
                            }
                        } else {
                            // previous line
                            break;
                        }
                    } else if !previous_text
                        .as_ref()
                        .map(|x| x.is_phantom())
                        .unwrap_or_default()
                    {
                        previous_text = Some(text);
                    }
                },
                Text::OriginText { text: origin_text } => {
                    if origin_text.origin_merge_col_contains(
                        origin_merge_col,
                        folded_line.last_line,
                    ) {
                        if origin_merge_col
                            <= origin_text.origin_merge_col_start() + 1
                        {
                            if let Some(previous_text) = previous_text.take() {
                                //  if true {...} |else {...}
                                match previous_text {
                                    Text::Phantom { text } => {
                                        return Ok(Some((
                                            text.origin_merge_col
                                                + folded_line.origin_interval.start,
                                            CursorAffinity::Forward,
                                        )));
                                    },
                                    _ => {
                                        bail!("unreachable")
                                    },
                                }
                            } else if origin_merge_col
                                == origin_text.origin_merge_col_start() + 1
                            {
                                //s|truct A;
                                return Ok(Some((
                                    origin_text.origin_merge_col_start()
                                        + folded_line.origin_interval.start,
                                    CursorAffinity::Backward,
                                )));
                            } else {
                                // previous line
                                break;
                            }
                        } else {
                            return Ok(Some((
                                buffer_offset - 1,
                                CursorAffinity::Backward,
                            )));
                        }
                    } else {
                        previous_text = Some(text);
                    }
                },
                Text::EmptyLine { .. } => {
                    // previous line
                    break;
                },
            }
        }
        if folded_line.line_index == 0 {
            return Ok(None);
        }
        debug!("{folded_line:?}");
        let Some(previous) =
            self.init_folded_line_layout_alone_by_index(folded_line.line_index - 1)?
        else {
            bail!("unreachable")
        };
        debug!("{previous:?}");
        let Some(text) = previous.text().last() else {
            bail!("unreachable")
        };
        match text {
            Text::Phantom { text } => Ok(Some((
                text.origin_merge_col + previous.origin_interval.start,
                CursorAffinity::Forward,
            ))),
            Text::OriginText { text } => {
                let line_end = previous.len() - previous.len_without_rn();
                Ok(Some((
                    text.origin_merge_col_end() + previous.origin_interval.start
                        - line_end,
                    CursorAffinity::Backward,
                )))
            },
            Text::EmptyLine { .. } => {
                bail!("unreachable")
            },
        }
    }

    pub fn move_up(
        &self,
        offset: usize,
        affinity: CursorAffinity,
        horiz: Option<ColPosition>,
        _mode: Mode,
        _count: usize,
    ) -> Result<Option<(usize, ColPosition, CursorAffinity)>> {
        let (visual_line, final_col, ..) = if offset >= self.buffer().len() {
            let folded_line =
                self.init_folded_line_layout_alone(self.buffer().last_line())?;
            let final_col = folded_line.len_without_rn();
            (folded_line, final_col)
        } else {
            self.folded_line_and_final_col_of_offset(offset, affinity)?
        };

        let horiz = horiz.unwrap_or(ColPosition::Col(final_col));
        let Some(previous_visual_line) = self
            .init_folded_line_layout_alone_by_index(
                visual_line.line_index.max(1) - 1,
            )?
        else {
            return Ok(None);
        };
        let (offset_of_buffer, affinity) = self.rvline_horiz_col(
            &horiz,
            _mode != Mode::Normal,
            &previous_visual_line,
        )?;

        // let Some((_previous_visual_line, final_col, offset_of_buffer)) =
        // self.previous_visual_line(     visual_line.line_index,
        //     final_col,
        //     affinity
        // ) else {
        //     return Ok(None);
        // };

        // // TODO: this should maybe be doing `new_offset ==
        // // info.interval.start`?
        // let affinity = if line_offset == 0 {
        //     CursorAffinity::Forward
        // } else {
        //     CursorAffinity::Backward
        // };
        Ok(Some((offset_of_buffer, horiz, affinity)))
    }

    pub fn end_of_line(
        &self,
        affinity: &mut CursorAffinity,
        offset: usize,
        _mode: Mode,
    ) -> Result<(usize, ColPosition)> {
        let (origin_folded_line, ..) =
            self.folded_line_and_final_col_of_offset(offset, *affinity)?;
        // let new_col = info.last_col(view.text_prov(), mode !=
        // Mode::Normal); let vline_end =
        // vl.visual_interval.end; let start_offset =
        // vl.visual_interval.start; // If these subtractions
        // crash, then it is likely due to a bad vline being kept
        // around // somewhere
        // let new_col = if mode == Mode::Normal &&
        // !vl.visual_interval.is_empty() {
        //     let vline_pre_end =
        // self.buffer().prev_grapheme_offset(vline_end, 1, 0);
        //     vline_pre_end - start_offset
        // } else {
        //     vline_end - start_offset
        // };

        // let origin_folded_line = self
        //     .origin_folded_lines
        //     .get(vl.origin_folded_line)
        //     .ok_or(anyhow!("origin_folded_line is not exist"))?;
        *affinity = if origin_folded_line.origin_interval.is_empty() {
            CursorAffinity::Forward
        } else {
            CursorAffinity::Backward
        };
        let new_offset = self.buffer().offset_of_line_col(
            origin_folded_line.origin_line_end,
            origin_folded_line.origin_interval.end,
        )?;

        Ok((new_offset, ColPosition::End))
    }

    pub fn move_down(
        &self,
        offset: usize,
        affinity: CursorAffinity,
        horiz: Option<ColPosition>,
        _mode: Mode,
        _count: usize,
    ) -> Result<Option<(usize, ColPosition, CursorAffinity)>> {
        let (visual_line, final_col, ..) =
            self.folded_line_and_final_col_of_offset(offset, affinity)?;
        // let Some((next_visual_line, final_col, offset_of_buffer, ..)) =
        //     self.next_visual_line(visual_line.line_index, final_col, affinity)
        // else {     return Ok(None);
        // };
        let horiz = horiz.unwrap_or(ColPosition::Col(final_col));
        let Some(next_visual_line) =
            self.init_folded_line_layout_alone_by_index(visual_line.line_index + 1)?
        else {
            return Ok(None);
        };
        let (offset_of_buffer, affinity) =
            self.rvline_horiz_col(&horiz, _mode != Mode::Normal, &next_visual_line)?;
        // let affinity = if next_line_offset == 0 {
        //     CursorAffinity::Forward
        // } else {
        //     CursorAffinity::Backward
        // };
        warn!("offset_of_buffer={offset_of_buffer} horiz={horiz:?}");

        Ok(Some((offset_of_buffer, horiz, affinity)))
    }

    /// return offset of buffer
    fn rvline_horiz_col(
        &self,
        horiz: &ColPosition,
        _caret: bool,
        visual_line: &OriginFoldedLine,
    ) -> Result<(usize, CursorAffinity)> {
        Ok(match *horiz {
            ColPosition::Col(final_col) => {
                visual_line.cursor_position_of_final_col(final_col)
            },
            ColPosition::End => {
                (visual_line.len_without_rn(), CursorAffinity::Forward)
            },
            ColPosition::Start => (0, CursorAffinity::Forward),
            ColPosition::FirstNonBlank => {
                // ?
                let Some(final_offset) = visual_line.first_no_whitespace() else {
                    return Ok((
                        visual_line.len_without_rn(),
                        CursorAffinity::Forward,
                    ));
                };
                (final_offset, CursorAffinity::Backward)
                // let rs = text_layout
                //     .phantom_text
                //     .cursor_position_of_final_col(final_offset);
                // rs.2 + rs.1
            },
        })
    }

    // fn update_screen_lines(&mut self) {
    //     let screen_lines =
    // self._compute_screen_lines(*self.signals.viewport.val());
    //     self.signals.screen_lines.update_force(screen_lines);
    // }

    // fn _compute_change_lines(
    //     &self,
    //     deltas: &[(Rope, RopeDelta, InvalLines)],
    // ) -> Result<OriginLinesDelta> {
    //     if deltas.len() == 1 {
    //         if let Some(delta) = deltas.first() {
    //             return resolve_delta_rs(&delta.0, &delta.1);
    //         }
    //     }
    //     Ok(OriginLinesDelta::default())
    // }

    // /// return [start...end), (start...end]
    // #[allow(clippy::type_complexity)]
    // fn compute_change_lines(
    //     &self,
    //     deltas: &[(Rope, RopeDelta, InvalLines)]
    // ) -> Result<OriginLinesDelta> {
    //     let rs = self._compute_change_lines(deltas);
    //     rs
    // }

    #[inline]
    pub fn buffer(&self) -> &Buffer {
        self.signals.buffer.val()
    }

    #[inline]
    fn buffer_mut(&mut self) -> &mut Buffer {
        self.signals.buffer.val_mut()
    }
}

type ComputeLines = DocLines;

impl ComputeLines {
    pub fn first_non_blank(
        &self,
        affinity: &mut CursorAffinity,
        offset: usize,
    ) -> Result<(usize, ColPosition)> {
        let (info, ..) =
            self.folded_line_and_final_col_of_offset(offset, *affinity)?;
        let non_blank_offset =
            WordCursor::new(self.buffer().text(), info.origin_interval.start)
                .next_non_blank_char();

        let start_line_offset = info.origin_interval.start;
        // TODO: is this always the correct affinity? It might be
        // desirable for the very first character on a wrapped line?
        *affinity = CursorAffinity::Backward;

        Ok(if offset > non_blank_offset {
            // Jump to the first non-whitespace character if we're
            // strictly after it
            (non_blank_offset, ColPosition::FirstNonBlank)
        } else {
            // If we're at the start of the line, also jump to the
            // first not blank
            if start_line_offset == offset {
                (non_blank_offset, ColPosition::FirstNonBlank)
            } else {
                // Otherwise, jump to the start of the line
                (start_line_offset, ColPosition::Start)
            }
        })
    }

    // pub fn line_point_of_visual_line_col(
    //     &self,
    //     visual_line: usize,
    //     col: usize,
    //     affinity: CursorAffinity,
    //     _force_affinity: bool
    // ) -> Result<Point> {
    //     self._line_point_of_visual_line_col(
    //         visual_line,
    //         col,
    //         affinity,
    //         _force_affinity
    //     )
    //     .ok_or(anyhow!("visual_line={visual_line} col={col} is empty"))
    // }
    //
    // pub fn _line_point_of_visual_line_col(
    //     &self,
    //     visual_line: usize,
    //     col: usize,
    //     affinity: CursorAffinity,
    //     _force_affinity: bool
    // ) -> Option<Point> {
    //     let line = self.origin_folded_lines.get(visual_line)?;
    //     Some(line.hit_position_aff(col, affinity).point)
    // }

    #[allow(clippy::type_complexity)]
    /// return (visual line of offset, offset of visual line, offset
    /// of folded line, is last char, viewport position of cursor,
    /// line_height, origin position of cursor)
    ///
    /// last_char should be check in future
    pub fn cursor_position_of_buffer_offset(
        &self,
        offset: usize,
        affinity: CursorAffinity,
    ) -> Result<Point> {
        let (vl, offset_folded) =
            self.folded_line_and_final_col_of_offset(offset, affinity)?;
        let mut point_of_document =
            vl.hit_position_aff(offset_folded, affinity).point;
        let line_height = self.config.line_height;
        point_of_document.y = (vl.line_index * line_height) as f64;

        // let info = crate::lines::InfoOfBufferOffset {
        //     origin_line,
        //     offset_of_origin_line,
        //     origin_folded_line_index: vl.line_index,
        //     offset_of_origin_folded_line: None,
        //     point_of_document,
        // };
        Ok(point_of_document)
    }

    // pub fn visual_position_of_cursor_position(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity
    // ) -> Result<
    //     Option<(
    //         usize,
    //         bool,
    //         Point,
    //         f64,
    //         Point,
    //         usize
    //     )>
    // > { let Some((offset_folded, last_char, vl)) =
    // > self.visual_info_of_cursor_offset(offset, affinity)? else { return
    // > Ok(None); }; let mut viewpport_point = hit_position_aff(
    // > &vl.text_layout.text, offset_folded, true ) .point; let line_height =
    // > self.screen_lines().line_height; let Some(screen_line) =
    // > self.screen_lines().visual_line_info_for_origin_folded_line(vl.line_index)
    // > else { return Ok(None); };
    //
    //     viewpport_point.y = screen_line.folded_line_y;
    //     viewpport_point.add_assign(self.screen_lines().base.origin().to_vec2());
    //     let mut origin_point = viewpport_point;
    //     origin_point.y = vl.line_index as f64 * line_height;
    //
    //     Ok(Some((
    //         offset_folded,
    //         last_char,
    //         viewpport_point,
    //         line_height,
    //         origin_point,
    //         self.config.line_height
    //     )))
    // }

    // pub fn char_rect_in_viewport(&self, offset: usize) -> Result<Vec<Rect>> {
    //     // let Ok((vl, _col, col_2, _, folded_line)) =
    //     // self.visual_line_of_offset(offset, CursorAffinity::Forward)
    //     // else {     error!("visual_line_of_offset
    //     // offset={offset} not exist");     return None
    //     // };
    //     // let rs = self.screen_lines().
    //     // visual_line_info_of_visual_line(&vl)?; let mut hit0
    //     // = folded_line.text_layout.text.hit_position(col_2);
    //     // let mut hit1 =
    //     // folded_line.text_layout.text.hit_position(col_2 + 1);
    //     // hit0.point.y += rs.y;
    //     // hit1.point.y += rs.y + self.config.line_height as f64;
    //     // Some((hit0.point, hit1.point))
    //     self.normal_selection(offset, offset + 1)
    // }

    // pub fn normal_selection(
    //     &self,
    //     start_offset: usize,
    //     end_offset: usize, screen_lines: &ScreenLines
    // ) -> Result<Vec<Rect>> {
    //     let (folded_line_start, col_start, ..) =
    //         self.folded_line_of_offset(start_offset, CursorAffinity::Forward)?;
    //     let (folded_line_end, col_end, ..) =
    //         self.folded_line_of_offset(end_offset, CursorAffinity::Forward)?;
    //
    //     let Some((rs_start, rs_end)) =
    // screen_lines.intersection_with_lines(folded_line_start.line_index,
    // folded_line_end.line_index) else {         return Ok(vec![]);
    //     };
    //     let base = screen_lines.base.origin().to_vec2();
    //     if folded_line_start.line_index == folded_line_end.line_index {
    //         let rs = folded_line_start.line_scope(
    //             col_start,
    //             col_end,
    //             self.config.line_height as f64,
    //             rs_start.folded_line_y,
    //             base
    //         );
    //         Ok(vec![rs])
    //     } else {
    //
    //         let mut first =
    //             Vec::with_capacity(folded_line_start.line_index -
    // folded_line_end.line_index + 1);         first.push(folded_line_start.
    // line_scope(             col_start,
    //             folded_line_start.len_without_rn(self.buffer().line_ending()),
    //             self.config.line_height as f64,
    //             rs_start.folded_line_y,
    //             base
    //         ));
    //
    //         for vl in &screen_lines.visual_lines {
    //             if vl.visual_line.line_index >= folded_line_end.line_index {
    //                 break;
    //             } else if vl.visual_line.line_index <=
    // folded_line_start.line_index {                 continue;
    //             } else {
    //                 let selection = vl.visual_line.line_scope(
    //                     0,
    //                     vl.visual_line.final_len(),
    //                     self.config.line_height as f64,
    //                     vl.folded_line_y,
    //                     base
    //                 );
    //                 first.push(selection)
    //             }
    //         }
    //         let last = folded_line_end.line_scope(
    //             0,
    //             col_end,
    //             self.config.line_height as f64,
    //             rs_end.folded_line_y,
    //             base
    //         );
    //         first.push(last);
    //         Ok(first)
    //     }
    // }
}

type LinesOnUpdate = DocLines;

impl LinesOnUpdate {
    fn on_update_buffer(&mut self) -> Result<()> {
        if self.syntax.styles.is_some() {
            self.parser
                .update_code(self.signals.buffer.val(), Some(&self.syntax))?;
        } else {
            self.parser.update_code(self.signals.buffer.val(), None)?;
        }
        self.init_diagnostics_with_buffer()?;
        Ok(())
    }

    fn on_update_lines(&mut self) {
        // self.max_width = 0.0;
        // self.origin_folded_lines.iter().for_each(|x| {
        //     if x.size_width().width > self.max_width {
        //         self.max_width = x.size_width().width;
        //     }
        // });

        self.signals
            .last_line
            .update_if_not_equal(self.compute_last_width(
                self.buffer().last_line() + 1,
                self.buffer().line_ending().get_chars(),
            ));
    }

    fn compute_last_width(
        &self,
        last_line: usize,
        line_ending: &'static str,
    ) -> (usize, f64) {
        let family =
            Cow::Owned(FamilyOwned::parse_list(&self.config.font_family).collect());
        // 设置字体属性
        let attrs = self.init_attrs_without_color(&family); // 等宽字体
        let attrs_list = AttrsList::new(attrs);
        let mut font_system = FONT_SYSTEM.lock();
        // 创建文本缓冲区
        let mut text_buffer = TextLayout::new_with_font_system(
            0,
            last_line.to_string(),
            attrs_list,
            &mut font_system,
            line_ending,
        );
        (last_line, text_buffer.size().width)
    }
}

type PubUpdateLines = DocLines;

pub enum EditBuffer<'a> {
    Init(Rope),
    SetLineEnding(LineEnding),
    EditBuffer {
        iter:      &'a [(Selection, &'a str)],
        edit_type: EditType,
    },
    SetPristine(u64),
    Reload {
        content:      Rope,
        set_pristine: bool,
    },
    ExecuteMotionMode {
        cursor:      &'a mut Cursor,
        motion_mode: MotionMode,
        range:       Range<usize>,
        is_vertical: bool,
        register:    &'a mut Register,
    },
    DoEditBuffer {
        cursor:    &'a mut Cursor,
        cmd:       &'a EditCommand,
        modal:     bool,
        register:  &'a mut Register,
        smart_tab: bool,
    },
    DoInsertBuffer {
        cursor: &'a mut Cursor,
        s:      &'a str,
    },
    SetCursor {
        before_cursor: CursorMode,
        after_cursor:  CursorMode,
    },
}

impl Debug for EditBuffer<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EditBuffer::Init(_val) => {
                write!(f, "EditBuffer::Init")
            },
            EditBuffer::SetLineEnding(val) => {
                write!(f, "EditBuffer::SetLineEnding {:?}", val)
            },
            EditBuffer::EditBuffer {
                iter, edit_type, ..
            } => {
                write!(f, "EditBuffer::Init iter {:?} edit_type{edit_type:?}", iter,)
            },
            EditBuffer::SetPristine(val) => {
                write!(f, "EditBuffer::SetPristine {:?}", val)
            },
            EditBuffer::Reload { set_pristine, .. } => {
                write!(f, "EditBuffer::Reload set_pristine {set_pristine:?}")
            },
            EditBuffer::ExecuteMotionMode {
                cursor,
                motion_mode,
                range,
                is_vertical,
                ..
            } => {
                write!(
                    f,
                    "EditBuffer::ExecuteMotionMode {:?} {motion_mode:?} \
                     range={range:?}, is_vertical={is_vertical}",
                    cursor.mode()
                )
            },
            EditBuffer::DoEditBuffer {
                cursor,
                cmd,
                modal,
                smart_tab,
                ..
            } => {
                write!(
                    f,
                    "EditBuffer::DoEditBuffer {:?} {cmd:?} modal={modal} \
                     smart_tab={smart_tab}",
                    cursor.mode()
                )
            },
            EditBuffer::DoInsertBuffer { cursor, s, .. } => {
                write!(f, "EditBuffer::DoInsertBuffer {:?} s={s:?}", cursor.mode())
            },
            EditBuffer::SetCursor {
                before_cursor,
                after_cursor,
            } => {
                write!(
                    f,
                    "EditBuffer::SetCursor before_cursor {before_cursor:?} \
                     after_cursor={after_cursor:?}"
                )
            },
        }
    }
}

impl PubUpdateLines {
    // pub fn buffer_edit(
    //     &mut self,
    //     edit: EditBuffer,
    // ) -> Vec<(Rope, RopeDelta, InvalLines)> {
    //     let rs = self._buffer_edit(edit);
    //     rs
    // }

    pub fn _buffer_edit(
        &mut self,
        edit: EditBuffer,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        let mut rs = Vec::new();
        debug!("buffer_edit {edit:?}, rev={}", self.buffer().rev());
        match edit {
            EditBuffer::Init(content) => {
                let indent =
                    IndentStyle::from_str(self.syntax.language.indent_unit());
                self.buffer_mut().init_content(content);
                self.buffer_mut().detect_indent(|| indent);
            },
            EditBuffer::SetLineEnding(line_ending) => {
                self.buffer_mut().set_line_ending(line_ending);
            },
            EditBuffer::EditBuffer { iter, edit_type } => {
                let delta = self.buffer_mut().edit(iter, edit_type);
                debug!("buffer_edit EditBuffer {:?} {:?}", delta.1, delta.2);
                rs.push(delta);
            },
            EditBuffer::SetPristine(recv) => {
                if recv == self.buffer().rev() {
                    self.buffer_mut().set_pristine();
                    self.signals.pristine.update_if_not_equal(true);
                    self.trigger_signals();
                }
                return rs;
            },
            EditBuffer::Reload {
                content,
                set_pristine,
            } => {
                let delta = self.buffer_mut().reload(content, set_pristine);
                debug!("buffer_edit Reload {:?} {:?}", delta.1, delta.2);
                self.inlay_hints = None;
                self.folding_ranges.0.clear();
                self.semantic_styles = None;
                // line_delta = self._compute_change_lines_one(&rs)?;
                rs.push(delta);
            },
            EditBuffer::ExecuteMotionMode {
                cursor,
                motion_mode,
                range,
                is_vertical,
                register,
            } => {
                rs = Action::execute_motion_mode(
                    cursor,
                    self.buffer_mut(),
                    motion_mode,
                    range,
                    is_vertical,
                    register,
                );
            },
            EditBuffer::DoEditBuffer {
                cursor,
                cmd,
                modal,
                register,
                smart_tab,
            } => {
                let syntax = &self.syntax;
                let mut clipboard = SystemClipboard::new();
                let old_cursor = cursor.mode().clone();
                rs = Action::do_edit(
                    cursor,
                    self.signals.buffer.val_mut(),
                    cmd,
                    &mut clipboard,
                    register,
                    EditConf {
                        comment_token: syntax.language.comment_token(),
                        modal,
                        smart_tab,
                        keep_indent: true,
                        auto_indent: true,
                    },
                );
                if !rs.is_empty() {
                    self.buffer_mut().set_cursor_before(old_cursor);
                    self.buffer_mut().set_cursor_after(cursor.mode().clone());
                }
            },
            EditBuffer::DoInsertBuffer { cursor, s } => {
                let auto_closing_matching_pairs =
                    self.config.auto_closing_matching_pairs;
                let auto_surround = self.config.auto_surround;
                let old_cursor = cursor.mode().clone();
                let syntax = &self.syntax;
                rs = Action::insert(
                    cursor,
                    self.signals.buffer.val_mut(),
                    s,
                    &|buffer, c, offset| {
                        util::syntax_prev_unmatched(buffer, syntax, c, offset)
                    },
                    auto_closing_matching_pairs,
                    auto_surround,
                );
                self.buffer_mut().set_cursor_before(old_cursor);
                self.buffer_mut().set_cursor_after(cursor.mode().clone());
            },
            EditBuffer::SetCursor {
                before_cursor,
                after_cursor,
            } => {
                self.buffer_mut().set_cursor_after(after_cursor);
                self.buffer_mut().set_cursor_before(before_cursor);
                return vec![];
            },
        }
        for delta in &rs {
            self.apply_delta(&delta.1);
        }
        self.signals
            .pristine
            .update_if_not_equal(self.buffer().is_pristine());
        self.signals
            .buffer_rev
            .update_if_not_equal(self.buffer().rev());
        if let Err(err) = self.on_update_buffer() {
            error!("{err}");
        }
        // self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        debug!("after buffer_edit rev={}", self.buffer().rev());
        rs
    }

    // pub fn set_line_ending(&mut self, line_ending: LineEnding) {
    //     self.buffer_edit(EditBuffer::SetLineEnding(line_ending));
    // }

    // pub fn edit_buffer(
    //     &mut self,
    //     iter: &[(Selection, &str)],
    //     edit_type: EditType,
    // ) -> (Rope, RopeDelta, InvalLines) {
    //     self.buffer_edit(EditBuffer::EditBuffer { edit_type, iter })
    //         .remove(0)
    // }

    // pub fn reload_buffer(
    //     &mut self,
    //     content: Rope,
    //     set_pristine: bool,
    // ) -> (Rope, RopeDelta, InvalLines) {
    //     self.buffer_edit(EditBuffer::Reload {
    //         content,
    //         set_pristine,
    //     })
    //     .remove(0)
    // }

    // pub fn set_pristine(&mut self, rev: u64) {
    //     self.buffer_edit(EditBuffer::SetPristine(rev));
    // }

    // pub fn set_cursor(
    //     &mut self,
    //     before_cursor: CursorMode,
    //     after_cursor: CursorMode,
    // ) {
    //     self.buffer_edit(EditBuffer::SetCursor {
    //         before_cursor,
    //         after_cursor,
    //     });
    // }

    // pub fn execute_motion_mode(
    //     &mut self,
    //     cursor: &mut Cursor,
    //     motion_mode: MotionMode,
    //     range: Range<usize>,
    //     is_vertical: bool,
    //     register: &mut Register,
    // ) -> Vec<(Rope, RopeDelta, InvalLines)> {
    //     self.buffer_edit(EditBuffer::ExecuteMotionMode {
    //         cursor,
    //         motion_mode,
    //         range,
    //         is_vertical,
    //         register,
    //     })
    // }

    // pub fn do_edit_buffer(
    //     &mut self,
    //     cursor: &mut Cursor,
    //     cmd: &EditCommand,
    //     modal: bool,
    //     register: &mut Register,
    //     smart_tab: bool,
    // ) -> Vec<(Rope, RopeDelta, InvalLines)> {
    //     self.buffer_edit(EditBuffer::DoEditBuffer {
    //         cursor,
    //         cmd,
    //         modal,
    //         register,
    //         smart_tab,
    //     })
    // }

    // pub fn do_insert_buffer(
    //     &mut self,
    //     cursor: &mut Cursor,
    //     s: &str,
    // ) -> Vec<(Rope, RopeDelta, InvalLines)> {
    //     self.buffer_edit(EditBuffer::DoInsertBuffer { cursor, s })
    // }

    pub fn clear_completion_lens(&mut self) {
        self.completion_lens = None;
        self.on_update_lines();
        self.signals.update_paint_text();
    }

    pub fn init_diagnostics(&mut self) -> Result<()> {
        self.init_diagnostics_with_buffer()?;
        self.on_update_lines();
        self.signals.update_paint_text();
        self.signals.trigger();
        Ok(())
    }

    // pub fn update_viewport_size(&mut self, viewport: Rect) -> Result<()> {
    //     let viewport_size = viewport.size();
    //
    //     let should_update =
    //         matches!(self.editor_style.wrap_method(), WrapMethod::EditorWidth)
    //             && self.viewport_size.width != viewport_size.width;
    //     if should_update {
    //         self.viewport_size = viewport_size;
    //     }
    //     if self.signals.viewport.update_if_not_equal(viewport) {
    //         self.signals.update_paint_text();
    //
    //     }
    //     self.trigger_signals();
    //     Ok(())
    // }
    //
    // pub fn update_viewport_by_scroll(&mut self, viewport: Rect) {
    //     debug!(
    //         "viewport={viewport:?} self.signals.viewport={:?} {:?}",
    //         self.signals.viewport.val(),
    //         self.editor_style.wrap_method()
    //     );
    //     if self.signals.viewport.val().y0 == viewport.y0
    //         && self.signals.viewport.val().y1 == viewport.y1
    //         && !matches!(self.editor_style.wrap_method(),
    // WrapMethod::EditorWidth)     {
    //         return;
    //     }
    //     if self.signals.viewport.update_if_not_equal(viewport) {
    //         self.signals.update_paint_text();
    //
    //         self.trigger_signals();
    //     }
    // }

    pub fn update_config(&mut self, config: EditorConfig) -> Result<()> {
        // todo
        // if self.config != config {
        self.config = config;
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        // }
        Ok(())
    }

    pub fn update_folding_ranges(&mut self, action: UpdateFolding) -> Result<()> {
        // log::info!("{}", serde_json::to_string(&action).unwrap());
        match action {
            UpdateFolding::UpdateByItem(item) => {
                self.folding_ranges.update_folding_item(item);
            },
            UpdateFolding::New(ranges) => {
                self.folding_ranges.update_ranges(ranges);
            },
            UpdateFolding::UpdateByPhantom(position) => {
                self.folding_ranges.update_by_phantom(position);
            },
            UpdateFolding::FoldCode(offset) => {
                let rope = self.signals.buffer.val().text();
                self.folding_ranges.fold_by_offset(offset, rope)?;
            },
        }
        // todo improve OriginLinesDelta
        // self.check_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn update_inline_completion(&mut self, delta: &RopeDelta) -> Result<()> {
        let Some((completion, ..)) = self.inline_completion.take() else {
            return Ok(());
        };
        let (line, col) = self.completion_pos;
        let offset = self.buffer().offset_of_line_col(line, col)?;

        // Shift the position by the rope delta
        let mut transformer = Transformer::new(delta);

        let new_offset = transformer.transform(offset, true);
        let new_pos = self.buffer().offset_to_line_col(new_offset)?;

        if delta.as_simple_insert().is_some() {
            let (iv, new_len) = delta.summary();
            if iv.start() == iv.end()
                && iv.start() == offset
                && new_len <= completion.len()
            {
                // Remove the # of newly inserted characters
                // These aren't necessarily the same as the characters
                // literally in the text, but the
                // completion will be updated when the completion
                // widget receives the update event,
                // and it will fix this if needed.
                self.inline_completion =
                    Some((completion[new_len..].to_string(), new_pos.0, new_pos.1));
            }
        } else {
            self.inline_completion = Some((completion, new_pos.0, new_pos.1));
        }
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn apply_delta(&mut self, delta: &RopeDelta) {
        if self.style_from_lsp {
            if let Some(styles) = &mut self.semantic_styles {
                styles.1.apply_shape(delta);
            }
        } else if let Some(styles) = self.syntax.styles.as_mut() {
            styles.apply_shape(delta);
        }
        self.syntax.lens.apply_delta(delta);
        self.update_diagnostics(delta);
        self.update_inlay_hints(delta);
        if let Err(err) = self.update_completion_lens(delta) {
            error!("{err}");
        }
        // self.update_lines();
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
    }

    pub fn trigger_syntax_change(
        &mut self,
        _edits: Option<SmallVec<[SyntaxEdit; 3]>>,
    ) -> Result<()> {
        self.syntax.cancel_flag.store(1, atomic::Ordering::Relaxed);
        self.syntax.cancel_flag = Arc::new(AtomicUsize::new(0));
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn set_inline_completion(
        &mut self,
        inline_completion: String,
        line: usize,
        col: usize,
    ) -> Result<()> {
        self.inline_completion = Some((inline_completion, line, col));
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn clear_inline_completion(&mut self) -> Result<()> {
        self.inline_completion = None;
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn set_syntax_with_rev(&mut self, syntax: Syntax, rev: u64) -> Result<bool> {
        if self.buffer().rev() != rev {
            return Ok(false);
        }
        self.set_syntax(syntax)
    }

    pub fn set_syntax(&mut self, syntax: Syntax) -> Result<bool> {
        self.syntax = syntax;
        if self.style_from_lsp {
            return Ok(false);
        }
        self.update_parser()?;

        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(true)
    }

    pub fn set_inlay_hints(&mut self, inlay_hint: Spans<InlayHint>) -> Result<()> {
        self.inlay_hints = Some(inlay_hint);
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn set_document_highlight(
        &mut self,
        document_highlight: Option<Vec<DocumentHighlight>>,
    ) {
        if self.document_highlight != document_highlight {
            self.document_highlight = document_highlight;
            // self.update_lines_new(OriginLinesDelta::default())?;
            // self.on_update_lines();
            self.signals.update_paint_text();
            self.trigger_signals();
        }
    }

    pub fn set_completion_lens(
        &mut self,
        completion_lens: String,
        line: usize,
        col: usize,
    ) -> Result<()> {
        self.completion_lens = Some(completion_lens);
        self.completion_pos = (line, col);
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(())
    }

    pub fn update_semantic_styles_from_lsp(
        &mut self,
        styles: (Option<String>, Spans<String>),
        rev: u64,
    ) -> Result<bool> {
        if self.buffer().rev() != rev {
            return Ok(false);
        }
        self.style_from_lsp = true;
        self.semantic_styles = Some(styles);
        self.on_update_lines();
        self.signals.update_paint_text();

        self.trigger_signals();
        Ok(true)
    }

    pub fn last_line_width(&self) -> f64 {
        self.signals.last_line.val().1
    }
}

type LinesEditorStyle = DocLines;

impl LinesEditorStyle {
    pub fn modal(&self) -> bool {
        self.editor_style.modal()
    }

    pub fn current_line_color(&self) -> Option<Color> {
        EditorStyle::current_line(&self.editor_style)
    }

    pub fn scroll_beyond_last_line(&self) -> bool {
        EditorStyle::scroll_beyond_last_line(&self.editor_style)
    }

    pub fn ed_caret(&self) -> Brush {
        self.editor_style.ed_caret()
    }

    pub fn document_highlight(&self) -> Color {
        self.editor_style.document_highlight()
    }

    pub fn selection_color(&self) -> Color {
        self.editor_style.selection()
    }

    pub fn indent_style(&self) -> IndentStyle {
        self.editor_style.indent_style()
    }

    pub fn indent_guide(&self) -> Color {
        self.editor_style.indent_guide()
    }

    pub fn visible_whitespace(&self) -> Color {
        self.editor_style.visible_whitespace()
    }

    pub fn update_editor_style(&mut self, cx: &mut StyleCx<'_>) -> Result<bool> {
        // todo
        let updated = self.editor_style.read(cx);
        let new_show_indent_guide = self.show_indent_guide();
        self.signals
            .show_indent_guide
            .update_if_not_equal(new_show_indent_guide);
        self.trigger_signals();
        Ok(updated)
    }

    pub fn show_indent_guide(&self) -> (bool, Color) {
        (
            self.editor_style.show_indent_guide(),
            self.editor_style.indent_guide(),
        )
    }
}

/// 以界面为单位，进行触发。
type LinesSignals = DocLines;

/// 以界面为单位，进行触发。
impl LinesSignals {
    pub fn signal_show_indent_guide(&self) -> ReadSignal<(bool, Color)> {
        self.signals.show_indent_guide.signal()
    }

    pub fn signal_buffer_rev(&self) -> ReadSignal<u64> {
        self.signals.signal_buffer_rev()
    }

    pub fn signal_buffer(&self) -> ReadSignal<Buffer> {
        self.signals.buffer.signal()
    }

    pub fn signal_last_line(&self) -> ReadSignal<(usize, f64)> {
        self.signals.last_line.signal()
    }

    pub fn signal_pristine(&self) -> ReadSignal<bool> {
        self.signals.pristine.signal()
    }

    pub fn signal_paint_content(&self) -> ReadSignal<usize> {
        self.signals.paint_content.signal()
    }

    pub fn signal_max_width(&self) -> ReadSignal<f64> {
        self.signals.max_width.signal()
    }
}

pub trait RopeTextPosition: RopeText {
    /// Converts a UTF8 offset to a UTF16 LSP position
    /// Returns None if it is not a valid UTF16 offset
    fn offset_to_position(&self, offset: usize) -> Result<Position> {
        let (line, col) = self.offset_to_line_col(offset)?;
        let line_offset = self.offset_of_line(line)?;

        let utf16_col =
            offset_utf8_to_utf16(self.char_indices_iter(line_offset..), col);

        Ok(Position {
            line:      line as u32,
            character: utf16_col as u32,
        })
    }

    fn offset_of_position(&self, pos: &Position) -> Result<usize> {
        let (line, column) = self.position_to_line_col(pos)?;

        self.offset_of_line_col(line, column)
    }

    fn position_to_line_col(&self, pos: &Position) -> Result<(usize, usize)> {
        let line = pos.line as usize;
        let line_offset = self.offset_of_line(line)?;

        let column = offset_utf16_to_utf8(
            self.char_indices_iter(line_offset..),
            pos.character as usize,
        );

        Ok((line, column))
    }
}

impl<T: RopeText> RopeTextPosition for T {}

#[derive(Debug, Eq, PartialEq)]
pub enum ClickResult {
    NoHintOrNothing,
    MatchWithoutLocation,
    MatchFolded,
    MatchHint(Location),
}

#[derive(Debug)]
/// 文档偏移位置的相关信息
pub struct InfoOfBufferOffset {
    /// 所在的原始行
    pub origin_line:                  usize,
    /// 在原始行的位置
    pub offset_of_origin_line:        usize,
    /// 所在的原始折叠行
    pub origin_folded_line_index:     usize,
    /// 在原始折叠行的位置。被折叠则为none
    pub offset_of_origin_folded_line: Option<usize>,
    /// 在整个文档的空间位置
    pub point_of_document:            Point,
}

use std::ops::Range;

use floem::{
    peniko::Color,
    text::{Attrs, AttrsList}
};
use lapce_xi_rope::Interval;
use log::{info, warn};
use lsp_types::Position;
use smallvec::SmallVec;

use crate::lines::{cursor::CursorAffinity, delta_compute::Offset};

/// `PhantomText` is for text that is not in the actual document, but
/// should be rendered with it.
///
/// Ex: Inlay hints, IME text, error lens' diagnostics, etc
#[derive(Debug, Clone, Default)]
pub struct PhantomText {
    /// The kind is currently used for sorting the phantom text on a
    /// line
    pub kind:       PhantomTextKind,
    /// Column on the line that the phantom text should be displayed
    /// at
    ///
    /// 在原始文本的行
    pub line:       usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 在原始行line文本的位置
    pub col:        usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 合并后，在多行原始行文本（不考虑折叠、幽灵文本）的位置。
    /// 与col相差前面折叠行的总长度
    pub merge_col:  usize,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本（考虑折叠、幽灵文本）的位置
    pub final_col:  usize,
    /// the affinity of cursor, e.g. for completion phantom text,
    /// we want the cursor always before the phantom text
    pub affinity:   Option<CursorAffinity>,
    pub text:       String,
    pub font_size:  Option<usize>,
    // font_family: Option<FontFamily>,
    pub fg:         Option<Color>,
    pub bg:         Option<Color>,
    pub under_line: Option<Color>
}

impl PhantomText {
    pub fn next_line(&self) -> Option<usize> {
        if let PhantomTextKind::LineFoldedRang { next_line, .. } = self.kind {
            next_line
        } else {
            None
        }
    }

    /// [start..end]
    pub fn final_col_range(&self) -> Option<(usize, usize)> {
        if self.text.is_empty() {
            None
        } else {
            Some((self.final_col, self.final_col + self.text.len() - 1))
        }
    }

    pub fn next_final_col(&self) -> usize {
        if let Some((_, end)) = self.final_col_range() {
            end + 1
        } else {
            self.final_col
        }
    }

    pub fn next_origin_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang { len, .. } = self.kind {
            self.col + len
        } else {
            self.col
        }
    }

    pub fn next_merge_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang { len, .. } = self.kind {
            self.merge_col + len
        } else {
            self.merge_col
        }
    }

    pub fn log(&self) {
        info!(
            "{:?} line={} col={} final_col={} text={} text.len()={}",
            self.kind,
            self.line,
            self.merge_col,
            self.final_col,
            self.text,
            self.text.len()
        );
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginText {
    /// 在原始文本的行
    pub line:      usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 在原始行文本的位置
    pub col:       Interval,
    ///
    /// 合并后原始行文本的位置
    pub merge_col: Interval,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本的位置
    pub final_col: Interval
}

impl OriginText {
    /// 视觉偏移的原始偏移
    pub fn origin_col_of_final_col(&self, final_col: usize) -> usize {
        final_col - self.final_col.start + self.col.start
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EmptyText {
    /// 在原始文本的行
    pub line:           usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 在原始行文本的位置
    pub offset_of_line: usize
}
#[derive(Debug, Clone)]
pub enum Text {
    Phantom { text: PhantomText },
    OriginText { text: OriginText },
    EmptyLine { text: EmptyText }
}

impl Text {
    pub fn adjust(&mut self, line_delta: Offset, _offset_delta: Offset) {
        match self {
            Text::Phantom { text } => {
                text.kind.adjust(line_delta);
                line_delta.adjust(&mut text.line);
            },
            Text::OriginText { text } => {
                line_delta.adjust(&mut text.line);
            },
            _ => {}
        }
    }

    fn merge_to(mut self, origin_text_len: usize, final_text_len: usize) -> Self {
        match &mut self {
            Text::Phantom { text } => {
                text.merge_col += origin_text_len;
                text.final_col += final_text_len;
            },
            Text::OriginText { text } => {
                text.merge_col = text.merge_col.translate(origin_text_len);
                text.final_col = text.final_col.translate(final_text_len);
            },
            _ => {}
        }
        self
    }
}

impl From<PhantomText> for Text {
    fn from(text: PhantomText) -> Self {
        Self::Phantom { text }
    }
}
impl From<OriginText> for Text {
    fn from(text: OriginText) -> Self {
        Self::OriginText { text }
    }
}

impl From<EmptyText> for Text {
    fn from(text: EmptyText) -> Self {
        Self::EmptyLine { text }
    }
}

#[derive(Debug, Clone, Copy, Ord, Eq, PartialEq, PartialOrd, Default)]
pub enum PhantomTextKind {
    #[default]
    /// Input methods
    Ime,
    Placeholder,
    /// Completion lens / Inline completion
    Completion,
    /// Inlay hints supplied by an LSP/PSP (like type annotations)
    InlayHint,
    /// Error lens
    Diagnostic,
    // 行内折叠。跨行折叠也都转换成行内折叠
    LineFoldedRang {
        next_line:      Option<usize>,
        // 被折叠的长度
        len:            usize,
        start_position: Position
    }
}

impl PhantomTextKind {
    pub fn adjust(&mut self, line_delta: Offset) {
        if let Self::LineFoldedRang {
            next_line,
            start_position,
            ..
        } = self
        {
            let mut position_line = start_position.line as usize;
            line_delta.adjust(&mut position_line);
            start_position.line = position_line as u32;
            if let Some(x) = next_line.as_mut() {
                line_delta.adjust(x)
            }
        }
    }
}

/// Information about the phantom text on a specific line.
///
/// This has various utility functions for transforming a coordinate
/// (typically a column) into the resulting coordinate after the
/// phantom text is combined with the line's real content.
#[derive(Debug, Default, Clone)]
pub struct PhantomTextLine {
    pub line:           usize,
    // 该行起点在文本中的偏移
    pub offset_of_line: usize,
    // 原文本的长度，包括换行符等，原始单行
    origin_text_len:    usize,
    // 最后展现的长度，包括幽灵文本、换行符.
    final_text_len:     usize,
    /// This uses a smallvec because most lines rarely have more than
    /// a couple phantom texts
    texts:              SmallVec<[Text; 6]>
}

impl PhantomTextLine {
    pub fn new(
        line: usize,
        origin_text_len: usize,
        offset_of_line: usize,
        mut phantom_texts: SmallVec<[PhantomText; 6]>
    ) -> Self {
        phantom_texts.sort_by(|a, b| {
            if a.merge_col == b.merge_col {
                a.kind.cmp(&b.kind)
            } else {
                a.merge_col.cmp(&b.merge_col)
            }
        });

        let mut final_last_end = 0;
        let mut origin_last_end = 0;
        let mut merge_last_end = 0;
        let mut texts = SmallVec::new();

        let mut offset = 0i32;
        for mut phantom in phantom_texts {
            match phantom.kind {
                PhantomTextKind::LineFoldedRang { len, .. } => {
                    phantom.final_col = usize_offset(phantom.merge_col, offset);
                    offset = offset + phantom.text.len() as i32 - len as i32;
                },
                _ => {
                    phantom.final_col = usize_offset(phantom.merge_col, offset);
                    offset += phantom.text.len() as i32;
                }
            }
            if final_last_end < phantom.final_col {
                let len = phantom.final_col - final_last_end;
                // insert origin text
                texts.push(
                    OriginText {
                        line:      phantom.line,
                        col:       Interval::new(
                            origin_last_end,
                            origin_last_end + len
                        ),
                        merge_col: Interval::new(
                            merge_last_end,
                            merge_last_end + len
                        ),
                        final_col: Interval::new(
                            final_last_end,
                            final_last_end + len
                        )
                    }
                    .into()
                );
            }
            final_last_end = phantom.next_final_col();
            origin_last_end = phantom.next_origin_col();
            merge_last_end = phantom.next_merge_col();
            texts.push(phantom.into());
        }

        let len = origin_text_len - origin_last_end;
        if len > 0 {
            texts.push(
                OriginText {
                    line,
                    col: Interval::new(origin_last_end, origin_last_end + len),
                    merge_col: Interval::new(merge_last_end, merge_last_end + len),
                    final_col: Interval::new(final_last_end, final_last_end + len)
                }
                .into()
            );
        } else if origin_text_len == 0 {
            texts.push(
                EmptyText {
                    line,
                    offset_of_line
                }
                .into()
            );
        }

        let final_text_len = usize_offset(origin_text_len, offset);
        Self {
            final_text_len,
            line,
            origin_text_len,
            texts,
            offset_of_line
        }
    }

    pub fn folded_line(&self) -> Option<usize> {
        if let Some(Text::Phantom { text }) = self.texts.iter().last() {
            if let PhantomTextKind::LineFoldedRang { next_line, .. } = text.kind {
                return next_line;
            }
        }
        None
    }
}

/// 1. 视觉行的内容都可以由text拼接
///
/// 2. 末尾的空行，由EmptyText表示
///
/// 3. 所有的原始字符都归属某个Text
#[derive(Debug, Default, Clone)]
pub struct PhantomTextMultiLine {
    /// 原始文本的行号
    pub line:            usize,
    pub last_line:       usize,
    // line行起点在文本中的偏移
    pub offset_of_line:  usize,
    // 所有合并在该行的原始行的总长度
    pub origin_text_len: usize,
    // 所有合并在该行的最后展现的长度，包括幽灵文本、换行符、
    // 包括后续的折叠行
    pub final_text_len:  usize,
    // // 各个原始行的行号、原始长度、最后展现的长度
    // pub len_of_line:     Vec<(usize, usize, usize)>,
    /// This uses a smallvec because most lines rarely have more than
    /// a couple phantom texts
    pub text:            SmallVec<[Text; 6]> /* 可以去掉，仅做记录
                                              * pub lines:
                                              * Vec<PhantomTextLine>,
                                              */
}

impl PhantomTextMultiLine {
    pub fn new(line: PhantomTextLine) -> Self {
        // let len_of_line =
        //     vec![(line.line, line.origin_text_len, line.final_text_len)];
        Self {
            line:            line.line,
            last_line:       line.line,
            offset_of_line:  line.offset_of_line,
            origin_text_len: line.origin_text_len,
            final_text_len:  line.final_text_len,
            // len_of_line,
            text:            line.texts
        }
    }

    pub fn merge(&mut self, line: PhantomTextLine) {
        // let index = self.len_of_line.len();
        // let last_len = self.len_of_line[index - 1];
        // for _ in index..line.line - self.line {
        //     self.len_of_line.push(last_len);
        // }
        // self.len_of_line.push((
        //     line.line,
        //     line.origin_text_len,
        //     line.final_text_len
        // ));

        let origin_text_len = self.origin_text_len;
        self.origin_text_len += line.origin_text_len;
        let final_text_len = self.final_text_len;
        self.final_text_len += line.final_text_len;
        for phantom in line.texts.clone() {
            self.text
                .push(phantom.merge_to(origin_text_len, final_text_len));
        }
        self.last_line = line.line;
        // self.lines.push(line);
    }

    pub fn final_text_len(&self) -> usize {
        self.final_text_len
    }

    pub fn update_final_text_len(&mut self, _len: usize) {
        self.final_text_len = _len;
    }

    pub fn iter_phantom_text(&self) -> impl Iterator<Item = &PhantomText> {
        self.text.iter().filter_map(|x| {
            if let Text::Phantom { text } = x {
                Some(text)
            } else {
                None
            }
        })
    }

    pub fn add_phantom_style(
        &self,
        attrs_list: &mut AttrsList,
        attrs: Attrs,
        phantom_color: Color
    ) {
        self.text.iter().for_each(|x| match x {
            Text::Phantom { text } => {
                if !text.text.is_empty() {
                    let mut attrs = attrs;
                    if let Some(fg) = text.fg {
                        attrs = attrs.color(fg);
                    } else {
                        attrs = attrs.color(phantom_color)
                    }
                    if let Some(phantom_font_size) = text.font_size {
                        attrs = attrs.font_size(
                            (phantom_font_size as f32).min(attrs.font_size)
                        );
                    }
                    attrs_list.add_span(
                        text.final_col..(text.final_col + text.text.len()),
                        attrs
                    );
                }
            },
            Text::OriginText { .. } | Text::EmptyLine { .. } => {}
        });
    }

    // /// 被折叠的范围。用于计算因折叠导致的原始文本的样式变化
    // ///
    // pub fn floded_ranges(&self) -> Vec<Range<usize>> {
    //     let mut ranges = Vec::new();
    //     for item in &self.text {
    //         if let Text::Phantom {
    //             text
    //         }
    //         if let PhantomTextKind::LineFoldedRang { .. } =
    // item.kind {
    // ranges.push(item.final_col..self.final_text_len);         }
    //     }
    //     ranges
    // }

    // /// 最终文本的文本信息
    // fn text_of_final_col(&self, final_col: usize) -> &Text {
    //     self.text_of_final_offset(final_col.min(self.final_text_len
    // - 1)).unwrap() }

    // /// 最终行偏移的文本信息。在文本外的偏移返回none
    // fn text_of_visual_char(&self, final_offset: usize) ->
    // Option<&Text> {     self.text.iter().find(|x| {
    //         match x {
    //             Text::Phantom { text } => {
    //                 if text.final_col <= final_offset &&
    // final_offset < text.next_final_col() {
    // return true;                 }
    //             }
    //             Text::OriginText { text } => {
    //                 if text.final_col.contains(final_offset) {
    //                     return true;
    //                 }
    //             }
    //             Text::EmptyLine{..} => {}
    //         }
    //         false
    //     })
    // }

    /// 视觉字符的Text
    fn text_of_visual_char(&self, visual_char_offset: usize) -> &Text {
        debug_assert!(
            visual_char_offset == 0 || (visual_char_offset < self.final_text_len)
        );
        self.text
            .iter()
            .find(|x| {
                match x {
                    Text::Phantom { text } => {
                        if text.final_col <= visual_char_offset
                            && visual_char_offset < text.next_final_col()
                        {
                            return true;
                        }
                    },
                    Text::OriginText { text } => {
                        if text.final_col.contains(visual_char_offset) {
                            return true;
                        }
                    },
                    Text::EmptyLine { .. } => return true
                }
                false
            })
            .unwrap()
    }

    fn text_of_origin_line_col(
        &self,
        origin_line: usize,
        origin_col: usize
    ) -> Option<&Text> {
        self.text.iter().find(|x| {
            match x {
                Text::Phantom { text } => {
                    if text.line == origin_line
                        && text.col <= origin_col
                        && origin_col < text.next_origin_col()
                    {
                        return true;
                    } else if let Some(next_line) = text.next_line() {
                        if origin_line < next_line {
                            return true;
                        }
                    }
                },
                Text::OriginText { text } => {
                    if text.line == origin_line && text.col.contains(origin_col) {
                        return true;
                    }
                },
                Text::EmptyLine { .. } => return true
            }
            false
        })
    }

    fn text_of_merge_col(&self, merge_col: usize) -> Option<&Text> {
        self.text.iter().find(|x| {
            match x {
                Text::Phantom { text } => {
                    if text.merge_col <= merge_col
                        && merge_col <= text.next_merge_col()
                    {
                        return true;
                    }
                },
                Text::OriginText { text } => {
                    if text.merge_col.contains(merge_col) {
                        return true;
                    }
                },
                Text::EmptyLine { .. } => {
                    return true;
                }
            }
            false
        })
    }

    /// 最终文本的原始文本位移。若为幽灵则返回none.超过最终文本长度，
    /// 则返回none(不应该在此情况下调用该方法)
    pub fn origin_col_of_final_offset(
        &self,
        final_col: usize
    ) -> Option<(usize, usize)> {
        // let final_col = final_col.min(self.final_text_len - 1);
        if let Text::OriginText { text } = self.text_of_visual_char(final_col) {
            let origin_col = text.col.start + final_col - text.final_col.start;
            return Some((text.line, origin_col));
        }
        None
    }

    /// Translate a column position into the text into what it would
    /// be after combining 求原始文本在最终文本的位置。场景：
    /// 计算原始文本的样式在最终文本的位置。
    ///
    /// 最终文本的位置 = 原始文本位置 + 之前的幽灵文本长度
    pub fn col_at(&self, merge_col: usize) -> Option<usize> {
        let text = self.text_of_merge_col(merge_col)?;
        match text {
            Text::Phantom { .. } => None,
            Text::OriginText { text } => {
                Some(text.final_col.start + merge_col - text.merge_col.start)
            },
            Text::EmptyLine { .. } => None
        }
    }

    /// 原始行的偏移字符！！！，的对应的合并后的位置。
    /// 用于求鼠标的实际位置
    ///
    /// Translate a column position into the text into what it would
    /// be after combining
    ///
    /// 暂时不考虑_before_cursor，等足够熟悉了再说
    pub fn final_col_of_col(
        &self,
        line: usize,
        pre_col: usize,
        _before_cursor: bool
    ) -> usize {
        let adjust_offset = if !_before_cursor { 1 } else { 0 };
        if self.text.is_empty() {
            return pre_col;
        }
        let text = self.text_of_origin_line_col(line, pre_col);
        if let Some(text) = text {
            match text {
                Text::Phantom { text } => {
                    if text.col == 0 {
                        // 后一个字符
                        text.next_final_col()
                    } else {
                        // 前一个字符
                        text.final_col
                    }
                },
                Text::OriginText { text } => {
                    text.final_col.start + pre_col - text.col.start + adjust_offset
                },
                Text::EmptyLine { .. } => 0
            }
        } else {
            self.final_text_len
        }
    }

    /// return (origin line, origin line offset, offset_of_line)
    pub fn cursor_position_of_final_col(
        &self,
        mut visual_char_offset: usize
    ) -> (usize, usize, usize) {
        // 因为通过hit_point获取的index会大于等于final_text_len
        if visual_char_offset >= self.final_text_len {
            visual_char_offset = self.final_text_len.max(1) - 1;
        }
        match self.text_of_final_col(visual_char_offset) {
            Text::Phantom { text } => {
                (text.line, text.next_origin_col(), self.offset_of_line)
            },
            Text::OriginText { text } => (
                text.line,
                text.origin_col_of_final_col(visual_char_offset),
                self.offset_of_line
            ),
            Text::EmptyLine { text } => (text.line, 0, text.offset_of_line)
        }
    }

    /// return (origin line, origin line offset, offset_of_line)
    pub fn text_of_final_col(&self, mut visual_char_offset: usize) -> &Text {
        // 因为通过hit_point获取的index会大于等于final_text_len
        if visual_char_offset >= self.final_text_len {
            visual_char_offset = self.final_text_len.max(1) - 1;
        }
        self.text_of_visual_char(visual_char_offset)
    }

    // /// Translate a column position into the position it would be
    // before combining ///
    // /// 将列位置转换为合并前的位置，也就是原始文本的位置？
    // 意义在于计算光标的位置（光标是用原始文本的offset来记录位置的）
    // ///
    // /// return  (line, index of line, index of buffer)
    // ///         (origin_line, _offset_of_line, offset_buffer)
    // pub fn cursor_position_of_final_col(&self, col: usize) ->
    // (usize, usize, usize) {     let text =
    // self.text_of_visual_char(col);     // if let Some(text) =
    // text {         match text {
    //             Text::Phantom { text } => {
    //                 return (text.line, text.col,
    // self.offset_of_line + text.merge_col)             }
    //             Text::OriginText { text } => {
    //                 return (
    //                     text.line,
    //                     text.col.start + col -
    // text.final_col.start,
    // self.offset_of_line + text.merge_col.start + col -
    // text.final_col.start,                 );
    //             }
    //             Text::EmptyLine{..} => {
    //                 panic!()
    //             }
    //         }
    //     // }
    //     let (line, offset, _) = self.len_of_line.last().unwrap();
    //     (
    //         *line,
    //         (*offset).max(1) - 1,
    //         (self.offset_of_line + self.origin_text_len).max(1) -
    // 1,     )
    //     // let (line, offset, _) =
    // self.len_of_line.last().unwrap();     // (*line, *offset-1)
    // }

    /// Translate a column position into the position it would be
    /// before combining
    ///
    /// 获取偏移位置的幽灵文本以及在该幽灵文本的偏移值
    pub fn phantom_text_of_final_col(
        &self,
        col: usize
    ) -> Option<(PhantomText, usize)> {
        let text = self.text_of_visual_char(col);
        if let Text::Phantom { text } = text {
            Some((text.clone(), text.final_col - col))
        } else {
            None
        }
        // if self.text.is_empty() {
        //     return None;
        // };
        // let text_iter = self.text.iter();
        // for text in text_iter {
        //     let Some((phantom_final_start, phantom_final_end)) =
        // text.final_col_range() else {         continue;
        //     };
        //     //  [origin_start                     [text.col
        //     //  [final_start       ..col..
        // [phantom_final_start   ..col..  phantom_final_end]  ..col..
        //     if phantom_final_start <= col && col <=
        // phantom_final_end {         return
        // Some((text.clone(), col - phantom_final_start))
        //     }
        // }
        // None
    }

    // /// Iterator over (col_shift, size, hint, pre_column)
    // /// Note that this only iterates over the ordered text, since
    // those depend on the text for where /// they'll be
    // positioned /// (finally col index, phantom len, phantom at
    // origin text index, phantom) ///
    // /// (最终文本上该幽灵文本前其他幽灵文本的总长度，
    // 幽灵文本的长度，(幽灵文本在原始文本的字符位置),
    // (幽灵文本在最终文本的字符位置)，幽灵文本) ///
    // /// 所以原始文本在最终文本的位置= 原始位置 +
    // 之前的幽灵文本总长度 ///
    // pub fn offset_size_iter(&self) -> impl Iterator<Item = (usize,
    // usize, (usize, usize), &PhantomText)> + '_ {     let mut
    // col_shift = 10usize;     let line = self.line;
    //     self.text.iter().map(move |phantom| {
    //         let rs = match phantom.kind {
    //             PhantomTextKind::LineFoldedRang {
    //                 ..
    //             } => {
    //                 let pre_col_shift = col_shift;
    //                 let phantom_line = line;
    //                     col_shift += phantom.text.len();
    //                 (
    //                     pre_col_shift,
    //                     phantom.text.len(),
    //                     (phantom_line, phantom.merge_col),
    //                     phantom,
    //                 )
    //             }
    //             _ => {
    //                 let pre_col_shift = col_shift;
    //                 col_shift += phantom.text.len();
    //                 (
    //                     pre_col_shift,
    //                     phantom.text.len(),
    //                     (line, phantom.merge_col),
    //                     phantom,
    //                 )
    //             }
    //         };
    //         tracing::debug!(
    //             "line={} offset={} len={} col={:?} text={} {:?}",
    //             self.line,
    //             rs.0,
    //             rs.1,
    //             rs.2,
    //             rs.3.text,
    //             rs.3.kind
    //         );
    //         rs
    //     })
    // }

    pub fn final_line_content(&self, origin: &str) -> String {
        combine_with_text(&self.text, origin)
    }

    pub fn adjust(&mut self, line_delta: Offset, offset_delta: Offset) {
        line_delta.adjust(&mut self.line);
        line_delta.adjust(&mut self.last_line);

        offset_delta.adjust(&mut self.offset_of_line);
        // for (line, _, _) in &mut self.len_of_line {
        //     line_delta(line);
        // }
        for text in &mut self.text {
            text.adjust(line_delta, offset_delta);
        }
    }
}

fn usize_offset(val: usize, offset: i32) -> usize {
    let rs = val as i32 + offset;
    assert!(rs >= 0);
    rs as usize
}

/// Not allowed to cross the range??
pub struct Ranges {
    ranges: Vec<Range<usize>>
}

impl Ranges {
    pub fn except(&self, mut rang: Range<usize>) -> Vec<Range<usize>> {
        let mut final_ranges = Vec::new();
        for exc in &self.ranges {
            if exc.end <= rang.start {
                // no change
            } else if exc.start <= rang.start
                && rang.start < exc.end
                && exc.end < rang.end
            {
                rang.start = exc.end
            } else if rang.start < exc.start && exc.end <= rang.end {
                final_ranges.push(rang.start..exc.start);
                rang.start = exc.end;
            } else if rang.start < exc.start
                && rang.end >= exc.start
                && rang.end < exc.end
            {
                rang.end = exc.start;
                break;
            } else if rang.end <= exc.start {
                break;
            } else if exc.start <= rang.start && rang.end <= exc.end {
                return Vec::with_capacity(0);
            } else {
                warn!("{exc:?} {rang:?}");
            }
        }
        if rang.start < rang.end {
            final_ranges.push(rang);
        }
        final_ranges
    }
}

pub fn combine_with_text(lines: &SmallVec<[Text; 6]>, origin: &str) -> String {
    let mut rs = String::new();
    // let mut latest_col = 0;
    for text in lines {
        match text {
            Text::Phantom { text } => {
                rs.push_str(text.text.as_str());
            },
            Text::OriginText { text } => {
                rs.push_str(sub_str(
                    origin,
                    text.merge_col.start.min(origin.len()),
                    text.merge_col.end.min(origin.len())
                ));
            },
            Text::EmptyLine { .. } => {
                break;
            }
        }
    }
    rs
}

fn sub_str(text: &str, begin: usize, end: usize) -> &str {
    unsafe { text.get_unchecked(begin..end) }
}

#[cfg(test)]
mod test {
    #![allow(unused_variables, dead_code)]

    use std::default::Default;

    use log::info;
    use smallvec::SmallVec;

    use super::{
        PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine, Text,
        combine_with_text
    };

    // "0123456789012345678901234567890123456789
    // "    if true {nr    } else {nr    }nr"
    // "    if true {...} else {...}nr"
    fn init_folded_line(visual_line: usize, folded: bool) -> PhantomTextLine {
        let mut text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        let origin_text_len;
        match (visual_line, folded) {
            (2, _) => {
                origin_text_len = 15;
                text.push(PhantomText {
                    kind: PhantomTextKind::LineFoldedRang {
                        len:            3,
                        next_line:      Some(3),
                        start_position: Default::default()
                    },
                    line: 1,
                    final_col: 12,
                    merge_col: 12,
                    col: 12,
                    text: "{...}".to_string(),
                    ..Default::default()
                });
            },
            (4, false) => {
                origin_text_len = 14;
                text.push(PhantomText {
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line:      None,
                        len:            5,
                        start_position: Default::default()
                    },
                    line: 3,
                    final_col: 0,
                    col: 0,
                    merge_col: 0,
                    text: "".to_string(),
                    ..Default::default()
                });
            },
            (4, true) => {
                // "0123456789012345678901234567890123456789
                // "    } else {nr    }nr"
                origin_text_len = 14;
                text.push(PhantomText {
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line:      None,
                        len:            5,
                        start_position: Default::default()
                    },
                    line: 3,
                    final_col: 0,
                    col: 0,
                    merge_col: 0,
                    text: "".to_string(),
                    ..Default::default()
                });
                text.push(PhantomText {
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line:      Some(5),
                        len:            3,
                        start_position: Default::default()
                    },
                    line: 3,
                    final_col: 11,
                    col: 11,
                    merge_col: 11,
                    text: "{...}".to_string(),
                    ..Default::default()
                });
            },
            (6, _) => {
                origin_text_len = 7;
                text.push(PhantomText {
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line:      None,
                        len:            5,
                        start_position: Default::default()
                    },
                    line: 5,
                    final_col: 0,
                    col: 0,
                    merge_col: 0,
                    text: "".to_string(),
                    ..Default::default()
                });
            },
            _ => {
                panic!("");
            }
        }
        PhantomTextLine::new(visual_line - 1, origin_text_len, 0, text)
    }
    // "0         10        20        30
    // "0123456789012345678901234567890123456789
    // "    let a = A;nr
    fn let_data() -> PhantomTextLine {
        let mut text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        let origin_text_len = 16;
        text.push(PhantomText {
            kind: PhantomTextKind::InlayHint,
            merge_col: 9,
            line: 6,
            col: 9,
            text: ": A ".to_string(),
            ..Default::default()
        });
        PhantomTextLine::new(6, origin_text_len, 0, text)
    }

    fn empty_data() -> PhantomTextLine {
        let text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        let origin_text_len = 0;
        PhantomTextLine::new(6, origin_text_len, 0, text)
    }
    #[test]
    fn test_all() {
        custom_utils::logger::logger_stdout_debug();
        test_merge();
        check_origin_position_of_final_col();
        check_col_at();
        check_final_col_of_col();
    }

    /**
     *2 |    if a.0 {...} else {...}
     */
    #[test]
    fn test_merge() {
        let line2 = init_folded_line(2, false);
        let line4 = init_folded_line(4, false);
        let line_folded_4 = init_folded_line(4, true);
        let line6 = init_folded_line(6, false);

        {
            /*
            2 |    if a.0 {...} else {
            */
            let mut lines = PhantomTextMultiLine::new(line2.clone());
            check_lines_col(
                &lines.text,
                lines.final_text_len,
                "    if true {\r\n",
                "    if true {...}"
            );
            lines.merge(line4);
            // print_lines(&lines);
            check_lines_col(
                &lines.text,
                lines.final_text_len,
                "    if true {\r\n    } else {\r\n",
                "    if true {...} else {\r\n"
            );
        }
        {
            /*
            2 |    if a.0 {...} else {...}
            */
            let mut lines = PhantomTextMultiLine::new(line2);
            check_lines_col(
                &lines.text,
                lines.final_text_len,
                "    if true {\r\n",
                "    if true {...}"
            );
            // print_lines(&lines);
            // print_line(&line_folded_4);
            lines.merge(line_folded_4);
            // print_lines(&lines);
            check_lines_col(
                &lines.text,
                lines.final_text_len,
                "    if true {\r\n    } else {\r\n",
                "    if true {...} else {...}"
            );
            lines.merge(line6);
            check_lines_col(
                &lines.text,
                lines.final_text_len,
                "    if true {\r\n    } else {\r\n    }\r\n",
                "    if true {...} else {...}\r\n"
            );
        }
    }

    #[test]
    fn check_origin_position_of_final_col() {
        _check_empty_origin_position_of_final_col();
        _check_folded_origin_position_of_final_col();
        _check_let_origin_position_of_final_col();
        _check_folded_origin_position_of_final_col_1();
    }
    fn _check_let_origin_position_of_final_col() {
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    let a = A;nr
        // "    let a: A  = A;nr
        // "0123456789012345678901234567890123456789
        // "0         10        20        30
        let let_line = PhantomTextMultiLine::new(let_data());
        // print_lines(&let_line);

        let orgin_text: Vec<char> =
            "    let a: A  = A;\r\n".chars().into_iter().collect();
        {
            assert_eq!(orgin_text[8], 'a');
            assert_eq!(let_line.cursor_position_of_final_col(8).1, 8);
        }
        {
            assert_eq!(orgin_text[11], 'A');
            assert_eq!(let_line.cursor_position_of_final_col(11).1, 9);
        }
        {
            assert_eq!(orgin_text[17], ';');
            assert_eq!(let_line.cursor_position_of_final_col(17).1, 13);
        }
        {
            assert_eq!(let_line.cursor_position_of_final_col(30).1, 15);
        }
    }

    fn _check_folded_origin_position_of_final_col_1() {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //  "    if true {nr"
        //2 "    } else {nr"
        //  "    if true {...} else {"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        //              s    e     s    e
        let line = {
            let line2 = init_folded_line(2, false);
            let line_folded_4 = init_folded_line(4, false);
            let mut lines = PhantomTextMultiLine::new(line2);
            lines.merge(line_folded_4);
            lines
        };
        // linesprint_lines(&line);
        let orgin_text: Vec<char> =
            "    if true {...} else {\r\n".chars().into_iter().collect();
        {
            assert_eq!(orgin_text[9], 'u');
            assert_eq!(line.cursor_position_of_final_col(9), (1, 9, 0));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.cursor_position_of_final_col(index), (1, 15, 0));
        }
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    if true {nr    } else {nr    }nr"
        {
            let index = 19;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.cursor_position_of_final_col(index), (3, 7, 0));
        }
        {
            assert_eq!(line.cursor_position_of_final_col(26), (3, 13, 0));
        }
    }

    fn _check_empty_origin_position_of_final_col() {
        let line = PhantomTextMultiLine::new(empty_data());
        info!("{:?}", line);
        let orgin_text: Vec<char> = "".chars().into_iter().collect();
        {
            assert_eq!(line.cursor_position_of_final_col(9), (6, 0, 0));
        }
    }
    fn _check_folded_origin_position_of_final_col() {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //  "    }nr"
        //2 "    } else {nr    }nr"
        //  "    if true {...} else {...}nr"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        //              s    e     s    e
        let line = get_merged_data();
        // print_lines(&line);
        info!("{:?}", line);
        let orgin_text: Vec<char> = "    if true {...} else {...}\r\n"
            .chars()
            .into_iter()
            .collect();
        {
            assert_eq!(orgin_text[9], 'u');
            assert_eq!(line.cursor_position_of_final_col(9), (1, 9, 0));
        }
        {
            assert_eq!(orgin_text[0], ' ');
            assert_eq!(line.cursor_position_of_final_col(0), (1, 0, 0));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.cursor_position_of_final_col(index), (1, 15, 0));
        }
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    if true {nr    } else {nr    }nr"
        {
            let index = 19;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.cursor_position_of_final_col(index), (3, 7, 0));
        }
        {
            let index = 25;
            assert_eq!(orgin_text[index], '.');
            assert_eq!(line.cursor_position_of_final_col(index), (3, 14, 0));
        }
        {
            let index = 29;
            assert_eq!(orgin_text[index], '\n');
            assert_eq!(line.cursor_position_of_final_col(index), (5, 6, 0));
        }

        {
            let index = 40;
            assert_eq!(line.cursor_position_of_final_col(index), (5, 6, 0));
        }
    }

    #[test]
    fn check_final_col_of_col() {
        _check_let_final_col_of_col();
        _check_folded_final_col_of_col();
    }
    fn _check_let_final_col_of_col() {
        let line = PhantomTextMultiLine::new(let_data());
        // print_lines(&line);
        let orgin_text: Vec<char> =
            "    let a: A  = A;\r\n".chars().into_iter().collect();
        {
            // "0         10        20        30
            // "0123456789012345678901234567890123456789
            // "    let a = A;nr
            // "    let a: A  = A;nr
            // "0123456789012345678901234567890123456789
            // "0         10        20        30
            let orgin_text: Vec<char> =
                "    let a = A;\r\n".chars().into_iter().collect();
            let col_line = 6;
            {
                let index = 8;
                assert_eq!(orgin_text[index], 'a');
                assert_eq!(line.final_col_of_col(col_line, index, true), 8);
                assert_eq!(line.final_col_of_col(col_line, index, false), 9);
            }
            {
                let index = 15;
                assert_eq!(orgin_text[index], '\n');
                assert_eq!(line.final_col_of_col(col_line, index, false), 20);
                assert_eq!(line.final_col_of_col(col_line, index, true), 19);
            }
            {
                let index = 18;
                assert_eq!(line.final_col_of_col(col_line, index, false), 20);
                assert_eq!(line.final_col_of_col(col_line, index, true), 20);
            }
        }
    }
    fn _check_folded_final_col_of_col() {
        //  "    if true {...} else {...}nr"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        let line = get_merged_data();
        // print_lines(&line);
        {
            //  "0         10        20        30
            //  "0123456789012345678901234567890123456789
            //2 "    if true {nr"
            let orgin_text: Vec<char> =
                "    if true {\r\n".chars().into_iter().collect();
            let col_line = 1;
            {
                let index = 9;
                assert_eq!(orgin_text[index], 'u');
                assert_eq!(line.final_col_of_col(col_line, index, true), 9);
                assert_eq!(line.final_col_of_col(col_line, index, false), 10);
            }
            {
                let index = 12;
                assert_eq!(orgin_text[index], '{');
                assert_eq!(line.final_col_of_col(col_line, index, true), 12);
                assert_eq!(line.final_col_of_col(col_line, index, false), 12);
            }
            let col_line = 2;
            {
                let index = 1;
                assert_eq!(line.final_col_of_col(col_line, index, false), 12);
                assert_eq!(line.final_col_of_col(col_line, index, true), 12);
            }
        }
        {
            //  "0         10        20        30
            //  "0123456789012345678901234567890123456789
            //2 "    } else {nr"
            let orgin_text: Vec<char> =
                "    } else {\r\n".chars().into_iter().collect();
            let col_line = 3;
            {
                let index = 1;
                assert_eq!(orgin_text[index], ' ');
                assert_eq!(line.final_col_of_col(col_line, index, false), 17);
                assert_eq!(line.final_col_of_col(col_line, index, true), 17);
            }
            {
                let index = 8;
                assert_eq!(orgin_text[index], 's');
                assert_eq!(line.final_col_of_col(col_line, index, true), 20);
                assert_eq!(line.final_col_of_col(col_line, index, false), 21);
            }
            {
                let index = 13;
                assert_eq!(orgin_text[index], '\n');
                assert_eq!(line.final_col_of_col(col_line, index, false), 23);
                assert_eq!(line.final_col_of_col(col_line, index, true), 23);
            }
            {
                let index = 18;
                assert_eq!(line.final_col_of_col(col_line, index, false), 23);
                assert_eq!(line.final_col_of_col(col_line, index, true), 23);
            }
        }
        {
            //  "0         10
            //  "0123456789012
            //2 "    }nr"
            let orgin_text: Vec<char> = "    }\r\n".chars().into_iter().collect();
            let col_line = 5;
            {
                let index = 1;
                assert_eq!(orgin_text[index], ' ');
                assert_eq!(line.final_col_of_col(col_line, index, false), 28);
                assert_eq!(line.final_col_of_col(col_line, index, true), 28);
            }
            {
                let index = 6;
                assert_eq!(orgin_text[index], '\n');
                assert_eq!(line.final_col_of_col(col_line, index, true), 29);
                assert_eq!(line.final_col_of_col(col_line, index, false), 30);
            }
            {
                let index = 13;
                assert_eq!(line.final_col_of_col(col_line, index, false), 30);
                assert_eq!(line.final_col_of_col(col_line, index, true), 30);
            }
        }
    }

    #[test]
    fn check_col_at() {
        {
            // "0         10        20        30
            // "0123456789012345678901234567890123456789
            // "    let a = A;nr
            // let line = PhantomTextMultiLine::new(let_data());
            // line.col_at(8).is_some()
        }
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    if true {nr    } else {nr    }nr"
        // "    if true {...} else {...}nr"
        // "0123456789012345678901234567890123456789
        // "0         10        20        30
        //              s    e     s    e
        let line = get_merged_data();
        let orgin_text: Vec<char> = "    if true {\r\n    } else {\r\n    }\r\n"
            .chars()
            .into_iter()
            .collect();
        {
            let index = 35;
            assert_eq!(orgin_text[index], '\n');
            assert_eq!(line.col_at(index), Some(29));
        }
        {
            let index = 26;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.col_at(index), None);
        }
        {
            let index = 22;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.col_at(index), Some(19));
        }
        {
            assert_eq!(orgin_text[9], 'u');
            assert_eq!(line.col_at(9), Some(9));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.col_at(index), None);
        }
        {
            let index = 19;
            assert_eq!(orgin_text[index], '}');
            assert_eq!(line.col_at(index), None);
        }
    }

    /*
    2 |    if a.0 {...} else {...}
    */
    fn get_merged_data() -> PhantomTextMultiLine {
        let line2 = init_folded_line(2, false);
        let line_folded_4 = init_folded_line(4, true);
        let line6 = init_folded_line(6, false);
        let mut lines = PhantomTextMultiLine::new(line2);
        lines.merge(line_folded_4);
        lines.merge(line6);
        lines
    }

    fn check_lines_col(
        lines: &SmallVec<[Text; 6]>,
        final_text_len: usize,
        origin: &str,
        expect: &str
    ) {
        let rs = combine_with_text(lines, origin);
        assert_eq!(expect, rs.as_str());
        assert_eq!(final_text_len, expect.len());
    }

    fn check_line_final_col(lines: &PhantomTextLine, rs: &str) {
        for text in &lines.texts {
            if let Text::Phantom { text } = text {
                assert_eq!(
                    text.text.as_str(),
                    sub_str(rs, text.final_col, text.final_col + text.text.len())
                );
            }
        }
    }

    fn sub_str(text: &str, begin: usize, end: usize) -> &str {
        unsafe { text.get_unchecked(begin..end) }
    }

    fn print_line(lines: &PhantomTextLine) {
        println!(
            "PhantomTextLine line={} origin_text_len={} final_text_len={}",
            lines.line, lines.origin_text_len, lines.final_text_len
        );
        for text in &lines.texts {
            match text {
                Text::Phantom { text } => {
                    println!(
                        "Phantom {:?} line={} col={} merge_col={} final_col={} \
                         text={} text.len()={}",
                        text.kind,
                        text.line,
                        text.col,
                        text.merge_col,
                        text.final_col,
                        text.text,
                        text.text.len()
                    );
                },
                Text::OriginText { text } => {
                    println!(
                        "OriginText line={} col={:?} merge_col={:?} final_col={:?}",
                        text.line, text.col, text.merge_col, text.final_col
                    );
                },
                Text::EmptyLine { .. } => {
                    println!("Empty");
                }
            }
        }
        println!();
    }
}

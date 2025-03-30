use std::{cmp::Ordering, fmt, ops::Range};

use anyhow::{Result, anyhow, bail};
use floem::{
    peniko::Color,
    text::{Attrs, AttrsList},
};
use lapce_xi_rope::Interval;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::lines::cursor::CursorAffinity;

/// `PhantomText` is for text that is not in the actual document, but
/// should be rendered with it.
///
/// Ex: Inlay hints, IME text, error lens' diagnostics, etc
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PhantomText {
    /// The kind is currently used for sorting the phantom text on a
    /// line
    pub kind:             PhantomTextKind,
    /// Column on the line that the phantom text should be displayed
    /// at
    ///
    /// 在原始文本的行
    pub line:             usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 在原始行line文本的位置
    pub col:              usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 合并后，在多行原始行文本（不考虑被折叠行、幽灵文本）的位置。
    /// 与col相差前面折叠行的总长度
    pub visual_merge_col: usize,
    /// 合并后，在多行原始行文本（不考虑幽灵文本，考虑被折叠行）的位置。
    /// 与col相差前面折叠行的总长度
    pub origin_merge_col: usize,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本（考虑折叠、幽灵文本）的位置
    pub final_col:        usize,
    pub text:             String,
    pub font_size:        Option<usize>,
    // font_family: Option<FontFamily>,
    pub fg:               Option<Color>,
    pub bg:               Option<Color>,
    pub under_line:       Option<Color>,
}

impl PartialEq for PhantomText {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.line == other.line
            && self.col == other.col
            && self.visual_merge_col == other.visual_merge_col
            && self.origin_merge_col == other.origin_merge_col
            && self.final_col == other.final_col
            && self.text == other.text
            && self.font_size == other.font_size
    }
}
impl Eq for PhantomText {}
impl fmt::Debug for PhantomText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhantomText")
            .field("kind", &self.kind)
            .field("line", &self.line)
            .field("col", &self.col)
            .field("visual_merge_col", &self.visual_merge_col)
            .field("origin_merge_col", &self.origin_merge_col)
            .field("final_col", &self.final_col)
            .field("text", &self.text)
            .finish()
    }
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

    fn next_origin_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang { len, .. } = self.kind {
            self.col + len
        } else {
            self.col
        }
    }

    /// todo 不准确，存在连续phantom的情况?
    pub fn next_visual_merge_col(&self) -> usize {
        // if let PhantomTextKind::LineFoldedRang { len, .. } = self.kind {
        // self.visual_merge_col + self.text.len()
        // } else {
        //     self.visual_merge_col
        // }
        if let PhantomTextKind::LineFoldedRang { len, .. } = self.kind {
            self.visual_merge_col + len
        } else {
            self.visual_merge_col
        }
    }

    pub fn next_origin_merge_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang { all_len, .. } = self.kind {
            self.origin_merge_col + all_len
        } else {
            self.origin_merge_col
        }
    }

    pub fn log(&self) {
        info!(
            "{:?} line={} col={} final_col={} text={} text.len()={}",
            self.kind,
            self.line,
            self.visual_merge_col,
            self.final_col,
            self.text,
            self.text.len()
        );
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginText {
    /// 在原始文本的行
    pub line:             usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 在原始行文本的位置
    pub col:              Interval,
    /// 合并后原始行文本的位置，不包含虚拟文本和被折叠行
    pub visual_merge_col: Interval,
    /// 合并后原始行文本的位置，包含被折叠行，但不包含虚拟文本
    origin_merge_col:     Interval,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本的位置，包含虚拟文本
    pub final_col:        Interval,
}

impl OriginText {
    /// 视觉偏移的原始偏移
    pub fn origin_col_of_final_col(&self, final_col: usize) -> usize {
        final_col - self.final_col.start + self.col.start
    }

    pub fn origin_merge_col_contains(&self, offset: usize, last_line: bool) -> bool {
        if last_line {
            self.origin_merge_col.start <= offset
                && offset <= self.origin_merge_col.end
        } else {
            self.origin_merge_col.contains(offset)
        }
    }

    #[inline]
    pub fn origin_merge_col_start(&self) -> usize {
        self.origin_merge_col.start
    }

    #[inline]
    pub fn origin_merge_col_end(&self) -> usize {
        self.origin_merge_col.end
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EmptyText {
    /// 在原始文本的行
    pub line:           usize,
    /// Column on the line that the phantom text should be displayed
    /// at.Provided by lsp
    ///
    /// 在原始行文本的位置
    pub offset_of_line: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Text {
    Phantom { text: PhantomText },
    OriginText { text: OriginText },
    EmptyLine { text: EmptyText },
}

impl Text {
    pub fn is_phantom(&self) -> bool {
        if let Text::Phantom { .. } = self {
            true
        } else {
            false
        }
    }

    // pub fn adjust(&mut self, line_delta: Offset, _offset_delta: Offset) {
    //     match self {
    //         Text::Phantom { text } => {
    //             text.kind.adjust(line_delta);
    //             line_delta.adjust(&mut text.line);
    //         },
    //         Text::OriginText { text } => {
    //             line_delta.adjust(&mut text.line);
    //         },
    //         _ => {},
    //     }
    // }

    fn merge_to(
        mut self,
        merge_offset: usize,
        origin_merge_offset: usize,
        final_text_len: usize,
    ) -> Self {
        match &mut self {
            Text::Phantom { text } => {
                text.visual_merge_col += merge_offset;
                text.origin_merge_col += origin_merge_offset;
                text.final_col += final_text_len;
            },
            Text::OriginText { text } => {
                text.visual_merge_col =
                    text.visual_merge_col.translate(merge_offset);
                text.origin_merge_col =
                    text.origin_merge_col.translate(origin_merge_offset);
                text.final_col = text.final_col.translate(final_text_len);
            },
            _ => {},
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

#[derive(
    Debug,
    Clone,
    Copy,
    Ord,
    Eq,
    PartialEq,
    PartialOrd,
    Default,
    Serialize,
    Deserialize,
)]
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
    // 行内折叠。跨行折叠也都转换成行内折叠。跨行折叠会转成2个PhantomText
    LineFoldedRang {
        next_line:      Option<usize>,
        // 本行被折叠的长度
        len:            usize,
        // 包括其他折叠行的长度
        all_len:        usize,
        start_position: usize,
    },
}

impl PhantomTextKind {
    // pub fn adjust(&mut self, line_delta: Offset) {
    //     if let Self::LineFoldedRang {
    //         next_line,
    //         start_position,
    //         ..
    //     } = self
    //     {
    //         let mut position_line = start_position.line as usize;
    //         line_delta.adjust(&mut position_line);
    //         start_position.line = position_line as u32;
    //         if let Some(x) = next_line.as_mut() {
    //             line_delta.adjust(x)
    //         }
    //     }
    // }
}

/// Information about the phantom text on a specific line.
///
/// This has various utility functions for transforming a coordinate
/// (typically a column) into the resulting coordinate after the
/// phantom text is combined with the line's real content.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
    pub texts:          SmallVec<[Text; 6]>,
}

impl PhantomTextLine {
    pub fn new(
        line: usize,
        origin_text_len: usize,
        offset_of_line: usize,
        mut phantom_texts: SmallVec<[PhantomText; 6]>,
    ) -> Result<Self> {
        phantom_texts.sort_by(|a, b| {
            if a.visual_merge_col == b.visual_merge_col {
                a.kind.cmp(&b.kind)
            } else {
                a.visual_merge_col.cmp(&b.visual_merge_col)
            }
        });

        let mut final_last_end = 0;
        let mut origin_merge_col_last_end = 0;
        let mut merge_last_end = 0;
        let mut texts = SmallVec::new();

        let mut final_offset = 0i32;
        for mut phantom in phantom_texts {
            match phantom.kind {
                PhantomTextKind::LineFoldedRang { len, .. } => {
                    phantom.final_col =
                        usize_offset(phantom.final_col, final_offset)?;
                    final_offset =
                        final_offset + phantom.text.len() as i32 - len as i32;
                    if origin_text_len as i32 + final_offset < 0 {
                        error!(
                            "{phantom:?} line={line} \
                             origin_text_len={origin_text_len} \
                             =offset_of_line{offset_of_line}"
                        );
                    }
                },
                _ => {
                    phantom.final_col =
                        usize_offset(phantom.final_col, final_offset)?;
                    final_offset += phantom.text.len() as i32;
                },
            }
            // phantom.visual_merge_col = phantom.final_col;
            if final_last_end < phantom.final_col {
                let len = phantom.final_col - final_last_end;
                // insert origin text
                texts.push(
                    OriginText {
                        line:             phantom.line,
                        col:              Interval::new(
                            origin_merge_col_last_end,
                            origin_merge_col_last_end + len,
                        ),
                        visual_merge_col: Interval::new(
                            merge_last_end,
                            merge_last_end + len,
                        ),
                        origin_merge_col: Interval::new(
                            origin_merge_col_last_end,
                            origin_merge_col_last_end + len,
                        ),
                        final_col:        Interval::new(
                            final_last_end,
                            final_last_end + len,
                        ),
                    }
                    .into(),
                );
            }
            final_last_end = phantom.next_final_col();
            origin_merge_col_last_end = phantom.next_origin_col();
            merge_last_end = phantom.next_visual_merge_col();
            texts.push(phantom.into());
        }

        if origin_text_len > origin_merge_col_last_end {
            let len = origin_text_len - origin_merge_col_last_end;
            texts.push(
                OriginText {
                    line,
                    col: Interval::new(
                        origin_merge_col_last_end,
                        origin_merge_col_last_end + len,
                    ),
                    visual_merge_col: Interval::new(
                        merge_last_end,
                        merge_last_end + len,
                    ),
                    origin_merge_col: Interval::new(
                        merge_last_end,
                        merge_last_end + len,
                    ),
                    final_col: Interval::new(final_last_end, final_last_end + len),
                }
                .into(),
            );
        } else if origin_text_len == 0 {
            texts.push(
                EmptyText {
                    line,
                    offset_of_line,
                }
                .into(),
            );
        }

        let final_text_len = usize_offset(origin_text_len, final_offset)?;
        Ok(Self {
            final_text_len,
            line,
            origin_text_len,
            texts,
            offset_of_line,
        })
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
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PhantomTextMultiLine {
    /// 原始文本的行号
    pub line:            usize,
    pub last_line:       usize,
    pub is_last_line:    bool,
    /// line行起点在文本中的偏移
    pub offset_of_line:  usize,
    // 所有合并在该行的原始行的总长度，中间被合并的行不在计算范围
    pub origin_text_len: usize,
    /// 所有合并在该行的最后展现的长度，包括幽灵文本、换行符、
    /// 包括后续的折叠行
    pub final_text_len:  usize,
    // // 各个原始行的行号、原始长度、最后展现的长度
    // pub len_of_line:     Vec<(usize, usize, usize)>,
    /// This uses a smallvec because most lines rarely have more than
    /// a couple phantom texts
    pub text:            SmallVec<[Text; 6]>, /* 可以去掉，仅做记录
                                               * pub lines:
                                               * Vec<PhantomTextLine>,
                                               */
}

impl PhantomTextMultiLine {
    pub fn new(line: PhantomTextLine, is_last_line: bool) -> Self {
        // let len_of_line =
        //     vec![(line.line, line.origin_text_len, line.final_text_len)];
        Self {
            line: line.line,
            last_line: line.line,
            is_last_line,
            offset_of_line: line.offset_of_line,
            origin_text_len: line.origin_text_len,
            final_text_len: line.final_text_len,
            // len_of_line,
            text: line.texts,
        }
    }

    pub fn merge(&mut self, line: PhantomTextLine, is_last_line: bool) {
        // 注意被折叠的长度
        let visual_merge_offset = self.origin_text_len;
        let origin_merge_offset = line.offset_of_line - self.offset_of_line;
        // let origin_text_len = self.origin_text_len;
        self.origin_text_len += line.origin_text_len;
        let final_text_len = self.final_text_len;
        self.final_text_len += line.final_text_len;
        for phantom in line.texts.clone() {
            self.text.push(phantom.merge_to(
                visual_merge_offset,
                origin_merge_offset,
                final_text_len,
            ));
        }
        self.last_line = line.line;
        self.is_last_line = is_last_line;
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
        phantom_color: Color,
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
                            (phantom_font_size as f32).min(attrs.font_size),
                        );
                    }
                    attrs_list.add_span(
                        text.final_col..(text.final_col + text.text.len()),
                        attrs,
                    );
                }
            },
            Text::OriginText { .. } | Text::EmptyLine { .. } => {},
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
    pub fn text_of_visual_char(&self, visual_char_offset: usize) -> &Text {
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
                    Text::EmptyLine { .. } => return true,
                }
                false
            })
            .unwrap()
    }

    pub fn last_origin_merge_col(&self) -> Option<usize> {
        for text in self.text.iter().rev() {
            match text {
                Text::OriginText { text } => {
                    return Some(text.origin_merge_col_end() - 1);
                },
                Text::EmptyLine { text } => return Some(text.offset_of_line),
                _ => continue,
            }
        }
        // will occur?
        None
    }

    // fn text_of_origin_line_col(
    //     &self,
    //     origin_line: usize,
    //     origin_col: usize
    // ) -> Option<&Text> {
    //     self.text.iter().find(|x| {
    //         match x {
    //             Text::Phantom { text } => {
    //                 if text.line == origin_line
    //                     && text.col <= origin_col
    //                     && origin_col < text.next_origin_col()
    //                 {
    //                     return true;
    //                 } else if let Some(next_line) = text.next_line() {
    //                     if origin_line < next_line {
    //                         return true;
    //                     }
    //                 }
    //             },
    //             Text::OriginText { text } => {
    //                 if text.line == origin_line && text.col.contains(origin_col)
    // {                     return true;
    //                 }
    //             },
    //             Text::EmptyLine { .. } => return true
    //         }
    //         false
    //     })
    // }

    pub(crate) fn final_col_of_origin_line_col(
        &self,
        origin_line: usize,
        origin_col: usize,
        origin_line_end: usize,
        origin_col_end: usize,
    ) -> Option<(usize, usize)> {
        self.text.iter().find_map(|x| {
            if let Text::OriginText { text } = x {
                if text.line == origin_line
                    && text.line == origin_line_end
                    && text.col.contains(origin_col)
                    && origin_col_end <= text.col.end()
                {
                    return Some((
                        origin_col - text.col.start + text.final_col.start,
                        origin_col_end - text.col.start + text.final_col.start,
                    ));
                }
            }
            None
        })
    }

    /// merge col一定会出现在某个text中
    pub fn text_of_origin_merge_col(
        &self,
        origin_merge_col: usize,
    ) -> Result<&Text> {
        self.text
            .iter()
            .find(|x| {
                match x {
                    Text::Phantom { text } => {
                        if text.origin_merge_col <= origin_merge_col
                            && origin_merge_col <= text.next_origin_merge_col()
                        {
                            return true;
                        }
                    },
                    Text::OriginText { text } => {
                        if text.origin_merge_col_contains(
                            origin_merge_col,
                            self.is_last_line,
                        ) {
                            return true;
                        }
                    },
                    Text::EmptyLine { .. } => {
                        return true;
                    },
                }
                false
            })
            .ok_or(anyhow!("No merge col found {}", origin_merge_col,))
    }

    /// 最终文本的原始文本位移。若为幽灵则返回none.超过最终文本长度，
    /// 则返回none(不应该在此情况下调用该方法)
    pub fn origin_col_of_final_offset(
        &self,
        final_col: usize,
    ) -> Option<(usize, usize)> {
        // let final_col = final_col.min(self.final_text_len - 1);
        if let Text::OriginText { text } = self.text_of_visual_char(final_col) {
            let origin_col = text.col.start + final_col - text.final_col.start;
            return Some((text.line, origin_col));
        }
        None
    }

    pub fn final_col_of_origin_merge_col(
        &self,
        origin_merge_col: usize,
    ) -> Result<Option<usize>> {
        let text = self.text_of_origin_merge_col(origin_merge_col)?;
        Ok(match text {
            Text::Phantom { .. } => None,
            Text::OriginText { text } => Some(
                text.final_col.start + origin_merge_col
                    - text.origin_merge_col.start,
            ),
            Text::EmptyLine { .. } => None,
        })
    }

    pub fn cursor_final_col_of_origin_merge_col(
        &self,
        origin_merge_col: usize,
        cursor_affinity: CursorAffinity,
    ) -> Result<usize> {
        // let Ok(text) = self.text_of_merge_col(merge_col) else {
        //     warn!("merge_col not found: line={} merge col={}", self.line,
        // merge_col);     return None;
        // };
        let text = self.text_of_origin_merge_col(origin_merge_col)?;
        Ok(match text {
            Text::Phantom { text, .. } => match cursor_affinity {
                CursorAffinity::Forward => text.next_final_col(),
                CursorAffinity::Backward => text.final_col,
            },
            Text::OriginText { text } => match cursor_affinity {
                CursorAffinity::Forward => {
                    text.final_col.start + origin_merge_col
                        - text.origin_merge_col.start
                        + 1
                },
                CursorAffinity::Backward => {
                    text.final_col.start + origin_merge_col
                        - text.origin_merge_col.start
                },
            },
            Text::EmptyLine { .. } => 0,
        })
    }

    // /// 原始行的偏移字符！！！，的对应的合并后的位置。
    // /// 用于求鼠标的实际位置
    // ///
    // /// Translate a column position into the text into what it would
    // /// be after combining
    // ///
    // /// 暂时不考虑_before_cursor，等足够熟悉了再说.
    // /// true: |a
    // //  false: a|
    // pub fn final_col_of_col(
    //     &self,
    //     line: usize,
    //     pre_col: usize,
    //     _before_cursor: bool
    // ) -> usize {
    //     let adjust_offset = if _before_cursor { 0 } else { 1 };
    //     if self.text.is_empty() {
    //         return pre_col;
    //     }
    //     let text = self.text_of_origin_line_col(line, pre_col);
    //     if let Some(text) = text {
    //         match text {
    //             Text::Phantom { text } => {
    //                 if text.col == 0 {
    //                     // 后一个字符
    //                     text.next_final_col()
    //                 } else {
    //                     // 前一个字符
    //                     text.final_col
    //                 }
    //             },
    //             Text::OriginText { text } => {
    //                 text.final_col.start + pre_col - text.col.start +
    // adjust_offset             },
    //             Text::EmptyLine { .. } => 0
    //         }
    //     } else {
    //         self.final_text_len
    //     }
    // }

    pub fn visual_offset_of_cursor_offset(
        &self,
        origin_line: usize,
        origin_col: usize,
        affinity: CursorAffinity,
    ) -> Option<usize> {
        for x in &self.text {
            match x {
                Text::Phantom { text } => {
                    match text.line.cmp(&origin_line) {
                        Ordering::Less => {
                            if let Some(next_line) = text.next_line() {
                                // be merged
                                if origin_line < next_line {
                                    return None;
                                }
                            }
                            continue;
                        },
                        Ordering::Equal => match text.col.cmp(&origin_col) {
                            Ordering::Less => {
                                if text.next_line().is_some() {
                                    return None;
                                }
                            },
                            Ordering::Equal => {
                                return Some(if affinity.forward() {
                                    text.next_final_col()
                                } else {
                                    text.final_col
                                });
                            },
                            Ordering::Greater => break,
                        },
                        Ordering::Greater => {
                            break;
                        },
                    }
                },
                Text::OriginText { text } => {
                    if text.line == origin_line && text.col.contains(origin_col) {
                        return Some(
                            text.final_col.start + origin_col - text.col.start,
                        );
                    }
                },
                Text::EmptyLine { .. } => return Some(0),
            }
        }
        Some(self.final_text_len)
    }

    // /// the position of the cursor should not be in phantom text
    // ///
    // /// return (buffer offset, cursor affinity
    // pub fn cursor_position_of_final_col(
    //     &self,
    //     mut visual_char_offset: usize
    // ) -> (usize, CursorAffinity) {
    //     // // 因为通过hit_point获取的index会大于等于final_text_len
    //     // // visual_char_offset可能是其他行的final_col
    //     // if visual_char_offset >= self.final_text_len {
    //     //     // the final_text_len of an empty line equals 0
    //     //     visual_char_offset = self.final_text_len.max(1) - 1;
    //     // }
    //     match self.text_of_final_col_even_overflow(visual_char_offset) {
    //         Text::Phantom { text } => {
    //             // 在虚拟文本的后半部分，则光标置于虚拟文本之后
    //             if visual_char_offset > text.final_col + text.text.len() / 2 {
    //                 (
    //                     text.merge_col + self.offset_of_line,
    //                     CursorAffinity::Forward
    //                 )
    //             } else {
    //                 (
    //                     text.merge_col + self.offset_of_line,
    //                     CursorAffinity::Backward
    //                 )
    //             }
    //         },
    //         Text::OriginText { text } => {
    //
    //             let merge_col = visual_char_offset - text.final_col.start +
    // text.merge_col.start;
    //
    //             (
    //                 text.line,
    //                 text.origin_col_of_final_col(visual_char_offset),
    //                 visual_char_offset,
    //                 self.offset_of_line,
    //                 CursorAffinity::Backward
    //             )
    //         },
    //         Text::EmptyLine { text } => (text.line, 0, 0, text.offset_of_line,
    // CursorAffinity::Backward)     }
    // }

    pub fn text_of_final_col_even_overflow(
        &self,
        mut visual_char_offset: usize,
    ) -> &Text {
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
        col: usize,
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

    // pub fn adjust(&mut self, line_delta: Offset, offset_delta: Offset) {
    //     line_delta.adjust(&mut self.line);
    //     line_delta.adjust(&mut self.last_line);

    //     offset_delta.adjust(&mut self.offset_of_line);
    //     // for (line, _, _) in &mut self.len_of_line {
    //     //     line_delta(line);
    //     // }
    //     for text in &mut self.text {
    //         text.adjust(line_delta, offset_delta);
    //     }
    // }
}

fn usize_offset(val: usize, offset: i32) -> Result<usize> {
    let rs = val as i32 + offset;
    if rs < 0 {
        bail!("value out of range: val={val}, offset={offset}");
    }
    Ok(rs as usize)
}

/// Not allowed to cross the range??
pub struct Ranges {
    ranges: Vec<Range<usize>>,
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

pub fn combine_with_text(lines: &[Text], origin: &str) -> String {
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
                    text.visual_merge_col.start.min(origin.len()),
                    text.visual_merge_col.end.min(origin.len()),
                ));
            },
            Text::EmptyLine { .. } => {
                break;
            },
        }
    }
    rs
}

fn sub_str(text: &str, begin: usize, end: usize) -> &str {
    unsafe { text.get_unchecked(begin..end) }
}

use std::{iter::Peekable, ops::RangeInclusive, slice::Iter};

use anyhow::{Result, anyhow};
use floem::{
    peniko::Color,
    prelude::{RwSignal, SignalGet, SignalUpdate},
    reactive::Scope,
};
use im::HashMap;
use lapce_xi_rope::{
    Interval,
    spans::{Spans, SpansBuilder},
};
use log::error;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use super::phantom_text::{PhantomText, PhantomTextKind};
use crate::lines::{
    buffer::{Buffer, rope_text::RopeText},
    screen_lines::ScreenLines,
};

pub struct FoldingRangesLine<'a> {
    folding: Peekable<Iter<'a, FoldedRange>>,
}

pub struct MergeFoldingRangesLine<'a> {
    folding: Peekable<Iter<'a, FoldedRange>>,
}

impl<'a> MergeFoldingRangesLine<'a> {
    pub fn new(folding: &'a [FoldedRange]) -> Self {
        let folding = folding.iter().peekable();
        Self { folding }
    }

    /// 计算line在实际展示时，位于第几行
    pub fn get_line_num(
        &mut self,
        origin_folded_index: usize,
        last_line: usize,
    ) -> Option<usize> {
        let mut index = 0;
        let mut line_num = 0;
        while line_num <= last_line {
            if index == origin_folded_index {
                return Some(line_num);
            }
            if let Some(folded) = self.get_folded_range_by_line(line_num) {
                line_num = *folded.end() + 1;
            } else {
                line_num += 1;
            }
            index += 1;
        }
        None
    }

    pub fn get_folded_range_by_line(
        &mut self,
        line: usize,
    ) -> Option<RangeInclusive<usize>> {
        loop {
            if let Some(folded) = self.folding.peek() {
                if folded.end_line < line {
                    self.folding.next();
                    continue;
                } else if folded.start_line <= line && line <= folded.end_line {
                    let start_line = folded.start_line;
                    let mut end_line = folded.end_line;
                    self.folding.next();
                    while let Some(next_folded) = self.folding.peek() {
                        if next_folded.start_line == end_line {
                            end_line = next_folded.end_line;
                            self.folding.next();
                            continue;
                        } else {
                            break;
                        }
                    }
                    return Some(start_line..=end_line);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    }

    /// 计算line在实际展示时，位于第几行
    pub fn get_origin_folded_line_index(&mut self, line: usize) -> usize {
        let mut index = 0;
        let mut line_num = 0;
        while line_num <= line {
            if line_num >= line {
                break;
            }
            if let Some(folded) = self.get_folded_range_by_line(line_num) {
                line_num = *folded.end() + 1;
            } else {
                line_num += 1;
            }
            index += 1;
        }
        index
    }
}
impl<'a> FoldingRangesLine<'a> {
    pub fn new(folding: &'a [FoldedRange]) -> Self {
        let folding = folding.iter().peekable();
        Self { folding }
    }

    pub fn get_folded_range_by_line(
        &mut self,
        line: usize,
    ) -> Option<RangeInclusive<usize>> {
        loop {
            if let Some(folded) = self.folding.peek() {
                if folded.end_line < line {
                    self.folding.next();
                    continue;
                } else if folded.start_line <= line && line <= folded.end_line {
                    let start_line = folded.start_line;
                    let mut end_line = folded.end_line;
                    self.folding.next();
                    while let Some(next_folded) = self.folding.peek() {
                        if next_folded.start_line == end_line {
                            end_line = next_folded.end_line;
                            self.folding.next();
                            continue;
                        } else {
                            break;
                        }
                    }
                    return Some(start_line..=end_line);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    }

    pub fn contain_offset(&mut self, offset: usize) -> bool {
        loop {
            if let Some(folded) = self.folding.peek() {
                if folded.interval.end < offset {
                    self.folding.next();
                    continue;
                } else {
                    return folded.interval.contains(offset);
                }
            } else {
                return false;
            }
        }
    }

    pub fn phantom_text(
        &mut self,
        line: usize,
        buffer: &Buffer,
        inlay_hint_font_size: usize,
        inlay_hint_foreground: Color,
        inlay_hint_background: Color,
    ) -> Result<SmallVec<[PhantomText; 6]>> {
        let mut textes = SmallVec::<[PhantomText; 6]>::new();
        loop {
            if let Some(folded) = self.folding.peek() {
                let offset_of_start_line =
                    buffer.offset_of_line(folded.start_line)?;
                let start = folded.interval.start - offset_of_start_line;
                let offset_of_end_line = buffer.offset_of_line(folded.end_line)?;
                let content_len_of_start_line =
                    buffer.line_content(folded.start_line)?.len();
                if folded.end_line < line {
                    self.folding.next();
                    continue;
                } else if folded.start_line == line {
                    let same_line = folded.start_line == folded.end_line;
                    let Some(start_char) =
                        buffer.char_at_offset(folded.interval.start)
                    else {
                        self.folding.next();
                        continue;
                    };
                    let Some(end_char) =
                        buffer.char_at_offset(folded.interval.end - 1)
                    else {
                        self.folding.next();
                        continue;
                    };

                    let mut text = String::new();
                    text.push(start_char);
                    text.push_str("...");
                    text.push(end_char);
                    let next_line = if same_line {
                        None
                    } else {
                        Some(folded.end_line)
                    };

                    let (all_len, len) = if same_line {
                        (folded.interval.size(), folded.interval.size())
                    } else {
                        (folded.interval.size(), content_len_of_start_line - start)
                    };
                    textes.push(PhantomText {
                        kind: PhantomTextKind::LineFoldedRang {
                            next_line,
                            len,
                            all_len,
                            start_position: folded.interval.start,
                        },
                        col: start,
                        text,
                        fg: Some(inlay_hint_foreground),
                        font_size: Some(inlay_hint_font_size),
                        bg: Some(inlay_hint_background),
                        under_line: None,
                        final_col: start,
                        line,
                        visual_merge_col: start,
                        origin_merge_col: start,
                    });
                    if !same_line {
                        break;
                    } else {
                        self.folding.next();
                    }
                } else if folded.end_line == line {
                    let text = String::new();
                    textes.push(PhantomText {
                        kind: PhantomTextKind::LineFoldedRang {
                            next_line:      None,
                            len:            folded.interval.end - offset_of_end_line,
                            all_len:        folded.interval.end - offset_of_end_line,
                            start_position: folded.interval.start,
                        },
                        col: 0,
                        text,
                        fg: None,
                        font_size: None,
                        bg: None,
                        under_line: None,
                        final_col: 0,
                        line,
                        visual_merge_col: 0,
                        origin_merge_col: 0,
                    });
                    self.folding.next();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(textes)
    }
}

#[derive(Default, Clone)]
pub struct FoldingRanges(pub Spans<RwSignal<FoldingRange>>);

#[derive(Default, Clone, Debug)]
pub struct FoldedRanges(pub Vec<FoldedRange>);

impl FoldingRanges {
    /// 将衔接在一起的range合并在一条中，这样便于找到合并行的起始行
    pub fn get_all_folded_folded_range(&self, buffer: &Buffer) -> FoldedRanges {
        let mut range = Vec::new();
        let mut limit_line = 0;
        let mut peek = self.0.iter().peekable();
        while let Some((interval, item)) = peek.next() {
            let item = item.get_untracked();

            if item.status.is_folded() {
                let start_line = buffer.line_of_offset(interval.start);
                let end_line = buffer.line_of_offset(interval.end);
                if start_line < limit_line && limit_line > 0 {
                    continue;
                }
                let mut end = end_line;
                while let Some((next_interval, _next_item)) = peek.peek() {
                    if _next_item.get_untracked().status.is_folded() {
                        let next_start_line =
                            buffer.line_of_offset(next_interval.start);
                        if end_line == next_start_line {
                            end = buffer.line_of_offset(next_interval.end);
                            peek.next();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                range.push(FoldedRange {
                    interval,
                    end_line: end,
                    start_line,
                });
                limit_line = end_line;
            }
        }
        FoldedRanges(range)
    }

    pub fn get_all_folded_range(&self, buffer: &Buffer) -> FoldedRanges {
        // 不能合并，因为后续是一行一行拼接的。合并会导致中间行缺失
        let mut range = Vec::new();
        let mut limit_line = 0;
        for (interval, item) in self.0.iter() {
            let item = item.get_untracked();
            let start_line = buffer.line_of_offset(interval.start);
            let end_line = buffer.line_of_offset(interval.end);
            if start_line < limit_line && limit_line > 0 {
                continue;
            }
            if item.status.is_folded() {
                range.push(FoldedRange {
                    interval,
                    end_line,
                    start_line,
                });
                limit_line = end_line;
            }
        }

        FoldedRanges(range)
    }

    // pub fn get_folded_range_by_line(&self, line: u32) -> FoldedRanges {
    //     let mut range = Vec::new();
    //     let mut limit_line = 0;
    //     for item in &self.0 {
    //         if item.start.line < limit_line && limit_line > 0 {
    //             continue;
    //         }
    //         if item.status.is_folded()
    //             && item.start.line <= line
    //             && item.end.line >= line
    //         {
    //             range.push(FoldedRange {
    //                 start:          item.start,
    //                 end:            item.end,
    //                 collapsed_text: item.collapsed_text.clone()
    //             });
    //             limit_line = item.end.line;
    //         }
    //     }
    //
    //     FoldedRanges(range)
    // }

    /// 所有包含该offset的折叠都要展开
    pub fn unfold_all_range_by_offset(&mut self, offset: usize) -> Result<()> {
        for (iv, item) in self.0.iter() {
            if item
                .try_update(|item| {
                    if iv.contains(offset) {
                        item.status = FoldingRangeStatus::Unfold;
                        // return Ok(Some(start));
                    } else if iv.end < offset {
                    } else {
                        return Result::<bool, anyhow::Error>::Ok(true);
                    }
                    Ok(false)
                })
                .ok_or(anyhow!("update fail"))??
            {
                break;
            }
        }
        Ok(())
    }

    /// 最小包含该offset的范围要折叠
    pub fn fold_min_range_by_offset(&mut self, offset: usize) -> Option<usize> {
        if let Some((item, iv)) = self.find_range_by_offset(offset) {
            item.update(|x| x.status = FoldingRangeStatus::Fold);
            Some(iv)
        } else {
            None
        }
    }

    pub fn find_range_by_offset(
        &mut self,
        offset: usize,
    ) -> Option<(RwSignal<FoldingRange>, usize)> {
        let mut fold_item: Option<(RwSignal<FoldingRange>, usize)> = None;
        for (interval, item) in self.0.iter() {
            if interval.contains(offset) {
                if !fold_item
                    .as_ref()
                    .map(|x| x.1 > interval.start)
                    .unwrap_or_default()
                {
                    fold_item = Some((*item, interval.start));
                }
            } else if interval.end < offset {
                continue;
            } else {
                break;
            }
        }
        fold_item
    }

    pub fn to_display_items(
        &self,
        lines: &ScreenLines,
        buffer: &Buffer,
    ) -> Vec<FoldingDisplayItem> {
        let mut folded = HashMap::new();
        let mut unfold_start: HashMap<usize, FoldingDisplayItem> = HashMap::new();
        let mut unfold_end = HashMap::new();
        let mut limit_line = 0;
        for (iv, item) in self.0.iter() {
            let item = item.get_untracked();
            let start_line = buffer.line_of_offset(iv.start);
            let end_line = buffer.line_of_offset(iv.end);
            if start_line < limit_line && limit_line > 0 {
                continue;
            }
            match item.status {
                FoldingRangeStatus::Fold => {
                    if let Some(line) =
                        lines.visual_line_info_for_origin_line(start_line)
                    {
                        folded.insert(
                            start_line,
                            FoldingDisplayItem {
                                iv,
                                // position: item.start,
                                y: line.folded_line_y() as i32,
                                ty: FoldingDisplayType::Folded,
                            },
                        );
                    }
                    limit_line = end_line;
                },
                FoldingRangeStatus::Unfold => {
                    {
                        if let Some(line) =
                            lines.visual_line_info_for_origin_line(start_line)
                        {
                            unfold_start.insert(
                                start_line,
                                FoldingDisplayItem {
                                    iv,
                                    // position: item.start,
                                    y: line.folded_line_y() as i32,
                                    ty: FoldingDisplayType::UnfoldStart,
                                },
                            );
                        }
                    }
                    {
                        if let Some(line) =
                            lines.visual_line_info_for_origin_line(end_line)
                        {
                            unfold_end.insert(
                                end_line,
                                FoldingDisplayItem {
                                    iv,
                                    // position: item.end,
                                    y: line.folded_line_y() as i32,
                                    ty: FoldingDisplayType::UnfoldEnd,
                                },
                            );
                        }
                    }
                    limit_line = 0;
                },
            };
        }
        for (key, val) in unfold_end {
            unfold_start.insert(key, val);
        }
        for (key, val) in folded {
            unfold_start.insert(key, val);
        }
        let mut items: Vec<FoldingDisplayItem> =
            unfold_start.into_iter().map(|x| x.1).collect();
        items.sort_by(|x, y| {
            x.iv.start.cmp(&y.iv.start)
            // let line_rs = x.position.line.cmp(&y.position.line);
            // if let Ordering::Equal = line_rs {
            //     x.position.character.cmp(&y.position.character)
            // } else {
            //     line_rs
            // }
        });
        items
    }

    pub fn update_ranges(
        &mut self,
        new: Vec<lsp_types::FoldingRange>,
        buffer: &Buffer,
        cx: Scope,
    ) -> Result<()> {
        let folded_range = self.get_all_folded_range(buffer);
        let mut builder = SpansBuilder::new(buffer.len());
        for item in new {
            let start = buffer.offset_of_line_col(
                item.start_line as usize,
                item.start_character.unwrap_or_default() as usize,
            )?;
            let end = buffer.offset_of_line_col(
                item.end_line as usize,
                item.end_character.unwrap_or_default() as usize,
            )?;
            // let start = buffer.offset_of_position(&item.start)?;
            // let end = buffer.offset_of_position(&item.end)?;
            // log::debug!("{start}-{end} {item:?}");
            let iv = Interval::new(start, end);
            let item = if folded_range.find_by_interval(iv) {
                FoldingRange {
                    status: FoldingRangeStatus::Fold,
                }
            } else {
                FoldingRange {
                    status: FoldingRangeStatus::Unfold,
                }
            };
            let data = cx.create_rw_signal(item);
            builder.add_span(iv, data);
        }
        self.0 = builder.build();
        Ok(())
    }

    pub fn update_folding_item(&mut self, item: FoldingDisplayItem) {
        match item.ty {
            FoldingDisplayType::UnfoldStart | FoldingDisplayType::Folded => {
                self.0.iter().find_map(|range| {
                    if range.0 == item.iv {
                        range.1.update(|x| {
                            x.status.click();
                        });
                        Some(())
                    } else {
                        None
                    }
                });
            },
            FoldingDisplayType::UnfoldEnd => {
                self.0.iter().find_map(|range| {
                    if range.0 == item.iv {
                        range.1.update(|x| {
                            x.status.click();
                        });
                        Some(())
                    } else {
                        None
                    }
                });
            },
        }
    }

    pub fn update_by_phantom(&mut self, position: usize) {
        self.0.iter().find_map(|range| {
            if range.0.start == position {
                range.1.update(|x| x.status.click());
                Some(())
            } else {
                None
            }
        });
    }
}

impl FoldedRanges {
    pub fn folded_line_count(&self) -> usize {
        self.0.iter().fold(0usize, |count, item| {
            count + item.end_line - item.start_line
        })
    }

    pub fn find_by_interval(&self, iv: Interval) -> bool {
        self.0.iter().any(|item| item.interval == iv)
    }

    pub fn filter_by_line(&self, line: usize) -> Self {
        Self(
            self.0
                .iter()
                .filter_map(|item| {
                    if item.start_line <= line && item.end_line >= line {
                        Some(item.clone())
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    pub fn visual_line(&self, line: usize) -> usize {
        for folded in &self.0 {
            if line <= folded.start_line {
                return line;
            } else if folded.start_line < line && line <= folded.end_line {
                return folded.start_line;
            }
        }
        line
    }

    /// ??line: 该行是否被折叠。
    /// start_index: 下次检查的起始点
    pub fn contain_line(&self, start_index: usize, line: usize) -> (bool, usize) {
        if start_index >= self.0.len() {
            return (false, start_index);
        }
        let mut last_index = start_index;
        for range in self.0[start_index..].iter() {
            if range.start_line >= line {
                return (false, last_index);
                // todo range.end.line >= line
            } else if range.start_line < line && range.end_line >= line {
                return (true, last_index);
            } else if range.end_line < line {
                last_index += 1;
            }
        }
        (false, last_index)
    }

    // pub fn contain_position(&self, position: Position) -> bool {
    //     self.0
    //         .iter()
    //         .any(|x| x.start <= position && x.end >= position)
    // }

    // pub fn update_status(&self, folding: &mut FoldingRange) {
    //     if self
    //         .0
    //         .iter()
    //         .any(|x| x.start == folding.start && x.end == folding.end)
    //     {
    //         folding.status = FoldingRangeStatus::Fold
    //     }
    // }

    pub fn into_phantom_text(
        self,
        buffer: &Buffer,
        // config: &LapceConfig,
        line: usize,
        inlay_hint_font_size: usize,
        inlay_hint_foreground: Color,
        inlay_hint_background: Color,
    ) -> Vec<PhantomText> {
        self.0
            .into_iter()
            .filter_map(|x| {
                match x.into_phantom_text(
                    buffer,
                    line,
                    inlay_hint_font_size,
                    inlay_hint_foreground,
                    inlay_hint_background,
                ) {
                    Ok(rs) => rs,
                    Err(err) => {
                        error!("{err}");
                        None
                    },
                }
            })
            .collect()
    }
}

// fn get_offset(buffer: &Buffer, positon: Position) -> Result<usize> {
//     Ok(buffer.offset_of_line(positon.line as usize)? + positon.character as
// usize) }

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FoldedRange {
    pub interval:   Interval,
    pub start_line: usize,
    pub end_line:   usize,
}

impl FoldedRange {
    pub fn into_phantom_text(
        self,
        buffer: &Buffer,
        // config: &LapceConfig,
        line: usize,
        inlay_hint_font_size: usize,
        inlay_hint_foreground: Color,
        inlay_hint_background: Color,
    ) -> Result<Option<PhantomText>> {
        // info!("line={line} start={:?} end={:?}", self.start,
        // self.end);
        let same_line = self.end_line == self.start_line;
        let folded = buffer.offset_of_line(self.end_line)?;
        let current = buffer.offset_of_line(self.start_line)?;
        let content = buffer.line_content(self.start_line)?.len();

        Ok(if self.start_line == line {
            let Some(start_char) = buffer.char_at_offset(self.interval.start) else {
                return Ok(None);
            };
            let Some(end_char) = buffer.char_at_offset(self.interval.end - 1) else {
                return Ok(None);
            };
            let mut text = String::new();
            text.push(start_char);
            text.push_str("...");
            text.push(end_char);
            let next_line = if same_line { None } else { Some(self.end_line) };
            let start = self.interval.start - current;
            let (all_len, len) = if same_line {
                (self.interval.size(), self.interval.size())
            } else {
                (folded - self.interval.start, content - start)
            };
            Some(PhantomText {
                kind: PhantomTextKind::LineFoldedRang {
                    next_line,
                    len,
                    all_len,
                    start_position: self.interval.start,
                },
                col: start,
                text,
                fg: Some(inlay_hint_foreground),
                font_size: Some(inlay_hint_font_size),
                bg: Some(inlay_hint_background),
                under_line: None,
                final_col: start,
                line,
                visual_merge_col: start,
                origin_merge_col: start,
            })
        } else if self.end_line == line && !same_line {
            let text = String::new();
            let all_len = self.interval.end - folded;
            Some(PhantomText {
                kind: PhantomTextKind::LineFoldedRang {
                    next_line: None,
                    len: all_len,
                    all_len,
                    start_position: self.interval.start,
                },
                col: 0,
                text,
                fg: None,
                font_size: None,
                bg: None,
                under_line: None,
                final_col: 0,
                line,
                visual_merge_col: 0,
                origin_merge_col: 0,
            })
        } else {
            None
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldingRange {
    // pub start:  Position,
    // pub end:    Position,
    pub status: FoldingRangeStatus,
    // pub collapsed_text: Option<String>,
}

// impl FoldingRange {
//     pub fn from_lsp(value: lsp_types::FoldingRange) -> Self {
//         let lsp_types::FoldingRange {
//             start_line,
//             start_character,
//             end_line,
//             end_character,
//             ..
//         } = value;
//         let status = FoldingRangeStatus::Unfold;
//         Self {
//             start: Position {
//                 line:      start_line,
//                 character: start_character.unwrap_or_default(),
//             },
//             end: Position {
//                 line:      end_line,
//                 character: end_character.unwrap_or_default(),
//             },
//             status,
//             // collapsed_text,
//         }
//     }
// }

#[derive(Debug, Clone, Eq, PartialEq, Hash, Copy, Serialize, Deserialize)]
pub struct FoldingPosition {
    pub line:      u32,
    pub character: Option<u32>, // pub kind: Option<FoldingRangeKind>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FoldingRangeStatus {
    Fold,
    #[default]
    Unfold,
}

impl FoldingRangeStatus {
    pub fn click(&mut self) {
        match self {
            FoldingRangeStatus::Fold => {
                *self = FoldingRangeStatus::Unfold;
            },
            FoldingRangeStatus::Unfold => {
                *self = FoldingRangeStatus::Fold;
            },
        }
    }

    pub fn is_folded(&self) -> bool {
        *self == Self::Fold
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FoldingDisplayItem {
    // pub position: Position,
    pub iv: Interval,
    pub y:  i32,
    pub ty: FoldingDisplayType,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum FoldingDisplayType {
    UnfoldStart,
    Folded,
    UnfoldEnd,
}

// impl FoldingDisplayItem {
//     pub fn position(&self) -> FoldingPosition {
//         self.position
//     }
// }

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, Clone, Hash, Copy)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}

impl From<lsp_types::FoldingRangeKind> for FoldingRangeKind {
    fn from(value: lsp_types::FoldingRangeKind) -> Self {
        match value {
            lsp_types::FoldingRangeKind::Comment => FoldingRangeKind::Comment,
            lsp_types::FoldingRangeKind::Imports => FoldingRangeKind::Imports,
            lsp_types::FoldingRangeKind::Region => FoldingRangeKind::Region,
        }
    }
}

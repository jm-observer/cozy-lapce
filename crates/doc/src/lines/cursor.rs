use anyhow::Result;
use lapce_xi_rope::{RopeDelta, Transformer};
use log::{debug, error};
use serde::{Deserialize, Serialize};

use crate::lines::{
    buffer::{Buffer, rope_text::RopeText},
    mode::{Mode, MotionMode, VisualMode},
    register::RegisterData,
    selection::{InsertDrift, SelRegion, Selection},
};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum ColPosition {
    FirstNonBlank,
    Start,
    End,
    Col(usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    mode:                   CursorMode,
    pub horiz:              Option<ColPosition>,
    pub motion_mode:        Option<MotionMode>,
    pub history_selections: Vec<Selection>,
    pub affinity:           CursorAffinity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CursorMode {
    Normal(usize),
    Visual {
        start: usize,
        end:   usize,
        mode:  VisualMode,
    },
    /// 非vim模式下，默认
    Insert(Selection),
}

struct RegionsIter<'c> {
    cursor_mode: &'c CursorMode,
    idx:         usize,
}

impl Iterator for RegionsIter<'_> {
    type Item = (usize, usize, Option<CursorAffinity>, Option<CursorAffinity>);

    fn next(&mut self) -> Option<Self::Item> {
        match self.cursor_mode {
            &CursorMode::Normal(offset) => (self.idx == 0).then(|| {
                self.idx = 1;
                (offset, offset, None, None)
            }),
            &CursorMode::Visual { start, end, .. } => (self.idx == 0).then(|| {
                self.idx = 1;
                (start, end, None, None)
            }),
            CursorMode::Insert(selection) => {
                // log::info!("selection: {:?}", selection);
                let next = selection.regions().get(self.idx).map(
                    |&SelRegion {
                         start,
                         end,
                         start_cursor_affi,
                         end_cursor_affi,
                         ..
                     }| {
                        (start, end, start_cursor_affi, end_cursor_affi)
                    },
                );

                if next.is_some() {
                    self.idx += 1;
                }

                next
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let total_len = match self.cursor_mode {
            CursorMode::Normal(_) | CursorMode::Visual { .. } => 1,
            CursorMode::Insert(selection) => selection.len(),
        };
        let len = total_len - self.idx;

        (len, Some(len))
    }
}

impl ExactSizeIterator for RegionsIter<'_> {}

impl CursorMode {
    pub fn simply_mode(&self) -> Mode {
        match &self {
            CursorMode::Normal(_) => Mode::Normal,
            CursorMode::Visual { mode, .. } => Mode::Visual(*mode),
            CursorMode::Insert(_) => Mode::Insert,
        }
    }

    pub fn is_insert(&self) -> bool {
        matches!(self, CursorMode::Insert(_))
    }

    pub fn offset(&self) -> usize {
        match &self {
            CursorMode::Normal(offset) => *offset,
            CursorMode::Visual { end, .. } => *end,
            CursorMode::Insert(selection) => selection.get_cursor_offset(),
        }
    }

    pub fn start_offset(&self) -> usize {
        match &self {
            CursorMode::Normal(offset) => *offset,
            CursorMode::Visual { start, .. } => *start,
            CursorMode::Insert(selection) => {
                selection.first().map(|s| s.start).unwrap_or(0)
            },
        }
    }

    pub fn regions_iter(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (usize, usize, Option<CursorAffinity>, Option<CursorAffinity>),
    > + '_ {
        RegionsIter {
            cursor_mode: self,
            idx:         0,
        }
    }
}

/// Decides how the cursor should be placed around special areas of
/// text. Ex:
/// ```rust,ignore
/// let j =            // soft linewrap
/// 1 + 2 + 3;
/// ```
/// where `let j = ` has the issue that there's two positions you
/// might want your cursor to be: `let j = |` or `|1 + 2 + 3;`  
/// These are the same offset in the text, but it feels more natural
/// to have it move in a certain way.  
/// If you're at `let j =| ` and you press the right-arrow key, then
/// it uses your backwards affinity to keep you on the line at `let j
/// = |`. If you're at `1| + 2 + 3;` and you press the left-arrow key,
/// then it uses your forwards affinity to keep you on the line at `|1
/// + 2 + 3;`.
///
/// For other special text, like inlay hints, this can also apply.  
/// ```rust,ignore
/// let j<: String> = ...
/// ```
/// where `<: String>` is our inlay hint, then  
/// `let |j<: String> =` and you press the right-arrow key, then it
/// uses your backwards affinity to keep you on the same side of the
/// hint, `let j|<: String>`. `let j<: String> |=` and you press the
/// right-arrow key, then it uses your forwards affinity to
/// keep you on the same side of the hint, `let j<: String>| =`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CursorAffinity {
    /// `<: String>|`
    Forward,
    /// `|<: String>`
    #[default]
    Backward,
}
impl CursorAffinity {
    pub fn invert(&self) -> Self {
        match self {
            CursorAffinity::Forward => CursorAffinity::Backward,
            CursorAffinity::Backward => CursorAffinity::Forward,
        }
    }

    pub fn forward(&self) -> bool {
        match self {
            CursorAffinity::Forward => true,
            CursorAffinity::Backward => false,
        }
    }
}

impl Cursor {
    pub fn new(
        mode: CursorMode,
        horiz: Option<ColPosition>,
        motion_mode: Option<MotionMode>,
    ) -> Self {
        Self {
            mode,
            horiz,
            motion_mode,
            history_selections: Vec::new(),
            // It should appear before any inlay hints at the very
            // first position
            affinity: CursorAffinity::Backward,
        }
    }

    pub fn is_block(&self) -> bool {
        match self.mode {
            CursorMode::Normal(_) | CursorMode::Visual { .. } => true,
            CursorMode::Insert(_) => false,
        }
    }

    pub fn origin(modal: bool) -> Self {
        Self::new(
            if modal {
                CursorMode::Normal(0)
            } else {
                CursorMode::Insert(Selection::caret(0))
            },
            None,
            None,
        )
    }

    pub fn offset(&self) -> usize {
        self.mode.offset()
    }

    pub fn start_offset(&self) -> usize {
        self.mode.start_offset()
    }

    pub fn regions_iter(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (usize, usize, Option<CursorAffinity>, Option<CursorAffinity>),
    > + '_ {
        self.mode.regions_iter()
    }

    pub fn is_normal(&self) -> bool {
        matches!(&self.mode, CursorMode::Normal(_))
    }

    pub fn is_insert(&self) -> bool {
        matches!(&self.mode, CursorMode::Insert(_))
    }

    pub fn is_visual(&self) -> bool {
        matches!(&self.mode, CursorMode::Visual { .. })
    }

    pub fn mode(&self) -> &CursorMode {
        &self.mode
    }

    pub fn mut_mode(&mut self) -> &mut CursorMode {
        &mut self.mode
    }

    pub fn set_mode(&mut self, mode: CursorMode) {
        if let CursorMode::Insert(selection) = &self.mode {
            self.history_selections.push(selection.clone());
        }
        self.mode = mode;
    }

    pub fn set_insert(&mut self, selection: Selection) {
        debug!("set_insert {selection:?}");
        self.set_mode(CursorMode::Insert(selection));
    }

    pub fn update_selection(&mut self, buffer: &Buffer, selection: Selection) {
        match self.mode {
            CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                let offset = selection.min_offset();
                let offset = match buffer.offset_line_end(offset, false) {
                    Ok(rs) => rs.min(offset),
                    Err(err) => {
                        error!("{err:?}");
                        return;
                    },
                };
                self.mode = CursorMode::Normal(offset);
            },
            CursorMode::Insert(_) => {
                self.set_insert(selection);
                // self.mode = CursorMode::Insert(selection);
            },
        }
    }

    pub fn edit_selection(&self, text: &impl RopeText) -> Result<Selection> {
        Ok(match &self.mode {
            CursorMode::Insert(selection) => selection.clone(),
            CursorMode::Normal(offset) => Selection::region(
                *offset,
                text.next_grapheme_offset(*offset, 1, text.len()),
            ),
            CursorMode::Visual { start, end, mode } => match mode {
                VisualMode::Normal => Selection::region(
                    *start.min(end),
                    text.next_grapheme_offset(*start.max(end), 1, text.len()),
                ),
                VisualMode::Linewise => {
                    let start_offset =
                        text.offset_of_line(text.line_of_offset(*start.min(end)))?;
                    let end_offset = text
                        .offset_of_line(text.line_of_offset(*start.max(end)) + 1)?;
                    Selection::region(start_offset, end_offset)
                },
                VisualMode::Blockwise => {
                    let mut selection = Selection::new();
                    let (start_line, start_col) =
                        text.offset_to_line_col(*start.min(end))?;
                    let (end_line, end_col) =
                        text.offset_to_line_col(*start.max(end))?;
                    let left = start_col.min(end_col);
                    let right = start_col.max(end_col) + 1;
                    for line in start_line..end_line + 1 {
                        let max_col = text.line_end_col(line, true)?;
                        if left > max_col {
                            continue;
                        }
                        let right = match &self.horiz {
                            Some(ColPosition::End) => max_col,
                            _ => {
                                if right > max_col {
                                    max_col
                                } else {
                                    right
                                }
                            },
                        };
                        let left = text.offset_of_line_col(line, left)?;
                        let right = text.offset_of_line_col(line, right)?;
                        selection.add_region(SelRegion::new(left, right, None));
                    }
                    selection
                },
            },
        })
    }

    pub fn apply_delta(&mut self, delta: &RopeDelta) {
        match &self.mode {
            CursorMode::Normal(offset) => {
                let mut transformer = Transformer::new(delta);
                let new_offset = transformer.transform(*offset, true);
                self.mode = CursorMode::Normal(new_offset);
            },
            CursorMode::Visual { start, end, mode } => {
                let mut transformer = Transformer::new(delta);
                let start = transformer.transform(*start, false);
                let end = transformer.transform(*end, true);
                self.mode = CursorMode::Visual {
                    start,
                    end,
                    mode: *mode,
                };
            },
            CursorMode::Insert(selection) => {
                let selection =
                    selection.apply_delta(delta, true, InsertDrift::Default);
                self.set_insert(selection);
                // self.mode = CursorMode::Insert(selection);
            },
        }
        self.horiz = None;
    }

    pub fn yank(&self, text: &impl RopeText) -> Result<RegisterData> {
        let (content, mode) = match &self.mode {
            CursorMode::Insert(selection) => {
                let mut mode = VisualMode::Normal;
                let mut content = "".to_string();
                for region in selection.regions() {
                    let region_content = if region.is_caret() {
                        mode = VisualMode::Linewise;
                        let line = text.line_of_offset(region.start);
                        text.line_content(line)?
                    } else {
                        text.slice_to_cow(region.min()..region.max())
                    };
                    if content.is_empty() {
                        content = region_content.to_string();
                    } else if content.ends_with('\n') {
                        content += &region_content;
                    } else {
                        content += "\n";
                        content += &region_content;
                    }
                }
                (content, mode)
            },
            CursorMode::Normal(offset) => {
                let new_offset = text.next_grapheme_offset(*offset, 1, text.len());
                (
                    text.slice_to_cow(*offset..new_offset).to_string(),
                    VisualMode::Normal,
                )
            },
            CursorMode::Visual { start, end, mode } => match mode {
                VisualMode::Normal => (
                    text.slice_to_cow(
                        *start.min(end)
                            ..text.next_grapheme_offset(
                                *start.max(end),
                                1,
                                text.len(),
                            ),
                    )
                    .to_string(),
                    VisualMode::Normal,
                ),
                VisualMode::Linewise => {
                    let start_offset =
                        text.offset_of_line(text.line_of_offset(*start.min(end)))?;
                    let end_offset = text
                        .offset_of_line(text.line_of_offset(*start.max(end)) + 1)?;
                    (
                        text.slice_to_cow(start_offset..end_offset).to_string(),
                        VisualMode::Linewise,
                    )
                },
                VisualMode::Blockwise => {
                    let mut lines = Vec::new();
                    let (start_line, start_col) =
                        text.offset_to_line_col(*start.min(end))?;
                    let (end_line, end_col) =
                        text.offset_to_line_col(*start.max(end))?;
                    let left = start_col.min(end_col);
                    let right = start_col.max(end_col) + 1;
                    for line in start_line..end_line + 1 {
                        let max_col = text.line_end_col(line, true)?;
                        if left > max_col {
                            lines.push("".to_string());
                        } else {
                            let right = match &self.horiz {
                                Some(ColPosition::End) => max_col,
                                _ => {
                                    if right > max_col {
                                        max_col
                                    } else {
                                        right
                                    }
                                },
                            };
                            let left = text.offset_of_line_col(line, left)?;
                            let right = text.offset_of_line_col(line, right)?;
                            lines.push(text.slice_to_cow(left..right).to_string());
                        }
                    }
                    (lines.join("\n") + "\n", VisualMode::Blockwise)
                },
            },
        };
        Ok(RegisterData { content, mode })
    }

    /// Return the current selection start and end position for a
    /// Single cursor selection
    pub fn get_selection(&self) -> Option<(usize, usize)> {
        match &self.mode {
            CursorMode::Visual {
                start,
                end,
                mode: _,
            } => Some((*start, *end)),
            CursorMode::Insert(selection) => selection
                .regions()
                .first()
                .map(|region| (region.start, region.end)),
            _ => None,
        }
    }

    pub fn get_line_col_char(
        &self,
        buffer: &Buffer,
    ) -> Result<Option<(usize, usize, usize)>> {
        Ok(match &self.mode {
            CursorMode::Normal(offset) => {
                let ln_col = buffer.offset_to_line_col(*offset)?;
                Some((ln_col.0, ln_col.1, *offset))
            },
            CursorMode::Visual {
                start,
                end,
                mode: _,
            } => {
                let v = buffer.offset_to_line_col(*start.min(end))?;
                Some((v.0, v.1, *start))
            },
            CursorMode::Insert(selection) => {
                if selection.regions().len() > 1 {
                    return Ok(None);
                }

                let x = selection.regions().first().unwrap();
                let v = buffer.offset_to_line_col(x.start)?;

                Some((v.0, v.1, x.start))
            },
        })
    }

    pub fn get_selection_count(&self) -> usize {
        match &self.mode {
            CursorMode::Insert(selection) => selection.regions().len(),
            _ => 0,
        }
    }

    pub fn set_offset(&mut self, offset: usize, modify: bool, new_cursor: bool) {
        self.set_offset_with_affinity(offset, modify, new_cursor, None);
    }

    pub fn set_offset_with_affinity(
        &mut self,
        offset: usize,
        modify: bool,
        new_cursor: bool,
        new_affinite: Option<CursorAffinity>,
    ) {
        // log::warn!(
        //     "cursor set_offset new_offset={offset} modify={modify} \
        //      new_cursor={new_cursor} old_offset={} new_affinite={new_affinite:?}",
        //     self.offset()
        // );

        if let Some(new_affinite) = new_affinite {
            self.affinity = new_affinite;
        }
        match &self.mode {
            CursorMode::Normal(_old_offset) => {
                // todo!()
                // if modify && *old_offset != offset {
                //     self.mode = CursorMode::Visual {
                //         start: *old_offset,
                //         end:   offset,
                //         mode:  VisualMode::Normal
                //     };
                // } else {
                //     self.mode = CursorMode::Normal(offset);
                // }
            },
            CursorMode::Visual {
                start: _,
                end: _,
                mode: _,
            } => {
                // todo
                // if modify {
                //     self.mode = CursorMode::Visual {
                //         start: *start,
                //         end:   offset,
                //         mode:  VisualMode::Normal
                //     };
                // } else {
                //     self.mode = CursorMode::Normal(offset);
                // }
            },
            CursorMode::Insert(selection) => {
                if new_cursor {
                    // todo
                    // let mut new_selection = selection.clone();
                    // if modify {
                    //     if let Some(region) =
                    // new_selection.last_inserted_mut() {
                    //         region.end = offset;
                    //     } else {
                    //         new_selection.
                    // add_region(SelRegion::caret(offset));
                    //     }
                    //     self.set_insert(new_selection);
                    // } else {
                    //     let mut new_selection = selection.clone();
                    //     new_selection.add_region(SelRegion::caret(offset));
                    //     self.set_insert(new_selection);
                    // }
                } else if modify {
                    let mut new_selection = Selection::new();
                    if let Some(region) = selection.first() {
                        let mut new_region =
                            SelRegion::new(region.start, offset, None);
                        new_region.start_cursor_affi = region.start_cursor_affi;
                        new_region.end_cursor_affi = new_affinite;
                        new_selection.add_region(new_region);
                    } else {
                        new_selection
                            .add_region(SelRegion::new(offset, offset, None));
                    }
                    self.set_insert(new_selection);
                } else {
                    let mut new_selection = Selection::new();
                    let mut new_region = SelRegion::new(offset, offset, None);
                    new_region.start_cursor_affi = new_affinite;
                    new_region.end_cursor_affi = new_affinite;
                    new_selection.add_region(new_region);
                    self.set_insert(new_selection);
                }
            },
        }
    }

    pub fn add_region(
        &mut self,
        start: usize,
        end: usize,
        modify: bool,
        new_cursor: bool,
        start_affinity: Option<CursorAffinity>,
    ) {
        match &self.mode {
            CursorMode::Normal(_offset) => {
                self.mode = CursorMode::Visual {
                    start,
                    end: end - 1,
                    mode: VisualMode::Normal,
                };
            },
            CursorMode::Visual {
                start: old_start,
                end: old_end,
                mode: _,
            } => {
                let forward = old_end >= old_start;
                let new_start = (*old_start).min(*old_end).min(start).min(end - 1);
                let new_end = (*old_start).max(*old_end).max(start).max(end - 1);
                let (new_start, new_end) = if forward {
                    (new_start, new_end)
                } else {
                    (new_end, new_start)
                };
                self.mode = CursorMode::Visual {
                    start: new_start,
                    end:   new_end,
                    mode:  VisualMode::Normal,
                };
            },
            CursorMode::Insert(selection) => {
                let new_selection = if new_cursor {
                    let mut new_selection = selection.clone();
                    if modify {
                        let mut new_region =
                            if let Some(last_inserted) = selection.last_inserted() {
                                last_inserted
                                    .merge_with(SelRegion::new(start, end, None))
                            } else {
                                SelRegion::new(start, end, None)
                            };
                        new_region.start_cursor_affi = start_affinity;
                        new_selection.replace_last_inserted_region(new_region);
                    } else {
                        let mut new_region = SelRegion::new(start, end, None);
                        new_region.start_cursor_affi = start_affinity;
                        new_selection.add_region(new_region);
                    }
                    new_selection
                } else if modify {
                    let mut new_selection = selection.clone();
                    let mut new_region = SelRegion::new(start, end, None);
                    new_region.start_cursor_affi = start_affinity;
                    new_selection.add_region(new_region);
                    new_selection
                } else {
                    let mut new_region = SelRegion::new(start, end, None);
                    new_region.start_cursor_affi = start_affinity;
                    Selection::sel_region(new_region)
                };
                self.set_insert(new_selection);
                // self.mode = CursorMode::Insert(new_selection);
            },
        }
    }
}

pub fn get_first_selection_after(
    cursor: &Cursor,
    buffer: &Buffer,
    delta: &RopeDelta,
) -> Option<Cursor> {
    let mut transformer = Transformer::new(delta);

    let offset = cursor.offset();
    let offset = transformer.transform(offset, false);
    let (ins, del) = delta.clone().factor();
    let ins = ins.transform_shrink(&del);
    for el in ins.els.iter() {
        match el {
            lapce_xi_rope::DeltaElement::Copy(b, e) => {
                // if b == e, ins.inserted_subset() will panic
                if b == e {
                    return None;
                }
            },
            lapce_xi_rope::DeltaElement::Insert(_) => {},
        }
    }

    // TODO it's silly to store the whole thing in memory, we only
    // need the first element.
    let mut positions = ins
        .inserted_subset()
        .complement_iter()
        .map(|s| s.1)
        .collect::<Vec<usize>>();
    positions.append(
        &mut del
            .complement_iter()
            .map(|s| transformer.transform(s.1, false))
            .collect::<Vec<usize>>(),
    );
    positions.sort_by_key(|p| {
        let p = *p as i32 - offset as i32;
        if p > 0 { p as usize } else { -p as usize }
    });

    positions
        .first()
        .cloned()
        .map(Selection::caret)
        .and_then(|selection| {
            let cursor_mode = match cursor.mode {
                CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                    let offset = selection.min_offset();
                    let offset = match buffer.offset_line_end(offset, false) {
                        Ok(rs) => rs.min(offset),
                        Err(err) => {
                            error!("{err:?}");
                            return None;
                        },
                    };
                    CursorMode::Normal(offset)
                },
                CursorMode::Insert(_) => CursorMode::Insert(selection),
            };

            Some(Cursor::new(cursor_mode, None, None))
        })
}

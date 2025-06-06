use std::{collections::HashSet, iter, ops::Range};

use anyhow::Result;
use itertools::Itertools;
use lapce_xi_rope::{DeltaElement, Rope, RopeDelta};
use log::{error, warn};

use super::screen_lines::ScreenLines;
use crate::lines::{
    buffer::{Buffer, InvalLines, rope_text::RopeText},
    command::EditCommand,
    cursor::{Cursor, CursorMode, get_first_selection_after},
    indent::{create_edit, create_outdent},
    mode::{Mode, MotionMode, VisualMode},
    register::{Clipboard, Register, RegisterData, RegisterKind},
    selection::{InsertDrift, SelRegion, Selection},
    util::{
        has_unmatched_pair, matching_auto_arround, matching_char,
        matching_pair_direction, str_is_pair_left, str_matching_pair,
    },
    word::{CharClassification, get_char_property},
};

fn format_start_end(
    buffer: &Buffer,
    range: Range<usize>,
    is_vertical: bool,
    first_non_blank: bool,
    count: usize,
) -> Result<Range<usize>> {
    let start = range.start;
    let end = range.end;

    Ok(if is_vertical {
        let start_line = buffer.line_of_offset(start.min(end));
        let end_line = buffer.line_of_offset(end.max(start));
        let start = if first_non_blank {
            buffer.first_non_blank_character_on_line(start_line)
        } else {
            buffer.offset_of_line(start_line)
        }?;
        let end = buffer.offset_of_line(end_line + count)?;
        start..end
    } else {
        let s = start.min(end);
        let e = start.max(end);
        s..e
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditType {
    InsertChars,
    Delete,
    DeleteSelection,
    InsertNewline,
    Cut,
    Paste,
    Indent,
    Outdent,
    ToggleComment,
    MoveLine,
    Completion,
    DeleteWord,
    DeleteToBeginningOfLine,
    DeleteToEndOfLine,
    DeleteToEndOfLineAndInsert,
    MotionDelete,
    NormalizeLineEndings,
    Undo,
    Redo,
    Other,
}

impl EditType {
    /// Checks whether a new undo group should be created between two
    /// edits.
    pub fn breaks_undo_group(self, previous: EditType) -> bool {
        !((self == EditType::InsertChars || self == EditType::Delete)
            && self == previous)
    }
}

pub struct EditConf<'a> {
    pub comment_token: &'a str,
    pub modal:         bool,
    pub smart_tab:     bool,
    pub keep_indent:   bool,
    pub auto_indent:   bool,
}

pub struct Action {}

impl Action {
    pub fn insert(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        s: &str,
        prev_unmatched: &dyn Fn(&Buffer, char, usize) -> Option<usize>,
        auto_closing_matching_pairs: bool,
        auto_surround: bool,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        let mut deltas = Vec::new();
        if let CursorMode::Insert(selection) = cursor.mode() {
            if s.chars().count() != 1 {
                let (text, delta, inval_lines) =
                    buffer.edit([(selection, s)], EditType::InsertChars);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                deltas.push((text, delta, inval_lines));
                cursor.set_mode(CursorMode::Insert(selection));
            } else {
                let c = s.chars().next().unwrap();
                let matching_pair_type = matching_pair_direction(c);

                // The main edit operations
                let mut edits = vec![];

                // "Late edits" - characters to be inserted after
                // particular regions
                let mut edits_after = vec![];

                let mut selection = selection.clone();
                for (idx, region) in selection.regions_mut().iter_mut().enumerate() {
                    let offset = region.end;
                    let cursor_char = buffer.char_at_offset(offset);
                    let Ok(prev_offset) = buffer.move_left(offset, Mode::Normal, 1)
                    else {
                        error!("{offset}");
                        continue;
                    };
                    let prev_cursor_char = if prev_offset < offset {
                        buffer.char_at_offset(prev_offset)
                    } else {
                        None
                    };

                    // when text is selected, and [,{,(,'," is
                    // inserted wrap the text with
                    // that char and its corresponding closing pair
                    if region.start != region.end
                        && auto_surround
                        && matching_auto_arround(c)
                    {
                        edits.push((
                            Selection::region(region.min(), region.min()),
                            c.to_string(),
                        ));
                        edits_after.push((idx, matching_char(c).unwrap()));
                        continue;
                    }

                    if auto_closing_matching_pairs {
                        if (c == '"' || c == '\'' || c == '`')
                            && cursor_char == Some(c)
                        {
                            // Skip the closing character
                            let new_offset =
                                buffer.next_grapheme_offset(offset, 1, buffer.len());

                            *region = SelRegion::caret(new_offset);
                            continue;
                        }

                        if matching_pair_type == Some(false) {
                            if cursor_char == Some(c) {
                                // Skip the closing character
                                let new_offset = buffer.next_grapheme_offset(
                                    offset,
                                    1,
                                    buffer.len(),
                                );

                                *region = SelRegion::caret(new_offset);
                                continue;
                            }

                            let line = buffer.line_of_offset(offset);
                            let Ok(line_start) = buffer.offset_of_line(line) else {
                                error!("{line}");
                                continue;
                            };
                            if buffer.slice_to_cow(line_start..offset).trim() == "" {
                                let opening_character = matching_char(c).unwrap();
                                if let Some(previous_offset) =
                                    prev_unmatched(buffer, opening_character, offset)
                                {
                                    // Auto-indent closing character
                                    // to the same level as the
                                    // opening.
                                    let previous_line =
                                        buffer.line_of_offset(previous_offset);
                                    let Ok(line_indent) =
                                        buffer.indent_on_line(previous_line)
                                    else {
                                        error!("{previous_line}");
                                        continue;
                                    };

                                    let current_selection =
                                        Selection::region(line_start, offset);

                                    edits.push((
                                        current_selection,
                                        format!("{line_indent}{c}"),
                                    ));
                                    continue;
                                }
                            }
                        }

                        if matching_pair_type == Some(true)
                            || c == '"'
                            || c == '\''
                            || c == '`'
                        {
                            // Create a late edit to insert the
                            // closing pair, if allowed.
                            let is_whitespace_or_punct = cursor_char
                                .map(|c| {
                                    let prop = get_char_property(c);
                                    prop == CharClassification::Lf
                                        || prop == CharClassification::Cr
                                        || prop == CharClassification::Space
                                        || prop == CharClassification::Punctuation
                                })
                                .unwrap_or(true);

                            let should_insert_pair = match c {
                                '"' | '\'' | '`' => {
                                    is_whitespace_or_punct
                                        && prev_cursor_char
                                            .map(|c| {
                                                let prop = get_char_property(c);
                                                prop == CharClassification::Lf
                                                    || prop == CharClassification::Cr
                                                    || prop == CharClassification::Space
                                                    || prop == CharClassification::Punctuation
                                            })
                                            .unwrap_or(true)
                                }
                                _ => is_whitespace_or_punct,
                            };

                            if should_insert_pair {
                                let insert_after = match c {
                                    '"' => '"',
                                    '\'' => '\'',
                                    '`' => '`',
                                    _ => matching_char(c).unwrap(),
                                };
                                edits_after.push((idx, insert_after));
                            }
                        };
                    }

                    let current_selection =
                        Selection::region(region.start, region.end);

                    edits.push((current_selection, c.to_string()));
                }

                // Apply edits to current selection
                let edits = edits
                    .iter()
                    .map(|(selection, content)| (selection, content.as_str()))
                    .collect::<Vec<_>>();

                let (text, delta, inval_lines) =
                    buffer.edit(&edits, EditType::InsertChars);

                buffer.set_cursor_before(CursorMode::Insert(selection.clone()));

                // Update selection
                let mut selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);

                buffer.set_cursor_after(CursorMode::Insert(selection.clone()));

                deltas.push((text, delta, inval_lines));
                // Apply late edits
                let edits_after = edits_after
                    .iter()
                    .map(|(idx, content)| {
                        let region = &selection.regions()[*idx];
                        (
                            Selection::region(region.max(), region.max()),
                            content.to_string(),
                        )
                    })
                    .collect::<Vec<_>>();

                let edits_after = edits_after
                    .iter()
                    .map(|(selection, content)| (selection, content.as_str()))
                    .collect::<Vec<_>>();

                if !edits_after.is_empty() {
                    let (text, delta, inval_lines) =
                        buffer.edit(&edits_after, EditType::InsertChars);
                    deltas.push((text, delta, inval_lines));
                }

                // Adjust selection according to previous late edits
                let mut adjustment = 0;
                for region in selection.regions_mut().iter_mut().sorted_by(
                    |region_a, region_b| region_a.start.cmp(&region_b.start),
                ) {
                    let new_region = SelRegion::new(
                        region.start + adjustment,
                        region.end + adjustment,
                        None,
                    );

                    if let Some(inserted) =
                        edits_after.iter().find_map(|(selection, str)| {
                            if selection.last_inserted().map(|r| r.start)
                                == Some(region.start)
                            {
                                Some(str)
                            } else {
                                None
                            }
                        })
                    {
                        adjustment += inserted.len();
                    }

                    *region = new_region;
                }

                cursor.set_mode(CursorMode::Insert(selection));
            }
        }
        deltas
    }

    fn toggle_visual(cursor: &mut Cursor, visual_mode: VisualMode, modal: bool) {
        if !modal {
            return;
        }

        match &cursor.mode() {
            CursorMode::Visual { start, end, mode } => {
                if mode != &visual_mode {
                    cursor.set_mode(CursorMode::Visual {
                        start: *start,
                        end:   *end,
                        mode:  visual_mode,
                    });
                } else {
                    cursor.set_mode(CursorMode::Normal(*end));
                };
            },
            _ => {
                let offset = cursor.offset();
                cursor.set_mode(CursorMode::Visual {
                    start: offset,
                    end:   offset,
                    mode:  visual_mode,
                });
            },
        }
    }

    fn insert_new_line(
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        selection: Selection,
        keep_indent: bool,
        auto_indent: bool,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        let mut edits = Vec::with_capacity(selection.regions().len());
        let mut extra_edits = Vec::new();
        let mut shift = 0i32;
        let line_ending = buffer.line_ending().get_chars();
        for region in selection.regions() {
            let offset = region.max();
            let line = buffer.line_of_offset(offset);
            let Ok(line_start) = buffer.offset_of_line(line) else {
                error!("{line}");
                continue;
            };
            let Ok(line_end) = buffer.line_end_offset(line, true) else {
                error!("{line}");
                continue;
            };
            let Ok(line_indent) = buffer.indent_on_line(line) else {
                error!("{line}");
                continue;
            };
            let first_half = buffer.slice_to_cow(line_start..offset);
            let second_half = buffer.slice_to_cow(offset..line_end);
            let second_half_trim = second_half.trim();

            // TODO: this could be done with 1 string
            let new_line_content = {
                let indent_storage;
                let indent = if auto_indent && has_unmatched_pair(&first_half) {
                    indent_storage =
                        format!("{}{}", line_indent, buffer.indent_unit());
                    &indent_storage
                } else if keep_indent
                    && (second_half_trim.is_empty()
                        || !second_half.starts_with(&line_indent))
                {
                    &line_indent
                } else {
                    indent_storage = String::new();
                    &indent_storage
                };
                format!("{line_ending}{indent}")
            };

            let selection = Selection::region(region.min(), region.max());

            shift -= (region.max() - region.min()) as i32;
            shift += new_line_content.len() as i32;

            edits.push((selection, new_line_content));

            if let Some(c) = first_half.chars().rev().find(|&c| c != ' ') {
                if let Some(true) = matching_pair_direction(c) {
                    if let Some(c) = matching_char(c) {
                        if second_half_trim.starts_with(c) {
                            let selection = Selection::caret(
                                (region.max() as i32 + shift) as usize,
                            );
                            let content = format!("{line_ending}{line_indent}",);
                            extra_edits.push((selection, content));
                        }
                    }
                }
            }
        }

        let edits = edits
            .iter()
            .map(|(selection, s)| (selection, s.as_str()))
            .collect::<Vec<_>>();
        let (text, delta, inval_lines) =
            buffer.edit(&edits, EditType::InsertNewline);
        let mut selection =
            selection.apply_delta(&delta, true, InsertDrift::Default);

        let mut deltas = vec![(text, delta, inval_lines)];

        if !extra_edits.is_empty() {
            let edits = extra_edits
                .iter()
                .map(|(selection, s)| (selection, s.as_str()))
                .collect::<Vec<_>>();
            let (text, delta, inval_lines) =
                buffer.edit(&edits, EditType::InsertNewline);
            selection = selection.apply_delta(&delta, false, InsertDrift::Default);
            deltas.push((text, delta, inval_lines));
        }

        cursor.set_mode(CursorMode::Insert(selection));

        deltas
    }

    pub fn execute_motion_mode(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        motion_mode: MotionMode,
        range: Range<usize>,
        is_vertical: bool,
        register: &mut Register,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        let mut deltas = Vec::new();
        match motion_mode {
            MotionMode::Delete { .. } => {
                let Ok(range) =
                    format_start_end(buffer, range.clone(), is_vertical, false, 1)
                else {
                    error!("{range:?}");
                    return vec![];
                };
                register.add(
                    RegisterKind::Delete,
                    RegisterData {
                        content: buffer.slice_to_cow(range.clone()).to_string(),
                        mode:    if is_vertical {
                            VisualMode::Linewise
                        } else {
                            VisualMode::Normal
                        },
                    },
                );
                let selection = Selection::region(range.start, range.end);
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::MotionDelete);
                cursor.apply_delta(&delta);
                deltas.push((text, delta, inval_lines));
            },
            MotionMode::Yank { .. } => {
                let Ok(range) =
                    format_start_end(buffer, range.clone(), is_vertical, false, 1)
                else {
                    error!("{range:?}");
                    return vec![];
                };
                register.add(
                    RegisterKind::Yank,
                    RegisterData {
                        content: buffer.slice_to_cow(range).to_string(),
                        mode:    if is_vertical {
                            VisualMode::Linewise
                        } else {
                            VisualMode::Normal
                        },
                    },
                );
            },
            MotionMode::Indent => {
                let selection = Selection::region(range.start, range.end);
                let Ok((text, delta, inval_lines)) =
                    Self::do_indent(buffer, &selection)
                else {
                    error!("{selection:?}");
                    return vec![];
                };
                deltas.push((text, delta, inval_lines));
            },
            MotionMode::Outdent => {
                let selection = Selection::region(range.start, range.end);
                let Ok((text, delta, inval_lines)) =
                    Self::do_outdent(buffer, &selection)
                else {
                    error!("{selection:?}");
                    return vec![];
                };
                deltas.push((text, delta, inval_lines));
            },
        }
        deltas
    }

    /// Compute the result of pasting `content` into `selection`.
    /// If the number of lines to be pasted is divisible by the number
    /// of [`SelRegion`]s in `selection`, partition the content to
    /// be pasted into groups of equal numbers of lines and
    /// paste one group at each [`SelRegion`].
    /// The way lines are counted and `content` is partitioned depends
    /// on `mode`.
    fn compute_paste_edit(
        buffer: &mut Buffer,
        selection: &Selection,
        content: &str,
        mode: VisualMode,
    ) -> (Rope, RopeDelta, InvalLines) {
        if selection.len() > 1 {
            let line_ends: Vec<_> =
                content.match_indices('\n').map(|(idx, _)| idx).collect();

            match mode {
                // Consider lines to be separated by the line
                // terminator. The number of lines ==
                // number of line terminators + 1. The
                // final line in each group does not include the line
                // terminator.
                VisualMode::Normal
                    if (line_ends.len() + 1) % selection.len() == 0 =>
                {
                    let lines_per_group = (line_ends.len() + 1) / selection.len();
                    let mut start_idx = 0;
                    let last_line_start = line_ends
                        .len()
                        .checked_sub(lines_per_group)
                        .and_then(|line_idx| line_ends.get(line_idx))
                        .map(|line_end| line_end + 1)
                        .unwrap_or(0);

                    let groups = line_ends
                        .iter()
                        .skip(lines_per_group - 1)
                        .step_by(lines_per_group)
                        .map(|&end_idx| {
                            let group = &content[start_idx..end_idx];
                            let group = group.strip_suffix('\r').unwrap_or(group);
                            start_idx = end_idx + 1;

                            group
                        })
                        .chain(iter::once(&content[last_line_start..]));

                    let edits = selection
                        .regions()
                        .iter()
                        .copied()
                        .map(Selection::sel_region)
                        .zip(groups);

                    buffer.edit(edits, EditType::Paste)
                },
                // Consider lines to be terminated by the line
                // terminator. The number of lines ==
                // number of line terminators.
                // The final line in each group includes the line
                // terminator.
                VisualMode::Linewise | VisualMode::Blockwise
                    if line_ends.len() % selection.len() == 0 =>
                {
                    let lines_per_group = line_ends.len() / selection.len();
                    let mut start_idx = 0;

                    let groups = line_ends
                        .iter()
                        .skip(lines_per_group - 1)
                        .step_by(lines_per_group)
                        .map(|&end_idx| {
                            let group = &content[start_idx..=end_idx];
                            start_idx = end_idx + 1;

                            group
                        });

                    let edits = selection
                        .regions()
                        .iter()
                        .copied()
                        .map(Selection::sel_region)
                        .zip(groups);

                    buffer.edit(edits, EditType::Paste)
                },
                _ => buffer.edit([(&selection, content)], EditType::Paste),
            }
        } else {
            buffer.edit([(&selection, content)], EditType::Paste)
        }
    }

    pub fn do_paste(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        data: &RegisterData,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        match Self::_do_paste(cursor, buffer, data) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                vec![]
            },
        }
    }

    fn _do_paste(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        data: &RegisterData,
    ) -> Result<Vec<(Rope, RopeDelta, InvalLines)>> {
        let mut deltas = Vec::new();
        match data.mode {
            VisualMode::Normal => {
                let selection = match cursor.mode() {
                    CursorMode::Normal(offset) => {
                        let line_end = buffer.offset_line_end(*offset, true)?;
                        let offset = (offset + 1).min(line_end);
                        Selection::caret(offset)
                    },
                    CursorMode::Insert { .. } | CursorMode::Visual { .. } => {
                        cursor.edit_selection(buffer)?
                    },
                };
                let after = cursor.is_insert() || !data.content.contains('\n');
                let (text, delta, inval_lines) = Self::compute_paste_edit(
                    buffer,
                    &selection,
                    &data.content,
                    data.mode,
                );
                let selection =
                    selection.apply_delta(&delta, after, InsertDrift::Default);
                deltas.push((text, delta, inval_lines));
                if !after {
                    cursor.update_selection(buffer, selection);
                } else {
                    match cursor.mode() {
                        CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                            let offset = buffer.prev_grapheme_offset(
                                selection.min_offset(),
                                1,
                                0,
                            );
                            cursor.set_mode(CursorMode::Normal(offset));
                        },
                        CursorMode::Insert { .. } => {
                            cursor.set_mode(CursorMode::Insert(selection));
                        },
                    }
                }
            },
            VisualMode::Linewise | VisualMode::Blockwise => {
                let (selection, content) = match cursor.mode() {
                    CursorMode::Normal(offset) => {
                        let line = buffer.line_of_offset(*offset);
                        let offset = buffer.offset_of_line(line + 1)?;
                        (Selection::caret(offset), data.content.clone())
                    },
                    CursorMode::Insert(selection) => {
                        let mut selection = selection.clone();
                        for region in selection.regions_mut() {
                            if region.is_caret() {
                                let line = buffer.line_of_offset(region.start);
                                let start = buffer.offset_of_line(line)?;
                                region.start = start;
                                region.end = start;
                            }
                        }
                        (selection, data.content.clone())
                    },
                    CursorMode::Visual { mode, .. } => {
                        let selection = cursor.edit_selection(buffer)?;
                        let data = match mode {
                            VisualMode::Linewise => data.content.clone(),
                            _ => "\n".to_string() + &data.content,
                        };
                        (selection, data)
                    },
                };
                let (text, delta, inval_lines) = Self::compute_paste_edit(
                    buffer, &selection, &content, data.mode,
                );
                let selection = selection.apply_delta(
                    &delta,
                    cursor.is_insert(),
                    InsertDrift::Default,
                );
                deltas.push((text, delta, inval_lines));
                match cursor.mode() {
                    CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                        let offset = selection.min_offset();
                        let offset = if cursor.is_visual() {
                            offset + 1
                        } else {
                            offset
                        };
                        let line = buffer.line_of_offset(offset);
                        let offset =
                            buffer.first_non_blank_character_on_line(line)?;
                        cursor.set_mode(CursorMode::Normal(offset));
                    },
                    CursorMode::Insert(_) => {
                        cursor.set_mode(CursorMode::Insert(selection));
                    },
                }
            },
        }
        Ok(deltas)
    }

    fn do_indent(
        buffer: &mut Buffer,
        selection: &Selection,
    ) -> Result<(Rope, RopeDelta, InvalLines)> {
        let indent = buffer.indent_unit();
        let mut edits = Vec::new();

        let mut lines = HashSet::new();
        for region in selection.regions() {
            let start_line = buffer.line_of_offset(region.min());
            let mut end_line = buffer.line_of_offset(region.max());
            if end_line > start_line {
                let end_line_start = buffer.offset_of_line(end_line)?;
                if end_line_start == region.max() {
                    end_line -= 1;
                }
            }
            for line in start_line..=end_line {
                if lines.insert(line) {
                    let line_content = buffer.line_content(line)?;
                    if line_content == "\n" || line_content == "\r\n" {
                        continue;
                    }
                    let nonblank = buffer.first_non_blank_character_on_line(line)?;
                    let edit = create_edit(buffer, nonblank, indent)?;
                    edits.push(edit);
                }
            }
        }

        Ok(buffer.edit(&edits, EditType::Indent))
    }

    fn do_outdent(
        buffer: &mut Buffer,
        selection: &Selection,
    ) -> Result<(Rope, RopeDelta, InvalLines)> {
        let indent = buffer.indent_unit();
        let mut edits = Vec::new();

        let mut lines = HashSet::new();
        for region in selection.regions() {
            let start_line = buffer.line_of_offset(region.min());
            let mut end_line = buffer.line_of_offset(region.max());
            if end_line > start_line {
                let end_line_start = buffer.offset_of_line(end_line)?;
                if end_line_start == region.max() {
                    end_line -= 1;
                }
            }
            for line in start_line..=end_line {
                if lines.insert(line) {
                    let line_content = buffer.line_content(line)?;
                    if line_content == "\n" || line_content == "\r\n" {
                        continue;
                    }
                    let nonblank = buffer.first_non_blank_character_on_line(line)?;
                    if let Some(edit) = create_outdent(buffer, nonblank, indent) {
                        edits.push(edit);
                    }
                }
            }
        }

        Ok(buffer.edit(&edits, EditType::Outdent))
    }

    fn duplicate_line(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        direction: DuplicateDirection,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        // TODO other modes
        let selection = match cursor.mut_mode() {
            CursorMode::Insert(sel) => sel,
            _ => return vec![],
        };

        let mut line_ranges = HashSet::new();
        for region in selection.regions_mut() {
            let start_line = buffer.line_of_offset(region.start);
            let end_line = buffer.line_of_offset(region.end) + 1;

            line_ranges.insert(start_line..end_line);
        }

        let mut edits = vec![];
        for range in line_ranges {
            let Ok(start) = buffer.offset_of_line(range.start) else {
                error!("{}", range.start);
                continue;
            };
            let Ok(end) = buffer.offset_of_line(range.end) else {
                error!("{}", range.end);
                continue;
            };

            let content = buffer.slice_to_cow(start..end).into_owned();
            edits.push((
                match direction {
                    DuplicateDirection::Up => Selection::caret(end),
                    DuplicateDirection::Down => Selection::caret(start),
                },
                content,
            ));
        }

        let edits = edits
            .iter()
            .map(|(sel, content)| (sel, content.as_str()))
            .collect::<Vec<_>>();

        let (text, delta, inval_lines) = buffer.edit(&edits, EditType::InsertChars);

        *selection = selection.apply_delta(&delta, true, InsertDrift::Default);

        vec![(text, delta, inval_lines)]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn do_edit<T: Clipboard>(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        cmd: &EditCommand,
        clipboard: &mut T,
        register: &mut Register,
        conf: EditConf,
        screen_lines: &ScreenLines,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        match Self::_do_edit(
            cursor,
            buffer,
            cmd,
            clipboard,
            register,
            conf,
            screen_lines,
        ) {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                vec![]
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn _do_edit<T: Clipboard>(
        cursor: &mut Cursor,
        buffer: &mut Buffer,
        cmd: &EditCommand,
        clipboard: &mut T,
        register: &mut Register,
        EditConf {
            comment_token,
            modal,
            smart_tab,
            keep_indent,
            auto_indent,
        }: EditConf,
        screen_lines: &ScreenLines,
    ) -> Result<Vec<(Rope, RopeDelta, InvalLines)>> {
        use EditCommand::*;
        Ok(match cmd {
            MoveLineUp => {
                let mut deltas = Vec::new();
                if let CursorMode::Insert(mut selection) = cursor.mode().clone() {
                    for region in selection.regions_mut() {
                        let start_line = buffer.line_of_offset(region.min());
                        if start_line > 0 {
                            let previous_line_len =
                                buffer.line_content(start_line - 1)?.len();

                            let end_line = buffer.line_of_offset(region.max());
                            let start = buffer.offset_of_line(start_line)?;
                            let end = buffer.offset_of_line(end_line + 1)?;
                            let content =
                                buffer.slice_to_cow(start..end).to_string();
                            let (text, delta, inval_lines) = buffer.edit(
                                [
                                    (&Selection::region(start, end), ""),
                                    (
                                        &Selection::caret(
                                            buffer.offset_of_line(start_line - 1)?,
                                        ),
                                        &content,
                                    ),
                                ],
                                EditType::MoveLine,
                            );
                            deltas.push((text, delta, inval_lines));
                            region.start -= previous_line_len;
                            region.end -= previous_line_len;
                        }
                    }
                    cursor.set_mode(CursorMode::Insert(selection));
                }
                deltas
            },
            MoveLineDown => {
                let mut deltas = Vec::new();
                if let CursorMode::Insert(mut selection) = cursor.mode().clone() {
                    for region in selection.regions_mut().iter_mut().rev() {
                        let last_line = buffer.last_line();
                        let start_line = buffer.line_of_offset(region.min());
                        let end_line = buffer.line_of_offset(region.max());
                        if end_line < last_line {
                            let next_line_len =
                                buffer.line_content(end_line + 1)?.len();

                            let start = buffer.offset_of_line(start_line)?;
                            let end = buffer.offset_of_line(end_line + 1)?;
                            let content =
                                buffer.slice_to_cow(start..end).to_string();
                            let (text, delta, inval_lines) = buffer.edit(
                                [
                                    (
                                        &Selection::caret(
                                            buffer.offset_of_line(end_line + 2)?,
                                        ),
                                        content.as_str(),
                                    ),
                                    (&Selection::region(start, end), ""),
                                ],
                                EditType::MoveLine,
                            );
                            deltas.push((text, delta, inval_lines));
                            region.start += next_line_len;
                            region.end += next_line_len;
                        }
                    }
                    cursor.set_mode(CursorMode::Insert(selection));
                }
                deltas
            },
            InsertNewLine => match cursor.mode().clone() {
                CursorMode::Normal(offset) => {
                    warn!("Normal");
                    Self::insert_new_line(
                        buffer,
                        cursor,
                        Selection::caret(offset),
                        keep_indent,
                        auto_indent,
                    )
                },
                CursorMode::Insert(selection) => Self::insert_new_line(
                    buffer,
                    cursor,
                    selection,
                    keep_indent,
                    auto_indent,
                ),
                CursorMode::Visual {
                    start: _,
                    end: _,
                    mode: _,
                } => {
                    warn!("Visual");
                    vec![]
                },
            },
            InsertTab => {
                let mut deltas = Vec::new();
                if let CursorMode::Insert(selection) = cursor.mode() {
                    if smart_tab {
                        let indent = buffer.indent_unit();
                        let mut edits = Vec::new();

                        for region in selection.regions() {
                            if region.is_caret() {
                                edits.push(create_edit(
                                    buffer,
                                    region.start,
                                    indent,
                                )?)
                            } else {
                                let start_line = buffer.line_of_offset(region.min());
                                let end_line = buffer.line_of_offset(region.max());
                                for line in start_line..=end_line {
                                    let offset = buffer
                                        .first_non_blank_character_on_line(line)?;
                                    edits.push(create_edit(buffer, offset, indent)?)
                                }
                            }
                        }

                        let (text, delta, inval_lines) =
                            buffer.edit(&edits, EditType::InsertChars);
                        let selection = selection.apply_delta(
                            &delta,
                            true,
                            InsertDrift::Default,
                        );
                        deltas.push((text, delta, inval_lines));
                        cursor.set_mode(CursorMode::Insert(selection));
                    } else {
                        let (text, delta, inval_lines) =
                            buffer.edit([(&selection, "\t")], EditType::InsertChars);
                        let selection = selection.apply_delta(
                            &delta,
                            true,
                            InsertDrift::Default,
                        );
                        deltas.push((text, delta, inval_lines));
                        cursor.set_mode(CursorMode::Insert(selection));
                    }
                }
                deltas
            },
            IndentLine => {
                let selection = cursor.edit_selection(buffer)?;
                let (text, delta, inval_lines) =
                    Self::do_indent(buffer, &selection)?;
                cursor.apply_delta(&delta);
                vec![(text, delta, inval_lines)]
            },
            JoinLines => {
                let offset = cursor.offset();
                let (line, _col) = buffer.offset_to_line_col(offset)?;
                if line < buffer.last_line() {
                    let start = buffer.line_end_offset(line, true)?;
                    let end = buffer.first_non_blank_character_on_line(line + 1)?;
                    vec![buffer.edit(
                        [(&Selection::region(start, end), " ")],
                        EditType::Other,
                    )]
                } else {
                    vec![]
                }
            },
            OutdentLine => {
                let selection = cursor.edit_selection(buffer)?;
                let (text, delta, inval_lines) =
                    Self::do_outdent(buffer, &selection)?;
                cursor.apply_delta(&delta);
                vec![(text, delta, inval_lines)]
            },
            ToggleLineComment => {
                let mut lines = HashSet::new();
                let selection = cursor.edit_selection(buffer)?;
                let mut had_comment = true;
                let mut smallest_indent = usize::MAX;
                for region in selection.regions() {
                    let mut line = buffer.line_of_offset(region.min());
                    let end_line = buffer.line_of_offset(region.max());
                    let end_line_offset = buffer.offset_of_line(end_line)?;
                    let end = if end_line > line && region.max() == end_line_offset {
                        end_line_offset
                    } else {
                        buffer.offset_of_line(end_line + 1)?
                    };
                    let start = buffer.offset_of_line(line)?;
                    for content in buffer.text().lines(start..end) {
                        let trimmed_content = content.trim_start();
                        if trimmed_content.is_empty() {
                            line += 1;
                            continue;
                        }
                        let indent = content.len() - trimmed_content.len();
                        if indent < smallest_indent {
                            smallest_indent = indent;
                        }
                        if !trimmed_content.starts_with(comment_token) {
                            had_comment = false;
                            lines.insert((line, indent, 0));
                        } else {
                            let had_space_after_comment =
                                trimmed_content.chars().nth(comment_token.len())
                                    == Some(' ');
                            lines.insert((
                                line,
                                indent,
                                comment_token.len()
                                    + usize::from(had_space_after_comment),
                            ));
                        }
                        line += 1;
                    }
                }

                let (text, delta, inval_lines) = if had_comment {
                    let mut selection = Selection::new();
                    for (line, indent, len) in lines.iter() {
                        let start = buffer.offset_of_line(*line)? + indent;
                        selection.add_region(SelRegion::new(
                            start,
                            start + len,
                            None,
                        ))
                    }
                    buffer.edit([(&selection, "")], EditType::ToggleComment)
                } else {
                    let mut selection = Selection::new();
                    for (line, _, _) in lines.iter() {
                        let start = buffer.offset_of_line(*line)? + smallest_indent;
                        selection.add_region(SelRegion::new(start, start, None))
                    }
                    buffer.edit(
                        [(&selection, format!("{comment_token} ").as_str())],
                        EditType::ToggleComment,
                    )
                };
                cursor.apply_delta(&delta);
                vec![(text, delta, inval_lines)]
            },
            Undo => {
                if let Some((text, delta, inval_lines, cursor_mode)) =
                    buffer.do_undo()
                {
                    apply_undo_redo(
                        cursor,
                        buffer,
                        modal,
                        text,
                        delta,
                        inval_lines,
                        cursor_mode,
                    )
                } else {
                    vec![]
                }
            },
            Redo => {
                if let Some((text, delta, inval_lines, cursor_mode)) =
                    buffer.do_redo()
                {
                    apply_undo_redo(
                        cursor,
                        buffer,
                        modal,
                        text,
                        delta,
                        inval_lines,
                        cursor_mode,
                    )
                } else {
                    vec![]
                }
            },
            ClipboardCopy => {
                let Ok(data) = cursor.yank(buffer) else {
                    error!("fail");
                    return Ok(vec![]);
                };
                clipboard.put_string(data.content);

                match cursor.mode() {
                    CursorMode::Visual {
                        start,
                        end,
                        mode: _,
                    } => {
                        let offset = *start.min(end);
                        let offset =
                            buffer.offset_line_end(offset, false)?.min(offset);
                        cursor.set_mode(CursorMode::Normal(offset));
                    },
                    CursorMode::Normal(_) | CursorMode::Insert(_) => {},
                }
                vec![]
            },
            ClipboardCut => {
                let data = cursor.yank(buffer)?;
                clipboard.put_string(data.content);

                let selection = if let CursorMode::Insert(mut selection) =
                    cursor.mode().clone()
                {
                    for region in selection.regions_mut() {
                        if region.is_caret() {
                            let line = buffer.line_of_offset(region.start);
                            let start = buffer.offset_of_line(line)?;
                            let end = buffer.offset_of_line(line + 1)?;
                            region.start = start;
                            region.end = end;
                        }
                    }
                    selection
                } else {
                    cursor.edit_selection(buffer)?
                };

                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::Cut);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            ClipboardPaste => {
                if let Some(s) = clipboard.get_string() {
                    // ??
                    let mode = if s.ends_with('\n') && cursor.is_visual() {
                        VisualMode::Linewise
                    } else {
                        VisualMode::Normal
                    };
                    let data = RegisterData { content: s, mode };
                    Self::do_paste(cursor, buffer, &data)
                } else {
                    vec![]
                }
            },
            Yank => {
                match cursor.mode() {
                    CursorMode::Visual { start, end, .. } => {
                        let data = cursor.yank(buffer)?;
                        register.add_yank(data);

                        let offset = *start.min(end);
                        let offset =
                            buffer.offset_line_end(offset, false)?.min(offset);
                        cursor.set_mode(CursorMode::Normal(offset));
                    },
                    CursorMode::Normal(_) => {},
                    CursorMode::Insert(_) => {},
                }
                vec![]
            },
            Paste => {
                let data = register.unnamed.clone();
                Self::do_paste(cursor, buffer, &data)
            },
            PasteBefore => {
                let offset = cursor.offset();
                let data = register.unnamed.clone();
                let mut local_cursor =
                    Cursor::new(CursorMode::Insert(Selection::new()), None, None);
                local_cursor.set_offset(offset, false, false);
                Self::do_paste(&mut local_cursor, buffer, &data)
            },
            NewLineAbove => {
                let offset = cursor.offset();
                let line = buffer.line_of_offset(offset);
                let offset = if line > 0 {
                    buffer.line_end_offset(line - 1, true)
                } else {
                    buffer.first_non_blank_character_on_line(line)
                }?;
                let delta = Self::insert_new_line(
                    buffer,
                    cursor,
                    Selection::caret(offset),
                    keep_indent,
                    auto_indent,
                );
                if line == 0 {
                    cursor.set_mode(CursorMode::Insert(Selection::caret(offset)));
                }
                delta
            },
            NewLineBelow => {
                let offset = cursor.offset();
                let offset = buffer.offset_line_end(offset, true)?;
                Self::insert_new_line(
                    buffer,
                    cursor,
                    Selection::caret(offset),
                    keep_indent,
                    auto_indent,
                )
            },
            DeleteBackward => {
                let (selection, edit_type) = match cursor.mode() {
                    CursorMode::Normal(_) => {
                        (cursor.edit_selection(buffer)?, EditType::Delete)
                    },
                    CursorMode::Visual { .. } => {
                        (cursor.edit_selection(buffer)?, EditType::DeleteSelection)
                    },
                    CursorMode::Insert(_) => {
                        let selection = cursor.edit_selection(buffer)?;
                        let edit_type = if selection.is_caret() {
                            EditType::Delete
                        } else {
                            EditType::DeleteSelection
                        };
                        let indent = buffer.indent_unit();
                        let mut new_selection = Selection::new();
                        for region in selection.regions() {
                            let new_region = if region.is_caret() {
                                if indent.starts_with('\t') {
                                    let new_end = buffer.move_left(
                                        region.end,
                                        Mode::Insert,
                                        1,
                                    )?;
                                    SelRegion::new(region.start, new_end, None)
                                } else {
                                    let line = buffer.line_of_offset(region.start);
                                    let nonblank = buffer
                                        .first_non_blank_character_on_line(line)?;
                                    let (_, col) =
                                        buffer.offset_to_line_col(region.start)?;
                                    let count =
                                        if region.start <= nonblank && col > 0 {
                                            let r = col % indent.len();
                                            if r == 0 { indent.len() } else { r }
                                        } else {
                                            1
                                        };
                                    let new_end = buffer.move_left(
                                        region.end,
                                        Mode::Insert,
                                        count,
                                    )?;
                                    SelRegion::new(region.start, new_end, None)
                                }
                            } else {
                                *region
                            };
                            new_selection.add_region(new_region);
                        }

                        let mut selection = new_selection;
                        if selection.regions().len() == 1 {
                            let delete_str = buffer
                                .slice_to_cow(
                                    selection.min_offset()..selection.max_offset(),
                                )
                                .to_string();
                            if str_is_pair_left(&delete_str)
                                || delete_str == "\""
                                || delete_str == "'"
                                || delete_str == "`"
                            {
                                let matching_char = match delete_str.as_str() {
                                    "\"" => Some('"'),
                                    "'" => Some('\''),
                                    "`" => Some('`'),
                                    _ => str_matching_pair(&delete_str),
                                };
                                if let Some(c) = matching_char {
                                    let offset = selection.max_offset();
                                    let line = buffer.line_of_offset(offset);
                                    let line_end =
                                        buffer.line_end_offset(line, true)?;
                                    let content = buffer
                                        .slice_to_cow(offset..line_end)
                                        .to_string();
                                    if content.trim().starts_with(&c.to_string()) {
                                        let index = content
                                            .match_indices(c)
                                            .next()
                                            .unwrap()
                                            .0;
                                        selection = Selection::region(
                                            selection.min_offset(),
                                            offset + index + 1,
                                        );
                                    }
                                }
                            }
                        }
                        (selection, edit_type)
                    },
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], edit_type);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            DeleteForward => {
                let (selection, edit_type) = match cursor.mode() {
                    CursorMode::Normal(_) => {
                        (cursor.edit_selection(buffer)?, EditType::Delete)
                    },
                    CursorMode::Visual { .. } => {
                        (cursor.edit_selection(buffer)?, EditType::DeleteSelection)
                    },
                    CursorMode::Insert(_) => {
                        let selection = cursor.edit_selection(buffer)?;
                        let edit_type = if selection.is_caret() {
                            EditType::Delete
                        } else {
                            EditType::DeleteSelection
                        };
                        let mut new_selection = Selection::new();
                        for region in selection.regions() {
                            let new_region = if region.is_caret() {
                                let new_end = buffer.move_right(
                                    region.end,
                                    Mode::Insert,
                                    1,
                                )?;
                                SelRegion::new(region.start, new_end, None)
                            } else {
                                *region
                            };
                            new_selection.add_region(new_region);
                        }
                        (new_selection, edit_type)
                    },
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], edit_type);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            DeleteLine => {
                let line = cursor
                    .get_selection()
                    .map(|x| {
                        if x.0 == x.1 {
                            screen_lines
                                .visual_line_for_buffer_offset(cursor.offset())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let selection = if let Some((_, vl)) = line {
                    Selection::region(
                        vl.folded_line.origin_interval.start,
                        vl.folded_line.origin_interval.end,
                    )
                } else {
                    let selection = cursor.edit_selection(buffer)?;
                    let range = format_start_end(
                        buffer,
                        selection.min_offset()..selection.max_offset(),
                        true,
                        false,
                        1,
                    )?;
                    Selection::region(range.start, range.end)
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::Delete);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.set_mode(CursorMode::Insert(selection));
                vec![(text, delta, inval_lines)]
            },

            DeleteWordForward => {
                let selection = match cursor.mode() {
                    CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                        cursor.edit_selection(buffer)?
                    },
                    CursorMode::Insert(_) => {
                        let mut new_selection = Selection::new();
                        let selection = cursor.edit_selection(buffer)?;

                        for region in selection.regions() {
                            let end = buffer.move_word_forward(region.end);
                            let new_region = SelRegion::new(region.start, end, None);
                            new_selection.add_region(new_region);
                        }

                        new_selection
                    },
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::DeleteWord);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            DeleteWordBackward => {
                let selection = match cursor.mode() {
                    CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                        cursor.edit_selection(buffer)?
                    },
                    CursorMode::Insert(_) => {
                        let mut new_selection = Selection::new();
                        let selection = cursor.edit_selection(buffer)?;

                        for region in selection.regions() {
                            let end = buffer.move_word_backward_deletion(region.end);
                            let new_region = SelRegion::new(region.start, end, None);
                            new_selection.add_region(new_region);
                        }

                        new_selection
                    },
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::DeleteWord);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            DeleteToBeginningOfLine => {
                let selection = match cursor.mode() {
                    CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                        cursor.edit_selection(buffer)?
                    },
                    CursorMode::Insert(_) => {
                        let selection = cursor.edit_selection(buffer)?;

                        let mut new_selection = Selection::new();
                        for region in selection.regions() {
                            let line = buffer.line_of_offset(region.end);
                            let end = buffer.offset_of_line(line)?;
                            let new_region = SelRegion::new(region.start, end, None);
                            new_selection.add_region(new_region);
                        }

                        new_selection
                    },
                };
                let (text, delta, inval_lines) = buffer
                    .edit([(&selection, "")], EditType::DeleteToBeginningOfLine);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            DeleteToEndOfLine => {
                let selection = match cursor.mode() {
                    CursorMode::Normal(_) | CursorMode::Visual { .. } => {
                        cursor.edit_selection(buffer)?
                    },
                    CursorMode::Insert(_) => {
                        let mut selection = cursor.edit_selection(buffer)?;

                        let cursor_offset = cursor.offset();
                        let line = buffer.line_of_offset(cursor_offset);
                        let end_of_line_offset =
                            buffer.line_end_offset(line, true)?;
                        let new_region =
                            SelRegion::new(cursor_offset, end_of_line_offset, None);
                        selection.add_region(new_region);

                        selection
                    },
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::DeleteToEndOfLine);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.update_selection(buffer, selection);
                vec![(text, delta, inval_lines)]
            },
            DeleteForwardAndInsert => {
                let selection = cursor.edit_selection(buffer)?;
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::Delete);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.set_mode(CursorMode::Insert(selection));
                vec![(text, delta, inval_lines)]
            },
            DeleteWordAndInsert => {
                let selection = {
                    let mut new_selection = Selection::new();
                    let selection = cursor.edit_selection(buffer)?;

                    for region in selection.regions() {
                        let end = buffer.move_word_forward(region.end);
                        let new_region = SelRegion::new(region.start, end, None);
                        new_selection.add_region(new_region);
                    }

                    new_selection
                };
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::DeleteWord);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.set_mode(CursorMode::Insert(selection));
                vec![(text, delta, inval_lines)]
            },
            DeleteLineAndInsert => {
                let selection = cursor.edit_selection(buffer)?;
                let range = format_start_end(
                    buffer,
                    selection.min_offset()..selection.max_offset(),
                    true,
                    true,
                    1,
                )?;
                let selection = Selection::region(range.start, range.end - 1); // -1 because we want to keep the line itself
                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::Delete);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.set_mode(CursorMode::Insert(selection));
                vec![(text, delta, inval_lines)]
            },
            DeleteToEndOfLineAndInsert => {
                let mut selection = cursor.edit_selection(buffer)?;

                let cursor_offset = cursor.offset();
                let line = buffer.line_of_offset(cursor_offset);
                let end_of_line_offset = buffer.line_end_offset(line, true)?;

                let new_region =
                    SelRegion::new(cursor_offset, end_of_line_offset, None);
                selection.add_region(new_region);

                let (text, delta, inval_lines) =
                    buffer.edit([(&selection, "")], EditType::Delete);
                let selection =
                    selection.apply_delta(&delta, true, InsertDrift::Default);
                cursor.set_mode(CursorMode::Insert(selection));
                vec![(text, delta, inval_lines)]
            },
            NormalMode => {
                if !modal {
                    if let CursorMode::Insert(selection) = &cursor.mode() {
                        match selection.regions().len() {
                            i if i > 1 => {
                                if let Some(region) = selection.last_inserted() {
                                    let new_selection =
                                        Selection::region(region.start, region.end);
                                    cursor
                                        .set_mode(CursorMode::Insert(new_selection));
                                    return Ok(vec![]);
                                }
                            },
                            1 => {
                                let region = selection.regions()[0];
                                if !region.is_caret() {
                                    let new_selection = Selection::caret(region.end);
                                    cursor
                                        .set_mode(CursorMode::Insert(new_selection));
                                    return Ok(vec![]);
                                }
                            },
                            _ => (),
                        }
                    }

                    return Ok(vec![]);
                }

                let offset = match cursor.mode() {
                    CursorMode::Insert(selection) => {
                        let offset = selection.min_offset();
                        buffer.prev_grapheme_offset(
                            offset,
                            1,
                            buffer.offset_of_line(buffer.line_of_offset(offset))?,
                        )
                    },
                    CursorMode::Visual { end, .. } => {
                        buffer.offset_line_end(*end, false)?.min(*end)
                    },
                    CursorMode::Normal(offset) => *offset,
                };

                buffer.reset_edit_type();
                cursor.set_mode(CursorMode::Normal(offset));
                cursor.horiz = None;
                vec![]
            },
            InsertMode => {
                cursor
                    .set_mode(CursorMode::Insert(Selection::caret(cursor.offset())));
                vec![]
            },
            InsertFirstNonBlank => {
                match &cursor.mode() {
                    CursorMode::Normal(offset) => {
                        let line = buffer.line_of_offset(*offset);
                        let offset =
                            buffer.first_non_blank_character_on_line(line)?;
                        cursor
                            .set_mode(CursorMode::Insert(Selection::caret(offset)));
                    },
                    CursorMode::Visual { .. } => {
                        let mut selection = Selection::new();
                        for region in cursor.edit_selection(buffer)?.regions() {
                            selection.add_region(SelRegion::caret(region.min()));
                        }
                        cursor.set_mode(CursorMode::Insert(selection));
                    },
                    CursorMode::Insert(_) => {},
                };
                vec![]
            },
            Append => {
                let offset = cursor.offset();
                let line = buffer.line_of_offset(offset);
                let Ok(line_len) = buffer.line_len(line) else {
                    error!("line len {line}");
                    return Ok(vec![]);
                };
                let count = (line_len > 1
                    || (buffer.last_line() == line && line_len > 0))
                    as usize;
                let Ok(offset) =
                    buffer.move_right(cursor.offset(), Mode::Insert, count)
                else {
                    error!("line_end_offset {line}");
                    return Ok(vec![]);
                };
                cursor.set_mode(CursorMode::Insert(Selection::caret(offset)));
                vec![]
            },
            AppendEndOfLine => {
                let offset = cursor.offset();
                let line = buffer.line_of_offset(offset);
                let Ok(offset) = buffer.line_end_offset(line, true) else {
                    error!("line_end_offset {line}");
                    return Ok(vec![]);
                };
                cursor.set_mode(CursorMode::Insert(Selection::caret(offset)));
                vec![]
            },
            ToggleVisualMode => {
                Self::toggle_visual(cursor, VisualMode::Normal, modal);
                vec![]
            },
            ToggleLinewiseVisualMode => {
                Self::toggle_visual(cursor, VisualMode::Linewise, modal);
                vec![]
            },
            ToggleBlockwiseVisualMode => {
                Self::toggle_visual(cursor, VisualMode::Blockwise, modal);
                vec![]
            },
            DuplicateLineUp => {
                Self::duplicate_line(cursor, buffer, DuplicateDirection::Up)
            },
            DuplicateLineDown => {
                Self::duplicate_line(cursor, buffer, DuplicateDirection::Down)
            },
            NormalizeLineEndings => {
                let Some((text, delta, inval)) = buffer.normalize_line_endings()
                else {
                    return Ok(vec![]);
                };

                cursor.apply_delta(&delta);

                vec![(text, delta, inval)]
            },
        })
    }
}

fn apply_undo_redo(
    cursor: &mut Cursor,
    buffer: &mut Buffer,
    modal: bool,
    text: Rope,
    delta: RopeDelta,
    inval_lines: InvalLines,
    cursor_mode: Option<CursorMode>,
) -> Vec<(Rope, RopeDelta, InvalLines)> {
    if let Some(cursor_mode) = cursor_mode {
        cursor.set_mode(if modal {
            CursorMode::Normal(cursor_mode.offset())
        } else if cursor.is_insert() {
            cursor_mode
        } else {
            CursorMode::Insert(Selection::caret(cursor_mode.offset()))
        });
    } else if let Some(new_cursor) =
        get_first_selection_after(cursor, buffer, &delta)
    {
        *cursor = new_cursor
    } else if !delta
        .els
        .iter()
        .any(|el| matches!(el, DeltaElement::Copy(_, _)))
    {
        // if there's no copy that means the whole document was
        // swapped we'd better not moving the cursor
    } else {
        cursor.apply_delta(&delta);
    }
    vec![(text, delta, inval_lines)]
}

enum DuplicateDirection {
    Up,
    Down,
}

#[cfg(test)]
mod test {
    use crate::lines::{
        buffer::{Buffer, rope_text::RopeText},
        cursor::{Cursor, CursorMode},
        edit::{Action, DuplicateDirection},
        selection::{SelRegion, Selection},
        word::WordCursor,
    };

    fn prev_unmatched(buffer: &Buffer, c: char, offset: usize) -> Option<usize> {
        WordCursor::new(buffer.text(), offset).previous_unmatched(c)
    }

    #[test]
    fn test_insert_simple() {
        let mut buffer = Buffer::new("abc");
        let mut cursor =
            Cursor::new(CursorMode::Insert(Selection::caret(1)), None, None);

        Action::insert(&mut cursor, &mut buffer, "e", &prev_unmatched, true, true);
        assert_eq!("aebc", buffer.slice_to_cow(0..buffer.len()));
    }

    #[test]
    fn test_insert_multiple_cursor() {
        let mut buffer = Buffer::new("abc\nefg\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(1));
        selection.add_region(SelRegion::caret(5));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::insert(&mut cursor, &mut buffer, "i", &prev_unmatched, true, true);
        assert_eq!("aibc\neifg\n", buffer.slice_to_cow(0..buffer.len()));
    }

    #[test]
    fn test_insert_complex() {
        let mut buffer = Buffer::new("abc\nefg\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(1));
        selection.add_region(SelRegion::caret(5));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::insert(&mut cursor, &mut buffer, "i", &prev_unmatched, true, true);
        assert_eq!("aibc\neifg\n", buffer.slice_to_cow(0..buffer.len()));
        Action::insert(&mut cursor, &mut buffer, "j", &prev_unmatched, true, true);
        assert_eq!("aijbc\neijfg\n", buffer.slice_to_cow(0..buffer.len()));
        Action::insert(&mut cursor, &mut buffer, "{", &prev_unmatched, true, true);
        assert_eq!("aij{bc\neij{fg\n", buffer.slice_to_cow(0..buffer.len()));
        Action::insert(&mut cursor, &mut buffer, " ", &prev_unmatched, true, true);
        assert_eq!("aij{ bc\neij{ fg\n", buffer.slice_to_cow(0..buffer.len()));
    }

    #[test]
    fn test_insert_pair() {
        let mut buffer = Buffer::new("a bc\ne fg\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(1));
        selection.add_region(SelRegion::caret(6));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::insert(&mut cursor, &mut buffer, "{", &prev_unmatched, true, true);
        assert_eq!("a{} bc\ne{} fg\n", buffer.slice_to_cow(0..buffer.len()));
        Action::insert(&mut cursor, &mut buffer, "}", &prev_unmatched, true, true);
        assert_eq!("a{} bc\ne{} fg\n", buffer.slice_to_cow(0..buffer.len()));
    }

    #[test]
    fn test_insert_pair_with_selection() {
        let mut buffer = Buffer::new("a bc\ne fg\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::new(0, 4, None));
        selection.add_region(SelRegion::new(5, 9, None));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);
        Action::insert(&mut cursor, &mut buffer, "{", &prev_unmatched, true, true);
        assert_eq!("{a bc}\n{e fg}\n", buffer.slice_to_cow(0..buffer.len()));
    }

    #[test]
    fn test_insert_pair_without_auto_closing() {
        let mut buffer = Buffer::new("a bc\ne fg\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(1));
        selection.add_region(SelRegion::caret(6));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::insert(&mut cursor, &mut buffer, "{", &prev_unmatched, false, false);
        assert_eq!("a{ bc\ne{ fg\n", buffer.slice_to_cow(0..buffer.len()));
        Action::insert(&mut cursor, &mut buffer, "}", &prev_unmatched, false, false);
        assert_eq!("a{} bc\ne{} fg\n", buffer.slice_to_cow(0..buffer.len()));
    }

    #[test]
    fn duplicate_down_simple() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Down);

        assert_ne!(cursor.offset(), 0);
        assert_eq!(
            "first line\nfirst line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_up_simple() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Up);

        assert_eq!(cursor.offset(), 0);
        assert_eq!(
            "first line\nfirst line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_down_multiple_cursors_in_same_line() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        selection.add_region(SelRegion::caret(1));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Down);

        assert_eq!(
            "first line\nfirst line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_up_multiple_cursors_in_same_line() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        selection.add_region(SelRegion::caret(1));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Up);

        assert_eq!(
            "first line\nfirst line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_down_multiple() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        selection.add_region(SelRegion::caret(15));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Down);

        assert_eq!(
            "first line\nfirst line\nsecond line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_up_multiple() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        selection.add_region(SelRegion::caret(15));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Up);

        assert_eq!(
            "first line\nfirst line\nsecond line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_down_multiple_with_swapped_cursor_order() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(15));
        selection.add_region(SelRegion::caret(0));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Down);

        assert_eq!(
            "first line\nfirst line\nsecond line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn duplicate_up_multiple_with_swapped_cursor_order() {
        let mut buffer = Buffer::new("first line\nsecond line\n");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(15));
        selection.add_region(SelRegion::caret(0));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::duplicate_line(&mut cursor, &mut buffer, DuplicateDirection::Up);

        assert_eq!(
            "first line\nfirst line\nsecond line\nsecond line\n",
            buffer.slice_to_cow(0..buffer.len())
        );
    }

    #[test]
    fn check_multiple_cursor_match_insertion() {
        let mut buffer = Buffer::new(" 123 567 9ab def");
        let mut selection = Selection::new();
        selection.add_region(SelRegion::caret(0));
        selection.add_region(SelRegion::caret(4));
        selection.add_region(SelRegion::caret(8));
        selection.add_region(SelRegion::caret(12));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);

        Action::insert(&mut cursor, &mut buffer, "(", &prev_unmatched, true, true);

        assert_eq!(
            "() 123() 567() 9ab() def",
            buffer.slice_to_cow(0..buffer.len())
        );

        let mut end_selection = Selection::new();
        end_selection.add_region(SelRegion::caret(1));
        end_selection.add_region(SelRegion::caret(7));
        end_selection.add_region(SelRegion::caret(13));
        end_selection.add_region(SelRegion::caret(19));
        assert_eq!(cursor.mode().clone(), CursorMode::Insert(end_selection));
    }

    // TODO(dbuga): add tests duplicating selections (multiple line
    // blocks)
}

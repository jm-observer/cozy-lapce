//! Movement logic for the editor.
use anyhow::{Result, bail};
use doc::lines::{
    buffer::rope_text::RopeText,
    command::MultiSelectionCommand,
    cursor::{ColPosition, Cursor, CursorAffinity, CursorMode},
    mode::{Mode, MotionMode, VisualMode},
    movement::{LinePosition, Movement},
    register::Register,
    selection::{SelRegion, Selection},
    soft_tab::SnapDirection,
};
use lapce_xi_rope::Rope;
use log::info;

use crate::{doc::Doc, editor::floem_editor::CommonAction};

/// Move a selection region by a given movement.
/// Much of the time, this will just be a matter of moving the cursor, but
/// some movements may depend on the current selection.
fn move_region(
    region: &SelRegion,
    affinity: &mut CursorAffinity,
    count: usize,
    modify: bool,
    movement: &Movement,
    mode: Mode,
    doc: &Doc,
) -> Result<SelRegion> {
    let (count, region) = if count >= 1 && !modify && !region.is_caret() {
        // If we're not a caret, and we are moving left/up or right/down, we want to
        // move the cursor to the left or right side of the selection.
        // Ex: `|abc|` -> left/up arrow key -> `|abc`
        // Ex: `|abc|` -> right/down arrow key -> `abc|`
        // and it doesn't matter which direction the selection is going, so we use
        // min/max
        match movement {
            Movement::Left | Movement::Up => {
                let leftmost = region.min();
                (count - 1, SelRegion::new(leftmost, leftmost, region.horiz))
            },
            Movement::Right | Movement::Down => {
                let rightmost = region.max();
                (
                    count - 1,
                    SelRegion::new(rightmost, rightmost, region.horiz),
                )
            },
            _ => (count, *region),
        }
    } else {
        (count, *region)
    };

    let (end, horiz) = if count == 0 {
        (region.end, region.horiz)
    } else {
        move_offset(
            region.end,
            region.horiz.as_ref(),
            affinity,
            count,
            movement,
            mode,
            doc,
        )?
    };

    let start = match modify {
        true => region.start,
        false => end,
    };
    Ok(SelRegion::new(start, end, horiz))
}

pub fn move_selection(
    selection: &Selection,
    affinity: &mut CursorAffinity,
    count: usize,
    modify: bool,
    movement: &Movement,
    mode: Mode,
    doc: &Doc,
) -> Result<Selection> {
    let mut new_selection = Selection::new();
    for region in selection.regions() {
        new_selection.add_region(move_region(
            region, affinity, count, modify, movement, mode, doc,
        )?);
    }
    Ok(new_selection)
}

// TODO: It would probably fit the overall logic better if affinity was
// immutable and it just returned the new affinity!
pub fn move_offset(
    offset: usize,
    horiz: Option<&ColPosition>,
    affinity: &mut CursorAffinity,
    count: usize,
    movement: &Movement,
    mode: Mode,
    doc: &Doc,
) -> Result<(usize, Option<ColPosition>)> {
    let rope = doc.rope_text();
    let (new_offset, horiz) = match movement {
        Movement::Left => {
            let new_offset = move_left(offset, affinity, mode, count, doc)?;

            (new_offset, None)
        },
        Movement::Right => {
            let new_offset = move_right(offset, affinity, mode, count, doc)?;

            (new_offset, None)
        },
        Movement::Up => {
            let Some((new_offset, horiz)) =
                move_up(offset, affinity, horiz.cloned(), mode, count, doc)?
            else {
                return Ok((offset, horiz.cloned()));
            };

            (new_offset, Some(horiz))
        },
        Movement::Down => {
            let Some((new_offset, horiz)) =
                move_down(offset, affinity, horiz.cloned(), mode, count, doc)?
            else {
                return Ok((offset, horiz.cloned()));
            };

            (new_offset, Some(horiz))
        },
        Movement::DocumentStart => {
            // Put it before any inlay hints at the very start
            *affinity = CursorAffinity::Backward;
            (0, Some(ColPosition::Start))
        },
        Movement::DocumentEnd => {
            let (new_offset, horiz) = document_end(rope, affinity, mode)?;

            (new_offset, Some(horiz))
        },
        Movement::FirstNonBlank => {
            let (new_offset, horiz) = first_non_blank(doc, affinity, offset)?;

            (new_offset, Some(horiz))
        },
        Movement::StartOfLine => {
            let (new_offset, horiz) = start_of_line(affinity, offset)?;
            info!("StartOfLine offset={offset} new_offset={new_offset}");
            (new_offset, Some(horiz))
        },
        Movement::EndOfLine => {
            let (new_offset, horiz) = end_of_line(doc, affinity, offset, mode)?;
            info!("EndOfLine offset={offset} new_offset={new_offset}");
            (new_offset, Some(horiz))
        },
        Movement::Line(position) => {
            let (new_offset, horiz) =
                to_line(offset, horiz.cloned(), mode, position);

            (new_offset, Some(horiz))
        },
        Movement::Offset(offset) => {
            let new_offset = doc.lines.with_untracked(|x| {
                x.buffer().text().prev_grapheme_offset(*offset + 1).unwrap()
            });
            // view.text()
            (new_offset, None)
        },
        Movement::WordEndForward => {
            let new_offset =
                rope.move_n_wordends_forward(offset, count, mode == Mode::Insert);
            (new_offset, None)
        },
        Movement::WordForward => {
            let new_offset = rope.move_n_words_forward(offset, count);
            (new_offset, None)
        },
        Movement::WordBackward => {
            let new_offset = rope.move_n_words_backward(offset, count, mode);
            (new_offset, None)
        },
        Movement::NextUnmatched(char) => {
            let new_offset = doc.find_unmatched(offset, false, *char);

            (new_offset, None)
        },
        Movement::PreviousUnmatched(char) => {
            let new_offset = doc.find_unmatched(offset, true, *char);

            (new_offset, None)
        },
        Movement::MatchPairs => {
            let new_offset = doc.find_matching_pair(offset);

            (new_offset, None)
        },
        Movement::ParagraphForward => {
            let new_offset = rope.move_n_paragraphs_forward(offset, count);

            (new_offset, None)
        },
        Movement::ParagraphBackward => {
            let new_offset = rope.move_n_paragraphs_backward(offset, count);

            (new_offset, None)
        },
    };

    // todo ?
    // let new_offset = correct_crlf(&rope, new_offset);

    Ok((new_offset, horiz))
}

// /// If the offset is at `\r|\n` then move it back.
// fn correct_crlf(text: &RopeTextVal, offset: usize) -> usize {
//     if offset == 0 || offset == text.len() {
//         return offset;
//     }
//
//     let mut cursor = lapce_xi_rope::Cursor::new(text.text(), offset);
//     if cursor.peek_next_codepoint() == Some('\n')
//         && cursor.prev_codepoint() == Some('\r')
//     {
//         return offset - 1;
//     }
//
//     offset
// }

// fn atomic_soft_tab_width_for_offset(ed: &Editor) -> Option<usize> {
//     // let line = ed
//     //     .visual_line_of_offset_v2(offset, CursorAffinity::Forward)
//     //     .ok()?
//     //     .0
//     //     .origin_line_start;
//     let style = ed.doc();
//     if style.atomic_soft_tabs() {
//         Some(style.tab_width())
//     } else {
//         None
//     }
// }

pub fn snap_to_soft_tab(
    text: &Rope,
    offset: usize,
    direction: SnapDirection,
    tab_width: usize,
) -> Result<usize> {
    // Fine which line we're on.
    let line = text.line_of_offset(offset);
    // Get the offset to the start of the line.
    let start_line_offset = text.offset_of_line(line)?;
    // And the offset within the lint.
    let offset_within_line = offset - start_line_offset;

    Ok(start_line_offset
        + snap_to_soft_tab_logic(
            text,
            offset_within_line,
            start_line_offset,
            direction,
            tab_width,
        ))
}

fn snap_to_soft_tab_logic(
    text: &Rope,
    offset_or_col: usize,
    start_line_offset: usize,
    direction: SnapDirection,
    tab_width: usize,
) -> usize {
    assert!(tab_width >= 1);

    // Number of spaces, ignoring incomplete soft tabs.
    let space_count =
        (count_spaces_from(text, start_line_offset) / tab_width) * tab_width;

    // If we're past the soft tabs, we don't need to snap.
    if offset_or_col >= space_count {
        return offset_or_col;
    }

    let bias = match direction {
        SnapDirection::Left => 0,
        SnapDirection::Right => tab_width - 1,
        SnapDirection::Nearest => tab_width / 2,
    };

    ((offset_or_col + bias) / tab_width) * tab_width
}

fn count_spaces_from(text: &Rope, from_offset: usize) -> usize {
    let mut cursor = lapce_xi_rope::Cursor::new(text, from_offset);
    let mut space_count = 0usize;
    while let Some(next) = cursor.next_codepoint() {
        if next != ' ' {
            break;
        }
        space_count += 1;
    }
    space_count
}

/// Move the offset to the left by `count` amount.
/// If `soft_tab_width` is `Some` (and greater than 1) then the offset will snap
/// to the soft tab.
fn move_left(
    offset: usize,
    affinity: &mut CursorAffinity,
    _mode: Mode,
    _count: usize,
    doc: &Doc,
) -> Result<usize> {
    log::info!("move_left {offset} {affinity:?}");
    let Some((new_offset, new_affinity)) = doc
        .lines
        .with_untracked(|x| x.move_left(offset, *affinity))?
    else {
        return Ok(offset);
    };
    log::info!("move_left result {new_offset} {new_affinity:?}");
    *affinity = new_affinity;
    Ok(new_offset)
    // let rope_text = ed.rope_text();
    // let mut new_offset = rope_text.move_left(offset, mode, count)?;
    //
    // if let Some(soft_tab_width) = atomic_soft_tab_width_for_offset(ed) {
    //     if soft_tab_width > 1 {
    //         new_offset = snap_to_soft_tab(
    //             rope_text.text(),
    //             new_offset,
    //             SnapDirection::Left,
    //             soft_tab_width
    //         )?;
    //     }
    // }
    //
    // *affinity = CursorAffinity::Forward;
    //
    // Ok(new_offset)
}
/// Move the offset to the right by `count` amount.
/// If `soft_tab_width` is `Some` (and greater than 1) then the offset will snap
/// to the soft tab.
fn move_right(
    offset: usize,
    affinity: &mut CursorAffinity,
    _mode: Mode,
    _count: usize,
    doc: &Doc,
) -> Result<usize> {
    log::info!("move_right {offset} {affinity:?}");
    let Some((new_offset, new_affinity)) = doc
        .lines
        .with_untracked(|x| x.move_right(offset, *affinity))?
    else {
        return Ok(offset);
    };
    log::info!("move_right result {new_offset} {new_affinity:?}");
    *affinity = new_affinity;
    Ok(new_offset)
}

/// Move the offset up by `count` amount.
/// `count` may be zero, because moving up in a selection just jumps to the
/// start of the selection.
fn move_up(
    offset: usize,
    affinity: &mut CursorAffinity,
    horiz: Option<ColPosition>,
    _mode: Mode,
    _count: usize,
    doc: &Doc,
) -> Result<Option<(usize, ColPosition)>> {
    log::info!("move_up {offset} {affinity:?} {horiz:?}");
    let Some((offset_of_buffer, horiz, new_affinity)) = doc
        .lines
        .with_untracked(|x| x.move_up(offset, *affinity, horiz, _mode, _count))?
    else {
        return Ok(None);
    };
    *affinity = new_affinity;
    Ok(Some((offset_of_buffer, horiz)))
}

#[allow(dead_code, unused_variables)]
/// Move down for when the cursor is on the last visual line.
fn move_down_last_rvline(
    _offset: usize,
    affinity: &mut CursorAffinity,
    _horiz: Option<ColPosition>,
    mode: Mode,
) -> Result<(usize, ColPosition)> {
    // let rope_text = rope;
    //
    // let last_line = rope_text.last_line();
    // let new_offset = rope_text.line_end_offset(last_line, mode != Mode::Normal)?;
    //
    // // We should appear after any phantom text at the very end of the line.
    // *affinity = CursorAffinity::Forward;

    // let horiz = horiz.unwrap_or_else(|| {
    //     ColPosition::Col(view.line_point_of_offset(offset, *affinity).x)
    // });
    // let horiz = horiz.unwrap_or_else(|| {
    //     ColPosition::Col(
    //         view.line_point_of_offset(offset, CursorAffinity::Backward)
    //             .map(|x| x.x)
    //             .unwrap_or_default()
    //     )
    // });
    //
    // Ok((new_offset, horiz))

    todo!()
}

/// Move the offset down by `count` amount.
/// `count` may be zero, because moving down in a selection just jumps to the
/// end of the selection.
fn move_down(
    offset: usize,
    affinity: &mut CursorAffinity,
    horiz: Option<ColPosition>,
    _mode: Mode,
    _count: usize,
    doc: &Doc,
) -> Result<Option<(usize, ColPosition)>> {
    // let line = view.doc.with_untracked(|x| x.lines.with_untracked(|x|
    // x.buffer().line_of_offset(offset)));
    //
    // view.visual_lines.with_untracked(|x| {
    //     let prev_line = None;
    //     for visual_line in x {
    //         if let LineTy::OriginText { origin_folded_line_index,
    // line_range_inclusive: line_number } =  visual_line.line_ty {
    // if line < line_number {                 continue;
    //             } else if line == line_number
    //         }
    //     }
    // })

    log::info!("move_down {offset}");
    let Some((offset_of_buffer, horiz, new_affinity)) = doc
        .lines
        .with_untracked(|x| x.move_down(offset, *affinity, horiz, _mode, _count))?
    else {
        return Ok(None);
    };
    *affinity = new_affinity;
    Ok(Some((offset_of_buffer, horiz)))
}

fn document_end(
    rope_text: impl RopeText,
    affinity: &mut CursorAffinity,
    mode: Mode,
) -> Result<(usize, ColPosition)> {
    let last_offset =
        rope_text.offset_line_end(rope_text.len(), mode != Mode::Normal)?;

    // Put it past any inlay hints directly at the end
    *affinity = CursorAffinity::Forward;

    Ok((last_offset, ColPosition::End))
}

fn first_non_blank(
    doc: &Doc,
    affinity: &mut CursorAffinity,
    offset: usize,
) -> Result<(usize, ColPosition)> {
    doc.lines
        .with_untracked(|x| x.first_non_blank(affinity, offset))
}

fn start_of_line(
    _affinity: &mut CursorAffinity,
    _offset: usize,
) -> Result<(usize, ColPosition)> {
    bail!("todo");
    // let start_offset = view.doc().lines.with_untracked(|x| {
    //     let origin_line = x.buffer().line_of_offset(offset);
    //     x.origin_lines
    //         .get(origin_line)
    //         .ok_or(anyhow!("origin_line is empty"))
    //         .map(|x| x.start_offset)
    // })?;
    // Ok((start_offset, ColPosition::Start))
}

fn end_of_line(
    doc: &Doc,
    affinity: &mut CursorAffinity,
    offset: usize,
    mode: Mode,
) -> Result<(usize, ColPosition)> {
    doc.lines
        .with_untracked(|x| x.end_of_line(affinity, offset, mode))
}
#[allow(dead_code, unused_variables)]
fn to_line(
    offset: usize,
    horiz: Option<ColPosition>,
    mode: Mode,
    position: &LinePosition,
) -> (usize, ColPosition) {
    // let rope_text = rope;
    //
    // // TODO(minor): Should this use rvline?
    // let line = match position {
    //     LinePosition::Line(line) => (line - 1).min(rope_text.last_line()),
    //     LinePosition::First => 0,
    //     LinePosition::Last => rope_text.last_line()
    // };
    // // TODO(minor): is this the best affinity?
    // let horiz = horiz.unwrap_or_else(|| {
    //     ColPosition::Col(
    //         view.line_point_of_offset(offset, CursorAffinity::Backward)
    //             .map(|x| x.x)
    //             .unwrap_or_default()
    //     )
    // });
    todo!()
    // let (line, col) = view.line_horiz_col(line, &horiz, mode !=
    // Mode::Normal); let new_offset = rope_text.offset_of_line_col(line,
    // col);
    //
    // (new_offset, horiz)
}

/// Move the current cursor.  
/// This will signal-update the document for some motion modes.
pub fn move_cursor(
    action: &dyn CommonAction,
    cursor: &mut Cursor,
    movement: &Movement,
    count: usize,
    modify: bool,
    register: &mut Register,
    doc: &Doc,
) -> Result<()> {
    let motion_mode = cursor.motion_mode.clone();
    let horiz = cursor.horiz;
    match cursor.mut_mode() {
        CursorMode::Normal(offset) => {
            let count = {
                if let Some(motion_mode) = &motion_mode {
                    count.max(motion_mode.count())
                } else {
                    count
                }
            };
            let offset = *offset;
            let (new_offset, horiz) = move_offset(
                offset,
                horiz.as_ref(),
                &mut cursor.affinity,
                count,
                movement,
                Mode::Normal,
                doc,
            )?;
            if let Some(motion_mode) = &motion_mode {
                let (moved_new_offset, _) = move_offset(
                    new_offset,
                    None,
                    &mut cursor.affinity,
                    1,
                    &Movement::Right,
                    Mode::Insert,
                    doc,
                )?;
                let range = match movement {
                    Movement::EndOfLine | Movement::WordEndForward => {
                        offset..moved_new_offset
                    },
                    Movement::MatchPairs => {
                        if new_offset > offset {
                            offset..moved_new_offset
                        } else {
                            moved_new_offset..new_offset
                        }
                    },
                    _ => offset..new_offset,
                };
                action.exec_motion_mode(
                    cursor,
                    motion_mode.clone(),
                    range,
                    movement.is_vertical(),
                    register,
                );
                cursor.motion_mode = None;
            } else {
                cursor.set_mode(CursorMode::Normal(new_offset));
                cursor.horiz = horiz;
            }
        },
        CursorMode::Visual { start, end, mode } => {
            let start = *start;
            let end = *end;
            let mode = *mode;
            let (new_offset, horiz) = move_offset(
                end,
                horiz.as_ref(),
                &mut cursor.affinity,
                count,
                movement,
                Mode::Visual(VisualMode::Normal),
                doc,
            )?;
            cursor.set_mode(CursorMode::Visual {
                start,
                end: new_offset,
                mode,
            });
            cursor.horiz = horiz;
        },
        CursorMode::Insert(selection) => {
            let selection = selection.clone();
            let selection = move_selection(
                &selection,
                &mut cursor.affinity,
                count,
                modify,
                movement,
                Mode::Insert,
                doc,
            );
            cursor.set_insert(selection?);
        },
    }
    Ok(())
}

pub fn do_multi_selection(
    cursor: &mut Cursor,
    cmd: &MultiSelectionCommand,
    doc: &Doc,
) -> Result<()> {
    use MultiSelectionCommand::*;
    let rope = doc.rope_text();

    match cmd {
        SelectUndo => {
            if let CursorMode::Insert(_) = cursor.mode().clone() {
                if let Some(selection) = cursor.history_selections.last().cloned() {
                    cursor.set_mode(CursorMode::Insert(selection));
                }
                cursor.history_selections.pop();
            }
        },
        InsertCursorAbove => {
            if let CursorMode::Insert(mut selection) = cursor.mode().clone() {
                let offset = selection.first().map(|s| s.end).unwrap_or(0);
                let (new_offset, _) = move_offset(
                    offset,
                    cursor.horiz.as_ref(),
                    &mut cursor.affinity,
                    1,
                    &Movement::Up,
                    Mode::Insert,
                    doc,
                )?;
                if new_offset != offset {
                    selection
                        .add_region(SelRegion::new(new_offset, new_offset, None));
                }
                cursor.set_insert(selection);
            }
        },
        InsertCursorBelow => {
            if let CursorMode::Insert(mut selection) = cursor.mode().clone() {
                let offset = selection.last().map(|s| s.end).unwrap_or(0);
                let (new_offset, _) = move_offset(
                    offset,
                    cursor.horiz.as_ref(),
                    &mut cursor.affinity,
                    1,
                    &Movement::Down,
                    Mode::Insert,
                    doc,
                )?;
                if new_offset != offset {
                    selection
                        .add_region(SelRegion::new(new_offset, new_offset, None));
                }
                cursor.set_insert(selection);
            }
        },
        InsertCursorEndOfLine => {
            if let CursorMode::Insert(selection) = cursor.mode().clone() {
                let mut new_selection = Selection::new();
                for region in selection.regions() {
                    let (start_line, _) = rope.offset_to_line_col(region.min())?;
                    let (end_line, end_col) =
                        rope.offset_to_line_col(region.max())?;
                    for line in start_line..end_line + 1 {
                        let offset = if line == end_line {
                            rope.offset_of_line_col(line, end_col)
                        } else {
                            rope.line_end_offset(line, true)
                        }?;
                        new_selection
                            .add_region(SelRegion::new(offset, offset, None));
                    }
                }
                cursor.set_insert(new_selection);
            }
        },
        SelectCurrentLine => {
            if let CursorMode::Insert(selection) = cursor.mode().clone() {
                let mut new_selection = Selection::new();
                for region in selection.regions() {
                    let start_line = rope.line_of_offset(region.min());
                    let start = rope.offset_of_line(start_line)?;
                    let end_line = rope.line_of_offset(region.max());
                    let end = rope.offset_of_line(end_line + 1)?;
                    new_selection.add_region(SelRegion::new(start, end, None));
                }
                cursor.set_insert(new_selection);
            }
        },
        SelectAllCurrent | SelectNextCurrent | SelectSkipCurrent => {
            // TODO: How should we handle these?
            // The specific common editor behavior is to use the editor's find
            // to do these finds and use it for the selections.
            // However, we haven't included a `find` in floem-editor
        },
        SelectAll => {
            let new_selection = Selection::region(0, rope.len());
            cursor.set_insert(new_selection);
        },
    }
    Ok(())
}

pub fn do_motion_mode(
    action: &dyn CommonAction,
    cursor: &mut Cursor,
    motion_mode: MotionMode,
    register: &mut Register,
) {
    if let Some(cached_motion_mode) = cursor.motion_mode.take() {
        // If it's the same MotionMode discriminant, continue, count is cached in the
        // old motion_mode.
        if core::mem::discriminant(&cached_motion_mode)
            == core::mem::discriminant(&motion_mode)
        {
            let offset = cursor.offset();
            action.exec_motion_mode(
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

// #[cfg(test)]
// mod tests {
//     use std::rc::Rc;

//     use floem_editor_core::{
//         buffer::rope_text::{RopeText, RopeTextVal},
//         cursor::{ColPosition, CursorAffinity},
//         mode::Mode,
//     };
//     use floem_reactive::{Scope, SignalUpdate};
//     use lapce_xi_rope::Rope;
//     use peniko::kurbo::{Rect, Size};

//     use super::Editor;
//     use crate::views::editor::{
//         movement::{correct_crlf, end_of_line, move_down, move_up},
//         text::SimpleStyling,
//         text_document::TextDocument,
//     };

//     fn make_ed(text: &str) -> Editor {
//         // let cx = Scope::new();
//         // let doc = Rc::new(TextDocument::new(cx, text));
//         // let style = Rc::new(SimpleStyling::new());
//         // let editor = Editor::new(cx, doc, style, false);
//         // editor
//         //     .viewport
//         //     .set(Rect::ZERO.with_size(Size::new(f64::MAX, f64::MAX)));
//         // editor
//         todo!()
//     }

//     // Tests for movement logic.
//     // Many of the locations that use affinity are unsure of the specifics,
// and     // should only be assumed to be mostly kinda correct.

//     #[test]
//     fn test_correct_crlf() {
//         let text = Rope::from("hello\nworld");
//         let text = RopeTextVal::new(text);
//         assert_eq!(correct_crlf(&text, 0), 0);
//         assert_eq!(correct_crlf(&text, 5), 5);
//         assert_eq!(correct_crlf(&text, 6), 6);
//         assert_eq!(correct_crlf(&text, text.len()), text.len());

//         let text = Rope::from("hello\r\nworld");
//         let text = RopeTextVal::new(text);
//         assert_eq!(correct_crlf(&text, 0), 0);
//         assert_eq!(correct_crlf(&text, 5), 5);
//         assert_eq!(correct_crlf(&text, 6), 5);
//         assert_eq!(correct_crlf(&text, 7), 7);
//         assert_eq!(correct_crlf(&text, text.len()), text.len());
//     }

//     #[test]
//     fn test_end_of_line() {
//         let ed = make_ed("abc\ndef\nghi");
//         let mut aff = CursorAffinity::Backward;
//         assert_eq!(end_of_line(&ed, &mut aff, 0, Mode::Insert).0, 3);
//         assert_eq!(aff, CursorAffinity::Backward);
//         assert_eq!(end_of_line(&ed, &mut aff, 1, Mode::Insert).0, 3);
//         assert_eq!(aff, CursorAffinity::Backward);
//         assert_eq!(end_of_line(&ed, &mut aff, 3, Mode::Insert).0, 3);
//         assert_eq!(aff, CursorAffinity::Backward);

//         assert_eq!(end_of_line(&ed, &mut aff, 4, Mode::Insert).0, 7);
//         assert_eq!(end_of_line(&ed, &mut aff, 5, Mode::Insert).0, 7);
//         assert_eq!(end_of_line(&ed, &mut aff, 7, Mode::Insert).0, 7);

//         let ed = make_ed("abc\r\ndef\r\nghi");
//         let mut aff = CursorAffinity::Forward;
//         assert_eq!(end_of_line(&ed, &mut aff, 0, Mode::Insert).0, 3);
//         assert_eq!(aff, CursorAffinity::Backward);

//         assert_eq!(end_of_line(&ed, &mut aff, 1, Mode::Insert).0, 3);
//         assert_eq!(aff, CursorAffinity::Backward);
//         assert_eq!(end_of_line(&ed, &mut aff, 3, Mode::Insert).0, 3);
//         assert_eq!(aff, CursorAffinity::Backward);

//         assert_eq!(end_of_line(&ed, &mut aff, 5, Mode::Insert).0, 8);
//         assert_eq!(end_of_line(&ed, &mut aff, 6, Mode::Insert).0, 8);
//         assert_eq!(end_of_line(&ed, &mut aff, 7, Mode::Insert).0, 8);
//         assert_eq!(end_of_line(&ed, &mut aff, 8, Mode::Insert).0, 8);

//         let ed = make_ed("testing\r\nAbout\r\nblah");
//         let mut aff = CursorAffinity::Backward;
//         assert_eq!(end_of_line(&ed, &mut aff, 0, Mode::Insert).0, 7);
//     }

//     #[test]
//     fn test_move_down() {
//         let ed = make_ed("abc\n\n\ndef\n\nghi");

//         let mut aff = CursorAffinity::Forward;

//         assert_eq!(move_down(&ed, 0, &mut aff, None, Mode::Insert, 1).0, 4);

//         let (offset, horiz) = move_down(&ed, 1, &mut aff, None, Mode::Insert,
// 1);         assert_eq!(offset, 4);
//         assert!(matches!(horiz, ColPosition::Col(_)));
//         let (offset, horiz) =
//             move_down(&ed, 4, &mut aff, Some(horiz), Mode::Insert, 1);
//         assert_eq!(offset, 5);
//         assert!(matches!(horiz, ColPosition::Col(_)));
//         let (offset, _) = move_down(&ed, 5, &mut aff, Some(horiz),
// Mode::Insert, 1);         // Moving down with a horiz starting from position
// 1 on first line will put         // cursor at (approximately) position 1 on
// the next line with content         // they arrive at
//         assert_eq!(offset, 7);
//     }

//     #[test]
//     fn test_move_up() {
//         let ed = make_ed("abc\n\n\ndef\n\nghi");

//         let mut aff = CursorAffinity::Forward;

//         assert_eq!(move_up(&ed, 0, &mut aff, None, Mode::Insert, 1).0, 0);

//         let (offset, horiz) = move_up(&ed, 7, &mut aff, None, Mode::Insert,
// 1);         assert_eq!(offset, 5);
//         assert!(matches!(horiz, ColPosition::Col(_)));
//         let (offset, horiz) =
//             move_up(&ed, 5, &mut aff, Some(horiz), Mode::Insert, 1);
//         assert_eq!(offset, 4);
//         assert!(matches!(horiz, ColPosition::Col(_)));
//         let (offset, _) = move_up(&ed, 4, &mut aff, Some(horiz),
// Mode::Insert, 1);         // Moving up with a horiz starting from position 1
// on first line will put         // cursor at (approximately) position 1 on the
// next line with content         // they arrive at
//         assert_eq!(offset, 1);
//     }
// }

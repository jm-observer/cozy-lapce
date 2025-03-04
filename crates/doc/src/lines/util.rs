use std::{collections::HashMap, str::FromStr};
use std::cell::{RefMut};
use floem::{kurbo::Rect, peniko::Color, reactive::SignalGet};
use log::error;

use super::{
    layout::{LineExtraStyle, TextLayout},
    phantom_text::{PhantomText, PhantomTextKind}
};
use crate::{
    EditorViewKind,
    lines::{
        buffer::{Buffer, rope_text::RopeText},
        char_buffer::CharBuffer,
        screen_lines::{ScreenLines, VisualLineInfo},
        text::PreeditData,
        word::WordCursor
    },
    syntax::Syntax
};
use crate::lines::line::OriginFoldedLine;

pub fn compute_screen_lines(
    view_kind: EditorViewKind,
    base: Rect,
    vline_infos: Vec<OriginFoldedLine>,
    line_height: usize,
    y0: f64, buffer_len: usize,
) -> ScreenLines {
    match view_kind {
        EditorViewKind::Normal => {
            let mut visual_lines = Vec::new();

            // let vline_infos = self.visual_lines(min_val, max_val);

            for visual_line in vline_infos {
                let folded_line_y = visual_line.line_index * line_height;
                // let folded_line_y = visual_line_y
                //     - visual_line.origin_folded_line_sub_index * line_height;
                let visual_line_info = VisualLineInfo {
                    folded_line_y: folded_line_y as f64 - y0,
                    // visual_line_y: visual_line_y as f64 - y0,
                    base,
                    visual_line: visual_line,
                };
                visual_lines.push(visual_line_info);
            }
            ScreenLines {
                visual_lines,
                diff_sections: None,
                base,
                line_height: line_height as f64,
                buffer_len,
            }
        },
        EditorViewKind::Diff(_diff_info) => {
            // TODO: let lines in diff view be wrapped, possibly
            // screen_lines should be impl'd
            // on DiffEditorData
            todo!()
            // let mut y_idx = 0;
            // let mut rvlines = Vec::new();
            // let mut info = HashMap::new();
            // let mut diff_sections = Vec::new();
            // let mut last_change: Option<&DiffLines> = None;
            // let mut changes = diff_info.changes.iter().peekable();
            // let is_right = diff_info.is_right;
            //
            // let line_y = |info: VLineInfo<()>, vline_y: usize| ->
            // usize {     vline_y.saturating_sub(info.
            // rvline.line_index * line_height) };
            //
            // while let Some(change) = changes.next() {
            //     match (is_right, change) {
            //         (true, DiffLines::Left(range)) => {
            //             if let Some(DiffLines::Right(_)) =
            // changes.peek() {             } else {
            //                 let len = range.len();
            //                 diff_sections.push(DiffSection {
            //                     y_idx,
            //                     height: len,
            //                     kind: DiffSectionKind::NoCode,
            //                 });
            //                 y_idx += len;
            //             }
            //         }
            //         (false, DiffLines::Right(range)) => {
            //             let len = if let Some(DiffLines::Left(r)) =
            // last_change {                 range.len() -
            // r.len().min(range.len())             } else
            // {                 range.len()
            //             };
            //             if len > 0 {
            //                 diff_sections.push(DiffSection {
            //                     y_idx,
            //                     height: len,
            //                     kind: DiffSectionKind::NoCode,
            //                 });
            //                 y_idx += len;
            //             }
            //         }
            //         (true, DiffLines::Right(range))
            //         | (false, DiffLines::Left(range)) => {
            //             // TODO: count vline count in the range
            // instead             let height =
            // range.len();
            //
            //             diff_sections.push(DiffSection {
            //                 y_idx,
            //                 height,
            //                 kind: if is_right {
            //                     DiffSectionKind::Added
            //                 } else {
            //                     DiffSectionKind::Removed
            //                 },
            //             });
            //
            //             let initial_y_idx = y_idx;
            //             // Mopve forward by the count given.
            //             y_idx += height;
            //
            //             if y_idx < min_vline.get() {
            //                 if is_right {
            //                     if let Some(DiffLines::Left(r)) =
            // last_change {                         //
            // TODO: count vline count in the other editor since this
            // is skipping an amount dependent on those vlines
            //                         let len = r.len() -
            // r.len().min(range.len());
            // if len > 0 {
            // diff_sections.push(DiffSection {
            //                                 y_idx,
            //                                 height: len,
            //                                 kind:
            // DiffSectionKind::NoCode,
            // });                             y_idx +=
            // len;                         }
            //                     };
            //                 }
            //                 last_change = Some(change);
            //                 continue;
            //             }
            //
            //             let start_rvline =
            //                 lines.rvline_of_line(text_prov,
            // range.start);
            //
            //             // TODO: this wouldn't need to produce
            // vlines if screen lines didn't
            // // require them.             let iter =
            // lines                 .iter_rvlines_init(
            //                     text_prov,
            //                     cache_rev,
            //                     config_id,
            //                     start_rvline,
            //                     false,
            //                 )
            //                 .take_while(|vline_info| {
            //                     vline_info.rvline.line < range.end
            //                 })
            //                 .enumerate();
            //             for (i, rvline_info) in iter {
            //                 let rvline = rvline_info.rvline;
            //                 if initial_y_idx + i < min_vline.0 {
            //                     continue;
            //                 }
            //
            //                 rvlines.push(rvline);
            //                 let vline_y = (initial_y_idx + i) *
            // line_height;                 info.insert(
            //                     rvline,
            //                     LineInfo {
            //                         y: line_y(rvline_info, vline_y)
            // as f64 - y0,
            // vline_y: vline_y as f64 - y0,
            // vline_info: rvline_info,
            // },                 );
            //
            //                 if initial_y_idx + i > max_vline.0 {
            //                     break;
            //                 }
            //             }
            //
            //             if is_right {
            //                 if let Some(DiffLines::Left(r)) =
            // last_change {                     // TODO:
            // count vline count in the other editor since this is
            // skipping an amount dependent on those vlines
            //                     let len = r.len() -
            // r.len().min(range.len());
            // if len > 0 {
            // diff_sections.push(DiffSection {
            //                             y_idx,
            //                             height: len,
            //                             kind:
            // DiffSectionKind::NoCode,
            // });                         y_idx += len;
            //                     }
            //                 };
            //             }
            //         }
            //         (_, DiffLines::Both(bothinfo)) => {
            //             let start = if is_right {
            //                 bothinfo.right.start
            //             } else {
            //                 bothinfo.left.start
            //             };
            //             let len = bothinfo.right.len();
            //             let diff_height = len
            //                 - bothinfo .skip .as_ref() .map(|skip|
            //                   skip.len().saturating_sub(1)) .unwrap_or(0);
            //             if y_idx + diff_height < min_vline.get() {
            //                 y_idx += diff_height;
            //                 last_change = Some(change);
            //                 continue;
            //             }
            //
            //             let start_rvline =
            // lines.rvline_of_line(text_prov, start);
            //
            //             let mut iter = lines
            //                 .iter_rvlines_init(
            //                     text_prov,
            //                     cache_rev,
            //                     config_id,
            //                     start_rvline,
            //                     false,
            //                 )
            //                 .take_while(|info| info.rvline.line <
            // start + len);             while let
            // Some(rvline_info) = iter.next() {
            //                 let line = rvline_info.rvline.line;
            //
            //                 // Skip over the lines
            //                 if let Some(skip) =
            // bothinfo.skip.as_ref() {
            // if Some(skip.start) == line.checked_sub(start) {
            //                         y_idx += 1;
            //                         // Skip by `skip` count
            //                         for _ in
            // 0..skip.len().saturating_sub(1) {
            //                             iter.next();
            //                         }
            //                         continue;
            //                     }
            //                 }
            //
            //                 // Add the vline if it is within view
            //                 if y_idx >= min_vline.get() {
            //                     rvlines.push(rvline_info.rvline);
            //                     let vline_y = y_idx * line_height;
            //                     info.insert(
            //                         rvline_info.rvline,
            //                         LineInfo {
            //                             y: line_y(rvline_info,
            // vline_y) as f64 - y0,
            // vline_y: vline_y as f64 - y0,
            // vline_info: rvline_info,
            // },                     );
            //                 }
            //
            //                 y_idx += 1;
            //
            //                 if y_idx - 1 > max_vline.get() {
            //                     break;
            //                 }
            //             }
            //         }
            //     }
            //     last_change = Some(change);
            // }
            // ScreenLines {
            //     lines: Rc::new(rvlines),
            //     info: Rc::new(info),
            //     diff_sections: Some(Rc::new(diff_sections)),
            //     base,
            // }
        }
    }
}

pub fn preedit_phantom(
    preedit: &PreeditData,
    buffer: &Buffer,
    under_line: Option<Color>,
    line: usize
) -> Option<PhantomText> {
    let preedit = preedit.preedit.get_untracked()?;

    let Ok((ime_line, col)) = buffer.offset_to_line_col(preedit.offset) else {
        error!("{}", preedit.offset);
        return None;
    };

    if line != ime_line {
        return None;
    }

    Some(PhantomText {
        kind: PhantomTextKind::Ime,
        line,
        text: preedit.text,
        final_col: col,
        visual_merge_col: col,
        font_size: None,
        fg: None,
        bg: None,
        under_line,
        col,
        origin_merge_col: col,
    })
}

pub fn push_strip_suffix(line_content_original: &str, rs: &mut String) {
    // if let Some(s) = line_content_original.strip_suffix("\r\n") {
    //     rs.push_str(s);
    //     rs.push_str("  ");
    // } else if let Some(s) = line_content_original.strip_suffix('\n') {
    //     rs.push_str(s);
    //     rs.push(' ');
    // } else {
        rs.push_str(line_content_original);
    // }
}

pub fn extra_styles_for_range<'a>(
    text_layout: &'a mut RefMut<TextLayout>,
    start: usize,
    end: usize,
    bg_color: Option<Color>,
    under_line: Option<Color>,
    wave_line: Option<Color>
) -> impl Iterator<Item = LineExtraStyle> + 'a {
    let start_hit = text_layout.hit_position(start);
    let end_hit = text_layout.hit_position(end);

    text_layout
        .layout_runs().enumerate()
        .filter_map(move |(current_line, run)| {
            if current_line < start_hit.line || current_line > end_hit.line {
                return None;
            }

            let x = if current_line == start_hit.line {
                start_hit.point.x
            } else {
                run.glyphs.first().map(|g| g.x).unwrap_or(0.0) as f64
            };
            let end_x = if current_line == end_hit.line {
                end_hit.point.x
            } else {
                run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0) as f64
            };
            let width = end_x - x;

            if width == 0.0 {
                return None;
            }

            let height = (run.max_ascent + run.max_descent) as f64;
            let y = run.line_y as f64 - run.max_ascent as f64;

            Some(LineExtraStyle {
                x,
                y,
                width: Some(width),
                height,
                bg_color,
                under_line,
                wave_line
            })
        })
}

/// Get the previous unmatched character `c` from the `offset` using
/// `syntax` if applicable
pub fn syntax_prev_unmatched(
    buffer: &Buffer,
    syntax: &Syntax,
    c: char,
    offset: usize
) -> Option<usize> {
    if syntax.layers.is_some() {
        syntax.find_tag(offset, true, &CharBuffer::new(c))
    } else {
        WordCursor::new(buffer.text(), offset).previous_unmatched(c)
    }
}

/// If the given character is a parenthesis, returns its matching bracket
pub fn matching_bracket_general<R: ToStaticTextType>(char: char) -> Option<R>
where
    &'static str: ToStaticTextType<R> {
    let pair = match char {
        '{' => "}",
        '}' => "{",
        '(' => ")",
        ')' => "(",
        '[' => "]",
        ']' => "[",
        _ => return None
    };
    Some(pair.to_static())
}

/// If the character is an opening bracket return Some(true), if closing, return
/// Some(false)
pub fn matching_pair_direction(c: char) -> Option<bool> {
    Some(match c {
        '{' => true,
        '}' => false,
        '(' => true,
        ')' => false,
        '[' => true,
        ']' => false,
        _ => return None
    })
}

pub fn matching_char(c: char) -> Option<char> {
    Some(match c {
        '{' => '}',
        '}' => '{',
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        _ => return None
    })
}

pub fn has_unmatched_pair(line: &str) -> bool {
    let mut count = HashMap::new();
    let mut pair_first = HashMap::new();
    for c in line.chars().rev() {
        if let Some(left) = matching_pair_direction(c) {
            let key = if left { c } else { matching_char(c).unwrap() };
            let pair_count = *count.get(&key).unwrap_or(&0i32);
            pair_first.entry(key).or_insert(left);
            if left {
                count.insert(key, pair_count - 1);
            } else {
                count.insert(key, pair_count + 1);
            }
        }
    }
    for (_, pair_count) in count.iter() {
        if *pair_count < 0 {
            return true;
        }
    }
    for (_, left) in pair_first.iter() {
        if *left {
            return true;
        }
    }
    false
}

pub fn str_is_pair_left(c: &str) -> bool {
    if c.chars().count() == 1 {
        let c = c.chars().next().unwrap();
        if matching_pair_direction(c).unwrap_or(false) {
            return true;
        }
    }
    false
}

pub fn str_matching_pair(c: &str) -> Option<char> {
    if c.chars().count() == 1 {
        let c = c.chars().next().unwrap();
        return matching_char(c);
    }
    None
}

pub trait ToStaticTextType<R: 'static = Self>: 'static {
    fn to_static(self) -> R;
}

impl ToStaticTextType for &'static str {
    #[inline]
    fn to_static(self) -> &'static str {
        self
    }
}

impl ToStaticTextType<char> for &'static str {
    #[inline]
    fn to_static(self) -> char {
        char::from_str(self).unwrap()
    }
}

impl ToStaticTextType<String> for &'static str {
    #[inline]
    fn to_static(self) -> String {
        self.to_string()
    }
}

impl ToStaticTextType for char {
    #[inline]
    fn to_static(self) -> char {
        self
    }
}

impl ToStaticTextType for String {
    #[inline]
    fn to_static(self) -> String {
        self
    }
}

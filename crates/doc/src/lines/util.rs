use std::{
    cell::RefMut, collections::HashMap, iter::Peekable, str::FromStr, vec::IntoIter,
};

use floem::{peniko::Color, reactive::SignalGet};
use log::error;
use lsp_types::DocumentHighlight;

use super::{
    layout::{LineExtraStyle, TextLayout},
    phantom_text::{PhantomText, PhantomTextKind},
};
use crate::{
    lines::{
        buffer::{Buffer, rope_text::RopeText},
        char_buffer::CharBuffer,
        text::PreeditData,
        word::WordCursor,
    },
    syntax::Syntax,
};

pub fn preedit_phantom(
    preedit: &PreeditData,
    buffer: &Buffer,
    under_line: Option<Color>,
    line: usize,
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

pub fn preedit_phantom_2(
    preedit: &PreeditData,
    buffer: &Buffer,
    under_line: Option<Color>,
) -> Option<PhantomText> {
    let preedit = preedit.preedit.get_untracked()?;

    let Ok((ime_line, col)) = buffer.offset_to_line_col(preedit.offset) else {
        error!("{}", preedit.offset);
        return None;
    };

    Some(PhantomText {
        kind: PhantomTextKind::Ime,
        line: ime_line,
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

pub fn get_document_highlight(
    changes: &mut Peekable<IntoIter<DocumentHighlight>>,
    start_line: u32,
    end_line: u32,
) -> Vec<DocumentHighlight> {
    let mut highs = Vec::new();
    loop {
        if let Some(high) = changes.peek() {
            if high.range.start.line < start_line {
                changes.next();
                continue;
            } else if start_line <= high.range.start.line
                && high.range.start.line <= end_line
                && start_line <= high.range.end.line
                && high.range.end.line <= end_line
            {
                highs.push(changes.next().unwrap());
                continue;
            } else if end_line < high.range.start.line {
                break;
            }
        } else {
            return highs;
        }
    }
    highs
}

pub fn extra_styles_for_range<'a>(
    text_layout: &'a mut RefMut<TextLayout>,
    start: usize,
    end: usize,
    bg_color: Option<Color>,
    under_line: Option<Color>,
    wave_line: Option<Color>,
    line_height: Option<f64>,
    adjust_y: bool,
) -> impl Iterator<Item = LineExtraStyle> + 'a {
    let start_hit = text_layout.hit_position(start);
    let end_hit = text_layout.hit_position(end);

    text_layout
        .layout_runs()
        .enumerate()
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

            let height =
                line_height.unwrap_or((run.max_ascent + run.max_descent) as f64);
            let y = if adjust_y {
                run.line_y as f64 - run.max_ascent as f64
            } else {
                0.0
            };

            Some(LineExtraStyle {
                x,
                y,
                width: Some(width),
                height,
                bg_color,
                under_line,
                wave_line,
            })
        })
}

/// Get the previous unmatched character `c` from the `offset` using
/// `syntax` if applicable
pub fn syntax_prev_unmatched(
    buffer: &Buffer,
    syntax: &Syntax,
    c: char,
    offset: usize,
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
    &'static str: ToStaticTextType<R>, {
    let pair = match char {
        '{' => "}",
        '}' => "{",
        '(' => ")",
        ')' => "(",
        '[' => "]",
        ']' => "[",
        _ => return None,
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
        _ => return None,
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
        _ => return None,
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

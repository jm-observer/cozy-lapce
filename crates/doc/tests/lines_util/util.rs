use doc::lines::phantom_text::{PhantomTextMultiLine, Text, combine_with_text};
use smallvec::SmallVec;

pub fn check_lines_col(
    lines: &SmallVec<[Text; 6]>,
    final_text_len: usize,
    origin: &str,
    expect: &str
) {
    let rs = combine_with_text(lines, origin);
    assert_eq!(expect, rs.as_str());
    assert_eq!(final_text_len, expect.len());
}

pub fn check_line_final_col(lines: &PhantomTextMultiLine, rs: &str) {
    for text in &lines.text {
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

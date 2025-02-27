use doc::lines::phantom_text::{PhantomTextMultiLine, Text, combine_with_text};
use smallvec::SmallVec;

#[macro_export] macro_rules! check_lines_col {
    ($lines:expr, $final_text_len:expr, $origin:expr, $expect:expr) => {
        let rs = combine_with_text($lines, $origin);
        assert_eq!(rs.as_str(), $expect);
        assert_eq!($final_text_len, $expect.len());
    };
}

#[macro_export] macro_rules! check_line_final_col {
    ($text:expr, $rs:expr) => {
        for text in $text {
        if let Text::Phantom { text } = text {
            assert_eq!(
                text.text.as_str(),
                sub_str($rs, text.final_col, text.final_col + text.text.len())
            );
        }
    }
    };
}

pub fn sub_str(text: &str, begin: usize, end: usize) -> &str {
    unsafe { text.get_unchecked(begin..end) }
}

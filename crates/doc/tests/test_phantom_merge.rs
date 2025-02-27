#![allow(unused)]
use log::{debug, info};
use smallvec::SmallVec;
use doc::lines::cursor::CursorAffinity;
use doc::lines::phantom_text::{PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine, Text, combine_with_text};
use crate::lines_util::*;


mod lines_util;

// fn empty_data() -> PhantomTextLine {
//     let text: SmallVec<[PhantomText; 6]> = SmallVec::new();
//     let origin_text_len = 0;
//     PhantomTextLine::new(6, origin_text_len, 0, text)
// }
#[test]
fn test_merge() {
    custom_utils::logger::logger_stdout_debug();
    _test_merge();
}

/**
 *2 |    if a.0 {...} else {...}
 */
fn _test_merge() {
    let line2 = init_folded_line(2, false);
    let line4 = init_folded_line(4, false);
    let line_folded_4 = init_folded_line(4, true);
    let line6 = init_folded_line(6, false);

    {
        /*
        2 |    if a.0 {...} else {
        */
        let mut lines = PhantomTextMultiLine::new(line2.clone());
        check_lines_col!(
            &lines.text,
            lines.final_text_len,
            "    if true {\r\n",
            "    if true {...}"
        );
        lines.merge(line4);
        // print_lines(&lines);
        check_lines_col!(
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
        check_lines_col!(
            &lines.text,
            lines.final_text_len,
            "    if true {\r\n",
            "    if true {...}"
        );
        // print_lines(&lines);
        // print_line(&line_folded_4);
        lines.merge(line_folded_4);
        // print_lines(&lines);
        check_lines_col!(
            &lines.text,
            lines.final_text_len,
            "    if true {\r\n    } else {\r\n",
            "    if true {...} else {...}"
        );
        lines.merge(line6);
        check_lines_col!(
            &lines.text,
            lines.final_text_len,
            "    if true {\r\n    } else {\r\n    }\r\n",
            "    if true {...} else {...}\r\n"
        );
    }
}


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
                visual_merge_col: 12,
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
                visual_merge_col: 0,
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
                visual_merge_col: 0,
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
                visual_merge_col: 11,
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
                visual_merge_col: 0,
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


fn print_line(lines: &PhantomTextMultiLine) {
    println!(
        "PhantomTextLine line={} origin_text_len={} final_text_len={}",
        lines.line, lines.origin_text_len, lines.final_text_len
    );
    for text in &lines.text {
        match text {
            Text::Phantom { text } => {
                println!(
                    "\tPhantom {:?} line={} col={} merge_col={} final_col={} \
                         text={} text.len()={}",
                    text.kind,
                    text.line,
                    text.col,
                    text.visual_merge_col,
                    text.final_col,
                    text.text,
                    text.text.len()
                );
            },
            Text::OriginText { text } => {
                println!(
                    "\tOriginText line={} col={:?} merge_col={:?} final_col={:?}",
                    text.line, text.col, text.visual_merge_col, text.final_col
                );
            },
            Text::EmptyLine { .. } => {
                println!("\tEmpty");
            }
        }
    }
    println!();
}

use log::{debug, info};
use smallvec::SmallVec;
use doc::lines::cursor::CursorAffinity;
use doc::lines::phantom_text::{PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine, Text, combine_with_text};
use crate::lines_util::*;


mod lines_util;

#[test]
fn test_folded_line_1() {
    custom_utils::logger::logger_stdout_debug();
    let mut _lines = init_main_2().unwrap();
    _lines.update_folding_ranges(folded_v1().into()).unwrap();
    // _lines.update_folding_ranges(folded_v2().into()).unwrap();
    {
        let text_layout = &_lines.folded_line_of_origin_line(1).unwrap().text_layout;
        check_lines_col!(
            &text_layout.phantom_text.text,
            text_layout.phantom_text.final_text_len,
            "    if true {\r\n    } else {\r\n",
            "    if true {...} else {\r\n"
        );
        check_line_final_col!(
            &text_layout.phantom_text,
            "    if true {...} else {\r\n"
        );
    }
    {
        let expect_str = "    let a: A  = A;\r\n";
        let text_layout = &_lines.folded_line_of_origin_line(6).unwrap().text_layout;
        // print_line(&text_layout.phantom_text);
        _lines.log();
        debug!("{:?}", text_layout.phantom_text);
        check_lines_col!(
            &text_layout.phantom_text.text,
            text_layout.phantom_text.final_text_len,
            "    let a = A;\r\n",
            expect_str
        );
        check_line_final_col!(&text_layout.phantom_text, expect_str);
    }
}

#[test]
fn test_folded_line_1_5() {
    let mut _lines = init_main_2().unwrap();
    _lines.update_folding_ranges(folded_v1().into()).unwrap();
    _lines.update_folding_ranges(folded_v2().into()).unwrap();
    {
        let text_layout = &_lines.folded_line_of_origin_line(1).unwrap().text_layout;
        check_lines_col!(
            &text_layout.phantom_text.text,
            text_layout.phantom_text.final_text_len,
            "    if true {\r\n    } else {\r\n    }\r\n",
            "    if true {...} else {...}\r\n"
        );
        check_line_final_col!(
            &text_layout.phantom_text,
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
                merge_col: 12,
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
                merge_col: 0,
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
                merge_col: 0,
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
                merge_col: 11,
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
                merge_col: 0,
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
// "0         10        20        30
// "0123456789012345678901234567890123456789
// "    let a = A;nr
fn let_data() -> PhantomTextLine {
    let mut text: SmallVec<[PhantomText; 6]> = SmallVec::new();
    let origin_text_len = 16;
    text.push(PhantomText {
        kind: PhantomTextKind::InlayHint,
        merge_col: 9,
        line: 6,
        col: 9,
        text: ": A ".to_string(),
        ..Default::default()
    });
    PhantomTextLine::new(6, origin_text_len, 0, text)
}

fn empty_data() -> PhantomTextLine {
    let text: SmallVec<[PhantomText; 6]> = SmallVec::new();
    let origin_text_len = 0;
    PhantomTextLine::new(6, origin_text_len, 0, text)
}
#[test]
fn test_all() {
    custom_utils::logger::logger_stdout_debug();
    _test_merge();
    _check_origin_position_of_final_col();
    _check_col_at();
    _check_final_col_of_col();
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

#[test]
fn check_origin_position_of_final_col() {
    custom_utils::logger::logger_stdout_debug();
    _check_origin_position_of_final_col();
}
fn _check_origin_position_of_final_col() {
    _check_empty_origin_position_of_final_col();
    _check_folded_origin_position_of_final_col();
    _check_let_origin_position_of_final_col();
    _check_folded_origin_position_of_final_col_1();
}
fn _check_let_origin_position_of_final_col() {
    // "0         10        20        30
    // "0123456789012345678901234567890123456789
    // "    let a = A;nr
    // "    let a: A  = A;nr
    // "0123456789012345678901234567890123456789
    // "0         10        20        30
    let let_line = PhantomTextMultiLine::new(let_data());
    let orgin_text: Vec<char> =
        "    let a: A  = A;\r\n".chars().into_iter().collect();
    {
        assert_eq!(orgin_text[8], 'a');
        assert_eq!(let_line.cursor_position_of_final_col(8).1, 8);
    }
    {
        assert_eq!(orgin_text[9], ':');
        let (origin_line, origin_col, _, _, affinity) =
            let_line.cursor_position_of_final_col(11);
        assert_eq!(origin_col, 9);
        assert_eq!(affinity, CursorAffinity::Backward);
        assert_eq!(
            let_line.visual_offset_of_cursor_offset(
                origin_line,
                origin_col,
                affinity
            ),
            Some(9)
        );
    }
    {
        assert_eq!(orgin_text[12], ' ');
        let (origin_line, origin_col, _,_, affinity) =
            let_line.cursor_position_of_final_col(12);
        assert_eq!(origin_col, 9);
        assert_eq!(affinity, CursorAffinity::Forward);
        assert_eq!(
            let_line.visual_offset_of_cursor_offset(
                origin_line,
                origin_col,
                affinity
            ),
            Some(13)
        );
    }
    {
        assert_eq!(orgin_text[17], ';');
        assert_eq!(let_line.cursor_position_of_final_col(17).1, 13);
    }
    {
        assert_eq!(let_line.cursor_position_of_final_col(30).1, 15);
    }
}

fn _check_folded_origin_position_of_final_col_1() {
    //  "0         10        20        30
    //  "0123456789012345678901234567890123456789
    //  "    if true {nr"
    //2 "    } else {nr"
    //  "    if true {...} else {"
    //  "0123456789012345678901234567890123456789
    //  "0         10        20        30
    //              s    e     s    e
    let line = {
        let line2 = init_folded_line(2, false);
        let line_folded_4 = init_folded_line(4, false);
        let mut lines = PhantomTextMultiLine::new(line2);
        lines.merge(line_folded_4);
        lines
    };
    // linesprint_lines(&line);
    let orgin_text: Vec<char> =
        "    if true {...} else {\r\n".chars().into_iter().collect();
    {
        assert_eq!(orgin_text[9], 'u');
        assert_eq!(line.cursor_position_of_final_col(9), (1, 9, 0, 10000, CursorAffinity::Backward));
    }
    {
        let index = 12;
        assert_eq!(orgin_text[index], '{');
        assert_eq!(
            line.cursor_position_of_final_col(index),
            (1, 15, 0, 10000, CursorAffinity::Backward));
    }
    // "0         10        20        30
    // "0123456789012345678901234567890123456789
    // "    if true {nr    } else {nr    }nr"
    {
        let index = 19;
        assert_eq!(orgin_text[index], 'l');
        assert_eq!(line.cursor_position_of_final_col(index), (3, 7, 0, 10000, CursorAffinity::Backward));
    }
    {
        assert_eq!(line.cursor_position_of_final_col(26), (3, 13, 0, 10000, CursorAffinity::Backward));
    }
}

#[test]
fn check_empty_origin_position_of_final_col() {
    custom_utils::logger::logger_stdout_debug();
    _check_empty_origin_position_of_final_col();
}

fn _check_empty_origin_position_of_final_col() {
    let mut _lines = init_main_2().unwrap();
    let line = _lines.folded_line_of_origin_line(28).unwrap();
    info!("{:?}", line);
    {
        assert_eq!(line.text_layout.phantom_text.cursor_position_of_final_col(9), (28, 0, 0, 461, CursorAffinity::Backward));
    }
}

#[test]
fn check_folded_origin_position_of_final_col() {
    custom_utils::logger::logger_stdout_debug();
    _check_folded_origin_position_of_final_col();
}
fn _check_folded_origin_position_of_final_col() {
    //  "0         10        20        30
    //  "0123456789012345678901234567890123456789
    //  "    }nr"
    //2 "    } else {nr    }nr"
    //  "    if true {...} else {...}nr"
    //  "0123456789012345678901234567890123456789
    //  "0         10        20        30
    //              s    e     s    e
    let line = get_merged_data();
    // print_lines(&line);
    info!("{:?}", line);
    let orgin_text: Vec<char> = "    if true {...} else {...}\r\n"
        .chars()
        .into_iter()
        .collect();
    {
        assert_eq!(orgin_text[9], 'u');
        assert_eq!(line.cursor_position_of_final_col(9), (1, 9, 9, 0, CursorAffinity::Backward));
    }
    {
        assert_eq!(orgin_text[0], ' ');
        assert_eq!(line.cursor_position_of_final_col(0), (1, 0, 0, 0, CursorAffinity::Backward));
    }
    {
        let index = 12;
        assert_eq!(orgin_text[index], '{');
        assert_eq!(
            line.cursor_position_of_final_col(index),
            (1, 15, 12, 0, CursorAffinity::Backward));
    }
    // "0         10        20        30
    // "0123456789012345678901234567890123456789
    // "    if true {nr    } else {nr    }nr"
    {
        let index = 19;
        assert_eq!(orgin_text[index], 'l');
        assert_eq!(line.cursor_position_of_final_col(index), (3, 7, 19, 0, CursorAffinity::Backward));
    }
    {
        let index = 25;
        assert_eq!(orgin_text[index], '.');
        assert_eq!(
            line.cursor_position_of_final_col(index),
            (3, 14, 23, 0, CursorAffinity::Backward));
    }
    {
        let index = 28;
        assert_eq!(orgin_text[index], '\r');
        assert_eq!(line.cursor_position_of_final_col(index), (5, 6, 27, 0, CursorAffinity::Forward));
    }

    {
        let index = 40;
        assert_eq!(line.cursor_position_of_final_col(index), (5, 6, 0, 0, CursorAffinity::Backward));
    }
}

fn _check_final_col_of_col() {
    _check_let_final_col_of_col();
    _check_folded_final_col_of_col();
}
fn _check_let_final_col_of_col() {
    let line = PhantomTextMultiLine::new(let_data());
    {
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    let a = A;nr
        // "    let a: A  = A;nr
        // "0123456789012345678901234567890123456789
        // "0         10        20        30
        let orgin_text: Vec<char> =
            "    let a = A;\r\n".chars().into_iter().collect();
        let col_line = 6;
        {
            let index = 8;
            assert_eq!(orgin_text[index], 'a');
            assert_eq!(line.final_col_of_col(col_line, index, true), 8);
            assert_eq!(line.final_col_of_col(col_line, index, false), 9);
        }
        {
            let index = 15;
            assert_eq!(orgin_text[index], '\n');
            assert_eq!(line.final_col_of_col(col_line, index, false), 20);
            assert_eq!(line.final_col_of_col(col_line, index, true), 19);
        }
        {
            let index = 18;
            assert_eq!(line.final_col_of_col(col_line, index, false), 20);
            assert_eq!(line.final_col_of_col(col_line, index, true), 20);
        }
    }
}
fn _check_folded_final_col_of_col() {
    //  "    if true {...} else {...}nr"
    //  "0123456789012345678901234567890123456789
    //  "0         10        20        30
    let line = get_merged_data();
    // print_lines(&line);
    {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //2 "    if true {nr"
        let orgin_text: Vec<char> =
            "    if true {\r\n".chars().into_iter().collect();
        let col_line = 1;
        {
            let index = 9;
            assert_eq!(orgin_text[index], 'u');
            assert_eq!(line.final_col_of_col(col_line, index, true), 9);
            assert_eq!(line.final_col_of_col(col_line, index, false), 10);
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.final_col_of_col(col_line, index, true), 12);
            assert_eq!(line.final_col_of_col(col_line, index, false), 12);
        }
        let col_line = 2;
        {
            let index = 1;
            assert_eq!(line.final_col_of_col(col_line, index, false), 12);
            assert_eq!(line.final_col_of_col(col_line, index, true), 12);
        }
    }
    {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //2 "    } else {nr"
        let orgin_text: Vec<char> =
            "    } else {\r\n".chars().into_iter().collect();
        let col_line = 3;
        {
            let index = 1;
            assert_eq!(orgin_text[index], ' ');
            assert_eq!(line.final_col_of_col(col_line, index, false), 17);
            assert_eq!(line.final_col_of_col(col_line, index, true), 17);
        }
        {
            let index = 8;
            assert_eq!(orgin_text[index], 's');
            assert_eq!(line.final_col_of_col(col_line, index, true), 20);
            assert_eq!(line.final_col_of_col(col_line, index, false), 21);
        }
        {
            let index = 13;
            assert_eq!(orgin_text[index], '\n');
            assert_eq!(line.final_col_of_col(col_line, index, false), 23);
            assert_eq!(line.final_col_of_col(col_line, index, true), 23);
        }
        {
            let index = 18;
            assert_eq!(line.final_col_of_col(col_line, index, false), 23);
            assert_eq!(line.final_col_of_col(col_line, index, true), 23);
        }
    }
    {
        //  "0         10
        //  "0123456789012
        //2 "    }nr"
        let orgin_text: Vec<char> = "    }\r\n".chars().into_iter().collect();
        let col_line = 5;
        {
            let index = 1;
            assert_eq!(orgin_text[index], ' ');
            assert_eq!(line.final_col_of_col(col_line, index, false), 28);
            assert_eq!(line.final_col_of_col(col_line, index, true), 28);
        }
        {
            let index = 6;
            assert_eq!(orgin_text[index], '\n');
            assert_eq!(line.final_col_of_col(col_line, index, true), 29);
            assert_eq!(line.final_col_of_col(col_line, index, false), 30);
        }
        {
            let index = 13;
            assert_eq!(line.final_col_of_col(col_line, index, false), 30);
            assert_eq!(line.final_col_of_col(col_line, index, true), 30);
        }
    }
}

fn _check_col_at() {
    {
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    let a = A;nr
        // let line = PhantomTextMultiLine::new(let_data());
        // line.col_at(8).is_some()
    }
    // "0         10        20        30
    // "0123456789012345678901234567890123456789
    // "    if true {nr    } else {nr    }nr"
    // "    if true {...} else {...}nr"
    // "0123456789012345678901234567890123456789
    // "0         10        20        30
    //              s    e     s    e
    let line = get_merged_data();
    let orgin_text: Vec<char> = "    if true {\r\n    } else {\r\n    }\r\n"
        .chars()
        .into_iter()
        .collect();
    {
        let index = 35;
        assert_eq!(orgin_text[index], '\n');
        assert_eq!(line.final_col_of_merge_col(index).unwrap(), Some(29));
    }
    {
        let index = 26;
        assert_eq!(orgin_text[index], '{');
        assert_eq!(line.final_col_of_merge_col(index).unwrap(), None);
    }
    {
        let index = 22;
        assert_eq!(orgin_text[index], 'l');
        assert_eq!(line.final_col_of_merge_col(index).unwrap(), Some(19));
    }
    {
        assert_eq!(orgin_text[9], 'u');
        assert_eq!(line.final_col_of_merge_col(9).unwrap(), Some(9));
    }
    {
        let index = 12;
        assert_eq!(orgin_text[index], '{');
        assert_eq!(line.final_col_of_merge_col(index).unwrap(), None);
    }
    {
        let index = 19;
        assert_eq!(orgin_text[index], '}');
        assert_eq!(line.final_col_of_merge_col(index).unwrap(), None);
    }
}

/*
2 |    if a.0 {...} else {...}
*/
fn get_merged_data() -> PhantomTextMultiLine {
    let line2 = init_folded_line(2, false);
    let line_folded_4 = init_folded_line(4, true);
    let line6 = init_folded_line(6, false);
    let mut lines = PhantomTextMultiLine::new(line2);
    lines.merge(line_folded_4);
    lines.merge(line6);
    lines
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
                    text.merge_col,
                    text.final_col,
                    text.text,
                    text.text.len()
                );
            },
            Text::OriginText { text } => {
                println!(
                    "\tOriginText line={} col={:?} merge_col={:?} final_col={:?}",
                    text.line, text.col, text.merge_col, text.final_col
                );
            },
            Text::EmptyLine { .. } => {
                println!("\tEmpty");
            }
        }
    }
    println!();
}

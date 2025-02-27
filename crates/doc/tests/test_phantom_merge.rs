#![allow(unused)]
use log::{debug, info};
use smallvec::SmallVec;
use doc::lines::cursor::CursorAffinity;
use doc::lines::phantom_text::{PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine, Text, combine_with_text};
use crate::lines_util::*;
use anyhow::{bail, Result};
use lapce_xi_rope::Interval;

mod lines_util;

// fn empty_data() -> PhantomTextLine {
//     let text: SmallVec<[PhantomText; 6]> = SmallVec::new();
//     let origin_text_len = 0;
//     PhantomTextLine::new(6, origin_text_len, 0, text)
// }
#[test]
fn test_merge() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_merge()?;
    Ok(())
}

/**
 *2 |    if a.0 {...} else {...}
 */
fn _test_merge() -> Result<()> {
    // let line2 = init_folded_line(2, false);
    // let line4 = init_folded_line(4, false);
    // let line_folded_4 = init_folded_line(4, true);
    // let line6 = init_folded_line(6, false);

    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;
    // for item in items {
    //     lines.update_folding_ranges(item.into())?;
    // }
    {
        /*
        2 |    if true {
        */
        let line = lines.folded_line_of_origin_line(1).unwrap();
        debug!("{:?}", lines.origin_lines.get(1).unwrap());
        debug!("{:?}", line);
        assert_eq!(line.len_without_rn(), 13);
        check_lines_col!(
            line.text(),
            line.len(),
            "    if true {\r\n",
            "    if true {\r\n"
        );
        lines.update_folding_ranges(items.get(0).unwrap().clone().into())?;
        let line = lines.folded_line_of_origin_line(1).unwrap();

        let line_1 = lines.origin_lines.get(1).unwrap();
        let line_2 = lines.origin_lines.get(2).unwrap();
        let line_3 = lines.origin_lines.get(3).unwrap();
        debug!("{:?}", line_1);
        debug!("{:?}", line_2);
        debug!("{:?}", line_3);
        debug!("{:?}", line);
        let expect_str = "    if true {...} else {\r\n";
        assert_eq!(line.len(), expect_str.len());
        check_lines_col!(
            line.text(),
            line.len(),
            "    if true {\r\n    } else {\r\n",
            expect_str
        );
        let texts = line.text();
        let text_1 = &texts[1];
        //
        let Text::Phantom {text: text_2 } = &texts[2] else {
            bail!("should be Phantom");
        };
        assert_eq!((text_2.line, text_2.col), (3, 0));
        assert_eq!((text_2.visual_merge_col, text_2.origin_merge_col, text_2.final_col), (line_1.len, line_3.start_offset - line_1.start_offset, "    if true {...}".len()));

        //  else {
        let Text::OriginText {text: text_3 } = &texts[3] else {
            bail!("should be Phantom");
        };
        assert_eq!((text_3.line, text_3.col), (3, Interval::from(5..14)));
        assert_eq!((text_3.visual_merge_col, text_3.origin_merge_col, text_3.final_col)
            , (Interval::from((line_1.len + 5)..(line_1.len + 14))
                , Interval::from((line_1.len + line_2.len + 5)..(line_1.len + line_2.len + 14)), Interval::from(17..26)));



        // // print_lines(&lines);
        // // print_line(&line_folded_4);
        // lines.merge(line_folded_4);
        // // print_lines(&lines);
        // check_lines_col!(
        //     &lines.text,
        //     lines.final_text_len,
        //     "    if true {\r\n    } else {\r\n",
        //     "    if true {...} else {...}"
        // );
        // lines.merge(line6);
        // check_lines_col!(
        //     &lines.text,
        //     lines.final_text_len,
        //     "    if true {\r\n    } else {\r\n    }\r\n",
        //     "    if true {...} else {...}\r\n"
        // );
    }
    Ok(())
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

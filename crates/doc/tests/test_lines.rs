#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{
    buffer::rope_text::RopeText,
    command::EditCommand,
    cursor::{Cursor, CursorAffinity, CursorMode},
    fold::{FoldingDisplayItem, FoldingDisplayType},
    register::Register,
    selection::Selection,
    word::WordCursor
};
use floem::{
    kurbo::{Point, Rect},
    reactive::SignalUpdate
};
use floem::kurbo::Size;
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::info;
use lsp_types::Position;

use crate::lines_util::{
    cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2,
    init_semantic_2
};
mod lines_util;

#[test]
fn test_performance() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let _file: PathBuf = "../../resources/test_code/empty.rs".into();
    let editor: PathBuf = "../../resources/test_code/editor.rs".into();
    let editor_code = std::fs::read_to_string(editor).unwrap();
    let mut lines = init_empty()?;

    lines.init_buffer(editor_code.into())?;
    Ok(())
}

// #[test]
// fn test_debug() -> Result<()> {
//     custom_utils::logger::logger_stdout_debug();
//     let _lines = init_main_2()?;
//     // let text = lines.buffer().text();
//     // let mut cursor = WordCursor::new(text, 5);
//     // let (start, end) = cursor.select_word();
//     //
//     // assert_eq!(text.slice_to_cow(Interval::new(start, end)),
//     // "main");
//     Ok(())
// }

#[test]
fn test_semantic_2() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;

    { // let grammars_dir: PathBuf = "C:\\Users\\36225\\AppData\\Local\\lapce\\Lapce-Debug\\data\\grammars".into();
        // let queries_directory: PathBuf =
        // "C:\\Users\\36225\\AppData\\Roaming\\lapce\\Lapce-Debug\\
        // config\\queries".into(); lines.syntax.parse(lines.
        // buffer().rev(), lines.buffer().text().clone(), None,
        // &grammars_dir, &queries_directory);
        //
        // for style in lines.syntax.styles.as_ref() {
        //     info!("{:?}", style);
        // }
    }
    {
        let line = &lines.origin_lines[1];
        assert!(line.diagnostic_styles.is_empty());
        assert_eq!(line.semantic_styles.len(), 2);

        let line = &lines.origin_lines[3];
        assert_eq!(line.semantic_styles.len(), 1);

        let line = &lines.origin_lines[6];
        assert_eq!(line.diagnostic_styles.len(), 1);
        // for style in &line.semantic_styles {
        //     info!("{:?}", style);
        // }
        assert_eq!(line.semantic_styles[0].origin_line_offset_start, 4);
        assert_eq!(line.semantic_styles[1].origin_line_offset_start, 8);
        assert_eq!(line.semantic_styles[2].origin_line_offset_start, 12);

        assert_eq!(line.semantic_styles.len(), 3);
    }
    {
        //  2|   if true {...} else {\r\n
        lines.update_folding_ranges(folded_v1().into())?;
        let line = &lines.origin_folded_lines[1];
        assert_eq!(line.semantic_styles[0].origin_line_offset_start, 4);
        assert_eq!(line.semantic_styles[1].origin_line_offset_start, 7);
        assert_eq!(line.semantic_styles[2].origin_line_offset_start, 21);
    }
    {
        //  2|   if true {...} else {...}\r\n
        lines.update_folding_ranges(folded_v2().into())?;
        let line = &lines.origin_folded_lines[1];
        assert_eq!(line.semantic_styles[0].origin_line_offset_start, 4);
        assert_eq!(line.semantic_styles[1].origin_line_offset_start, 7);
        assert_eq!(line.semantic_styles[2].origin_line_offset_start, 21);
    }
    Ok(())
}

#[test]
fn test_buffer_offset_of_click() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    // let file: PathBuf = "resources/test_code/main.rs".into();
    let mut lines = init_main()?;
    assert_eq!(lines.line_height, 20);
    lines.log();

    let screen_lines = lines._compute_screen_lines(Rect::from_origin_size((0.0, 0.0), Size::new(300.,300.))).0;

    //below end of buffer
    {
        let (offset_of_buffer, is_inside, affinity) = lines.buffer_offset_of_click(
            &CursorMode::Normal(0),
            Point::new(131.1, 432.1)
        )?;
        assert_eq!((offset_of_buffer, is_inside, affinity), (145, false, CursorAffinity::Backward));

        let (vl, final_offset) = screen_lines.cursor_info_of_buffer_offset(offset_of_buffer, affinity).unwrap().unwrap();
        assert_eq!(vl.visual_line.line_index, 10);
        assert_eq!(final_offset, 0);
    }
    // (line_index=1 offset with \r\n [2..19))
    // new_offset=4 Backward (32.708343505859375, 30.089889526367188)
    // pub [f]n main()
    {
        let (offset_of_buffer, is_inside, affinity) = lines
            .buffer_offset_of_click(&CursorMode::Normal(0), Point::new(32.7, 30.0))?;
        assert_eq!(offset_of_buffer, 6);
        assert_eq!(is_inside, true);
        assert_eq!(affinity, CursorAffinity::Backward);
    }
    // empty of first line(line_index=0)
    // new_offset=0 Forward (109.70834350585938, 11.089889526367188)
    {
        let point = Point::new(109.70834350585938, 11.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        assert_eq!(offset_of_buffer, 0);
        assert_eq!(is_inside, false);
        assert_eq!(affinity, CursorAffinity::Backward);
    }
    // empty of end line(line_index=1 offset with \r\n [2..19))
    // pub fn main() {   [    ]
    // new_offset=16 Forward (176.7, 25.0)
    {
        let point = Point::new(176.7, 25.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        // 16
        assert_eq!(lines.buffer().char_at_offset(offset_of_buffer).unwrap(), '{');
        assert_eq!(is_inside, false);
        assert_eq!(affinity, CursorAffinity::Forward);

        let (visual_line, final_col, ..) =
            lines.folded_line_of_offset(offset_of_buffer, affinity)?;
        // let info = lines.cursor_position_of_buffer_offset(offset_of_buffer, affinity).unwrap();
        assert_eq!(final_col, 15);
    }

    // (line_index=7 offset with \r\n [115, 135))
    //     let a[: A ] = A;
    // first half:  new_offset=124 Backward (72.70834350585938, 150.0898895263672)
    // second half: new_offset=124 Forward (87.70834350585938, 149.0898895263672)
    {
        let point = Point::new(72.7, 150.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        assert_eq!(offset_of_buffer, 124);
        assert_eq!(is_inside, true);
        assert_eq!(affinity, CursorAffinity::Backward);

        let point = Point::new(87.7, 150.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        assert_eq!(offset_of_buffer, 124);
        assert_eq!(is_inside, true);
        assert_eq!(affinity, CursorAffinity::Forward);
    }

    // (line_index=7 offset with \r\n [115, 131))
    //  |    let a: A  = A;      []
    //  |    let a = A;|      []
    {
        let point = Point::new(172.7, 150.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        assert_eq!((offset_of_buffer, is_inside, affinity), (128, false, CursorAffinity::Forward));

        // screen_lines.cursor_position_of_buffer_offset(offset_of_buffer, affinity)
        let (vl, final_offset) = screen_lines.cursor_info_of_buffer_offset(offset_of_buffer, affinity).unwrap().unwrap();
        assert_eq!(vl.visual_line.line_index, 7);
        assert_eq!(final_offset, 18);
    }
    Ok(())
}

#[test]
fn test_buffer_offset_of_click_2() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;

    // scroll 23 line { x0: 0.0, y0: 480.0, x1: 606.8886108398438, y1:
    // 1018.1586303710938 }
    // lines.update_viewport_by_scroll(Rect::new(0.0, 480.0, 606.8, 1018.1));
    //below end of buffer
    {
        // single_click (144.00931549072266, 632.1586074829102)
        // new_offset=480
        let (offset_of_buffer, is_inside, affinity) = lines.buffer_offset_of_click(
            &CursorMode::Normal(0),
            Point::new(186.0, 608.1)
        )?;
        lines.log();
        assert_eq!(offset_of_buffer, 461);
        assert_eq!(is_inside, false);
        assert_eq!(affinity, CursorAffinity::Backward);
    }
    Ok(())
}

#[test]
fn test_buffer_edit() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;

    // info!("enable_error_lens {}", lines.config.enable_error_lens);
    // let start_offset = lines.buffer().offset_of_line(10);
    // let end_offset = lines.buffer().offset_of_line(11);
    // let styles = lines.get_line_diagnostic_styles(start_offset,
    // end_offset, &mut None, 0); info!("{:?}", styles);
    // lines.log();

    info!(
        "{:?} {:?} {:?} {:?}",
        lines.buffer().char_at_offset(181),
        lines.buffer().char_at_offset(182),
        lines.buffer().char_at_offset(183),
        lines.buffer().char_at_offset(184)
    );
    let mut cursor = cursor_insert(139, 139);
    let mut register = Register::default();
    let deltas = lines.do_edit_buffer(
        &mut cursor,
        &EditCommand::InsertNewLine,
        false,
        &mut register,
        true
    )?;

    let mut change_start = usize::MAX;
    let mut change_end = 0;
    for (rope, delta, _inval) in &deltas {
        let mut single_change_start = 0;
        let mut single_change_end = rope.len();
        if let Some(first) = delta.els.first() {
            match first {
                DeltaElement::Copy(start, end) => {
                    if *start == 0 {
                        single_change_start = *end;
                    }
                },
                DeltaElement::Insert(_) => {}
            }
        }
        if let Some(last) = delta.els.last() {
            match last {
                DeltaElement::Copy(start, end) => {
                    if *end == single_change_end {
                        single_change_end = *start;
                    }
                },
                DeltaElement::Insert(_) => {}
            }
        }
        if single_change_start < change_start {
            change_start = single_change_start
        }
        if single_change_end > change_end {
            change_end = single_change_end
        }
    }
    Ok(())
}

fn cursor_normal() -> Cursor {
    let mode = CursorMode::Normal(183);
    Cursor::new(mode, None, None)
}

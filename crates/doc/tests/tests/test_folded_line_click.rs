#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{buffer::rope_text::RopeText, command::EditCommand, cursor::{Cursor, CursorAffinity, CursorMode}, fold::{FoldingDisplayItem, FoldingDisplayType}, register::Register, selection::Selection, word::WordCursor, ClickResult};
use floem::{
    kurbo::{Point, Rect},
    reactive::SignalUpdate
};
use floem::kurbo::Size;
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::{debug, info};
use lsp_types::Position;

use crate::tests::lines_util::{cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2, init_main_folded_item_2, init_semantic_2};

#[test]
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_buffer_offset_of_click()?;
    _test_buffer_offset_of_click_2()?;
    Ok(())
}

#[test]
fn test_buffer_offset_of_click() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_buffer_offset_of_click()?;
    Ok(())
}


pub fn _test_buffer_offset_of_click() -> Result<()> {
    // let file: PathBuf = "resources/test_code/main.rs".into();
    let mut lines = init_main()?;
    assert_eq!(lines.line_height, 20);

    let screen_lines = lines._compute_screen_lines(Rect::from_origin_size((0.0, 0.0), Size::new(300.,300.))).0;
    lines._log_folded_lines();
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
        assert_eq!(lines.buffer().char_at_offset(offset_of_buffer).unwrap(), '\r');
        assert_eq!(is_inside, false);
        assert_eq!(affinity, CursorAffinity::Backward);

        let (_visual_line, final_col, ..) =
            lines.folded_line_of_offset(offset_of_buffer, affinity)?;
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
        assert_eq!(lines.buffer().char_at_offset(offset_of_buffer).unwrap(), '\r');
        assert_eq!((offset_of_buffer, is_inside, affinity), (129, false, CursorAffinity::Backward));

        // screen_lines.cursor_position_of_buffer_offset(offset_of_buffer, affinity)
        let (vl, final_offset) = screen_lines.cursor_info_of_buffer_offset(offset_of_buffer, affinity).unwrap().unwrap();
        assert_eq!(vl.visual_line.line_index, 7);
        assert_eq!(final_offset, 18);
    }
    lines._log_folded_lines();
    Ok(())
}

#[test]
fn test_buffer_offset_of_click_2() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_buffer_offset_of_click_2()?;
    Ok(())
}
pub fn _test_buffer_offset_of_click_2() -> Result<()> {
    let mut lines = init_main_2()?;

    let items = init_main_folded_item_2()?;
    lines.update_folding_ranges(items.get(0).unwrap().clone().into())?;
    let screen_lines = lines._compute_screen_lines(Rect::from_origin_size((0.0, 0.0), Size::new(1000.,800.))).0;
    // lines.log();

    {
        //|    if true {...} else {\r\n  [    ]
        let point = Point::new(252., 25.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        // 16
        assert_eq!(lines.buffer().char_at_offset(offset_of_buffer).unwrap(), '\r');
        assert_eq!((offset_of_buffer, is_inside), (70, false));
        assert_eq!(affinity, CursorAffinity::Backward);
    }
    {
        //|    if true {...} els[]e {\r\n
        let point = Point::new(160., 25.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        assert_eq!(lines.buffer().char_at_offset(offset_of_buffer).unwrap(), 'e');
        assert_eq!((offset_of_buffer, is_inside), (67, true));
        assert_eq!(affinity, CursorAffinity::Backward);

        let point = screen_lines.cursor_position_of_buffer_offset(offset_of_buffer, affinity).unwrap();
        assert_eq!(157, point.unwrap().x as usize);
    }
    {
        //|    if true {..[].} else {\r\n
        let point = Point::new(109.7, 30.0);
        let rs =
            lines.result_of_left_click(point)?;
        assert_eq!(rs, ClickResult::MatchFolded);
    }


    Ok(())
}

#[test]
fn test_buffer_offset_of_click_3() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_buffer_offset_of_click_3()?;
    Ok(())
}

pub fn _test_buffer_offset_of_click_3() -> Result<()> {
    let mut lines = init_main_2()?;

    let items = init_main_folded_item_2()?;
    for item in items {
        lines.update_folding_ranges(item.into())?;
    }
    //  |    let a: A [] = A;
    {
        let point = Point::new(97.7, 49.0);
        let (offset_of_buffer, is_inside, affinity) =
            lines.buffer_offset_of_click(&CursorMode::Normal(0), point)?;
        assert_eq!(lines.buffer().char_at_offset(offset_of_buffer).unwrap(), ' ');
        assert_eq!((offset_of_buffer, is_inside, affinity), (118, true, CursorAffinity::Forward));
    }
    // {
    //     //    if true {...} else {...}|
    //     let point = Point::new(109.7, 30.0);
    //     let rs =
    //         lines.result_of_left_click(point)?;
    //     assert_eq!(rs, ClickResult::);
    // }
    Ok(())
}
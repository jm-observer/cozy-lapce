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
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_buffer_offset_of_click()?;
    _test_buffer_offset_of_click_2()?;
    Ok(())
}

fn _test_buffer_offset_of_click() -> Result<()> {
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

fn _test_buffer_offset_of_click_2() -> Result<()> {
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

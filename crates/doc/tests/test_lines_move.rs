#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{
    buffer::rope_text::RopeText,
    cursor::{Cursor, CursorAffinity, CursorMode},
    fold::{FoldingDisplayItem, FoldingDisplayType},
    selection::Selection,
    word::WordCursor
};
use floem::{
    kurbo::{Point, Rect},
    reactive::SignalUpdate
};
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::info;
use lsp_types::Position;
use doc::lines::cursor::ColPosition;
use doc::lines::mode::Mode;
use crate::lines_util::{cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2, init_main_folded_item_2, init_semantic_2};
mod lines_util;

#[test]
fn test_move_right() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;
    for item in items {
        lines.update_folding_ranges(item.into())?;
    }
    lines._log_folded_lines();
    {
        // |
        // fn test() {...}
        let rs = lines.move_right(139, CursorAffinity::Forward).unwrap().unwrap();
        assert_eq!((lines.buffer().char_at_offset(139).unwrap(), lines.buffer().char_at_offset(141).unwrap()), ('\r', 'f'));
        assert_eq!((141, CursorAffinity::Backward), rs);
    }
    {
        //     if true |{...} else {...}
        let rs = lines.move_right(25, CursorAffinity::Forward).unwrap().unwrap();
        assert_eq!((lines.buffer().char_at_offset(25).unwrap(), lines.buffer().char_at_offset(64).unwrap()), ('{', 'e'));
        assert_eq!(rs, (64, CursorAffinity::Backward));
    }
    {
        // | if true {...} else {...}
        let rs = lines.move_right(16, CursorAffinity::Backward).unwrap().unwrap();
        assert_eq!(lines.buffer().char_at_offset(17).unwrap(), 'i');
        assert_eq!(rs, (17, CursorAffinity::Backward));
    }
    {
        //  if true {...} else {...}|
        let rs = lines.move_right(69, CursorAffinity::Forward).unwrap().unwrap();
        assert_eq!(lines.buffer().char_at_offset(108).unwrap(), '\n');
        assert_eq!(rs, (109, CursorAffinity::Backward));
    }
    // _lines._log_folding_ranges();
    // _lines._log_visual_lines();
    // _lines._log_screen_lines();
    Ok(())
}


#[test]
fn test_move_up() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;
    for item in items {
        lines.update_folding_ranges(item.into())?;
    }
    // lines._log_folded_lines();
    lines.log();
    {
        //    if true {...} []else {...}
        //    let a: A  = A;|
        let rs = lines.move_up(122, CursorAffinity::Forward, None, Mode::Insert, 0).unwrap().unwrap();
        assert_eq!(lines.buffer().char_at_offset(122).unwrap(), ';');
        assert_eq!(rs, (64, ColPosition::Col(18), CursorAffinity::Backward),);
    }
    Ok(())
}



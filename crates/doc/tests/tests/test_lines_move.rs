#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{
    buffer::rope_text::RopeText,
    cursor::{ColPosition, Cursor, CursorAffinity, CursorMode},
    fold::{FoldingDisplayItem, FoldingDisplayType},
    mode::Mode,
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

use super::lines_util::{
    cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2,
    init_main_folded_item_2, init_semantic_2
};

#[test]
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    test_move_right()?;
    test_move_left()?;
    test_move_up()?;
    Ok(())
}
pub fn test_move_right() -> Result<()> {
    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;
    for item in items {
        lines.update_folding_ranges(item.into())?;
    }
    // lines._log_folded_lines();
    {
        // |
        // fn test() {...}
        let rs = lines
            .move_right(139, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                lines.buffer().char_at_offset(139).unwrap(),
                lines.buffer().char_at_offset(141).unwrap()
            ),
            ('\r', 'f')
        );
        assert_eq!((141, CursorAffinity::Backward), rs);
    }
    {
        //
        // fn test() {...}|
        //
        let rs = lines
            .move_right(151, CursorAffinity::Forward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(459).unwrap(), '\r');
        assert_eq!(rs, (461, CursorAffinity::Backward));
    }
    {
        //     if true {...}| else {...}
        let rs = lines
            .move_right(25, CursorAffinity::Forward)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                lines.buffer().char_at_offset(25).unwrap(),
                lines.buffer().char_at_offset(64).unwrap()
            ),
            ('{', 'e')
        );
        assert_eq!(rs, (64, CursorAffinity::Backward));
    }
    {
        // | if true {...} else {...}
        let rs = lines
            .move_right(16, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(17).unwrap(), 'i');
        assert_eq!(rs, (17, CursorAffinity::Backward));
    }
    {
        //  if true {...} else |{...}
        let rs = lines
            .move_right(69, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(69).unwrap(), '{');
        assert_eq!(rs, (69, CursorAffinity::Forward));
    }

    {
        //struct A|;
        let rs = lines
            .move_right(136, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(136).unwrap(), ';');
        assert_eq!(rs, (137, CursorAffinity::Backward));
        //struct A;|
        let rs = lines
            .move_right(137, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(rs, (139, CursorAffinity::Backward));
    }
    // _lines._log_folding_ranges();
    // _lines._log_visual_lines();
    // _lines._log_screen_lines();
    Ok(())
}

pub fn test_move_left() -> Result<()> {
    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;
    for item in items {
        lines.update_folding_ranges(item.into())?;
    }
    // lines._log_folded_lines();
    {
        //
        // |fn test() {...}
        let rs = lines
            .move_left(141, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                lines.buffer().char_at_offset(139).unwrap(),
                lines.buffer().char_at_offset(141).unwrap()
            ),
            ('\r', 'f')
        );
        assert_eq!(rs, (139, CursorAffinity::Backward));
    }
    {
        //     if true {...}| else {...}
        let rs = lines
            .move_left(25, CursorAffinity::Forward)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                lines.buffer().char_at_offset(25).unwrap(),
                lines.buffer().char_at_offset(64).unwrap()
            ),
            ('{', 'e')
        );
        assert_eq!(rs, (25, CursorAffinity::Backward));
    }
    {
        //  if true {...} |else {...}
        let rs = lines
            .move_left(64, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(64).unwrap(), 'e');
        assert_eq!(rs, (25, CursorAffinity::Forward));
    }

    {
        //struct A;|
        let rs = lines
            .move_left(137, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(136).unwrap(), ';');
        assert_eq!(rs, (136, CursorAffinity::Backward));
    }
    {
        //s|truct A;
        let rs = lines
            .move_left(129, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(128).unwrap(), 's');
        assert_eq!(rs, (128, CursorAffinity::Backward));
    }
    // _lines._log_folding_ranges();
    // _lines._log_visual_lines();
    // _lines._log_screen_lines();
    // lines._log_folded_lines();
    Ok(())
}

pub fn test_move_up() -> Result<()> {
    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;
    // for item in items {
    //     lines.update_folding_ranges(item.into())?;
    // }

    // lines._log_folded_lines();
    lines.update_folding_ranges(items.get(0).unwrap().clone().into())?;
    lines.update_folding_ranges(items.get(1).unwrap().clone().into())?;

    // lines.log();
    {
        //    if true {...} []else {...}
        //    let a: A  = A;|
        let rs = lines
            .move_up(122, CursorAffinity::Forward, None, Mode::Insert, 0)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(122).unwrap(), ';');
        assert_eq!(rs, (64, ColPosition::Col(18), CursorAffinity::Backward),);
    }
    {
        //
        //|         empty line
        let rs = lines
            .move_up(461, CursorAffinity::Backward, None, Mode::Insert, 0)
            .unwrap()
            .unwrap();
        assert_eq!(lines.buffer().char_at_offset(122).unwrap(), ';');
        assert_eq!(rs, (458, ColPosition::Col(0), CursorAffinity::Backward),);
    }
    Ok(())
}

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

use crate::lines_util::{
    cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2,
    init_semantic_2
};
mod lines_util;

#[test]
fn test_move_right() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;
    let items = init_item()?;
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
        //     if true {...}| else {...}
        let rs = lines.move_right(25, CursorAffinity::Forward).unwrap().unwrap();
        assert_eq!((lines.buffer().char_at_offset(25).unwrap(), lines.buffer().char_at_offset(64).unwrap()), ('{', 'e'));
        assert_eq!(rs, (64, CursorAffinity::Backward));
    }
    // _lines._log_folding_ranges();
    // _lines._log_visual_lines();
    // _lines._log_screen_lines();
    Ok(())
}

fn init_item() -> Result<Vec<FoldingDisplayItem>> {
    Ok(vec![serde_json::from_str(
        r#"{"position":{"line":1,"character":12},"y":20,"ty":"UnfoldStart"}"#
    )?, serde_json::from_str(
        r#"{"position":{"line":5,"character":5},"y":60,"ty":"UnfoldEnd"}"#
    )?, serde_json::from_str(
        r#"{"position":{"line":10,"character":10},"y":120,"ty":"UnfoldStart"}"#
    )?])
}

#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::{Result, bail};
use doc::{
    EditorViewKind,
    lines::{
        ClickResult,
        buffer::rope_text::RopeText,
        command::EditCommand,
        cursor::{Cursor, CursorAffinity, CursorMode},
        fold::{FoldingDisplayItem, FoldingDisplayType},
        register::Register,
        selection::Selection,
        word::WordCursor,
    },
};
use floem::{
    kurbo::{Point, Rect, Size},
    reactive::SignalUpdate,
};
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::{debug, info};
use lsp_types::Position;

use crate::tests::lines_util::{
    cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2,
    init_main_3, init_main_folded_item_2, init_main_folded_item_3, init_semantic_2,
};

#[test]
fn test_folding() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_folding()?;
    Ok(())
}

pub fn _test_folding() -> Result<()> {
    let mut lines = init_main_2()?;

    let items = init_main_folded_item_2()?;
    // 7|       let a = A;
    assert_eq!(lines.get_folded_index(6), 6);
    // 5|        println!("startss");
    assert_eq!(lines.get_folded_line(4), 4);
    {
        assert_eq!(
            lines
                .folding_ranges
                .get_all_folded_range(lines.buffer())
                .folded_line_count(),
            0
        );
    }
    lines.update_folding_ranges(items.get(1).unwrap().clone().into())?;
    assert_eq!(lines.get_folded_index(6), 4);
    assert_eq!(lines.get_folded_line(4), 3);
    {
        assert_eq!(
            lines
                .folding_ranges
                .get_all_folded_range(lines.buffer())
                .folded_line_count(),
            2
        );
    }
    lines.update_folding_ranges(items.get(0).unwrap().clone().into())?;
    assert_eq!(lines.get_folded_index(6), 2);
    assert_eq!(lines.get_folded_line(4), 1);
    {
        assert_eq!(
            lines
                .folding_ranges
                .get_all_folded_range(lines.buffer())
                .folded_line_count(),
            4
        );
    }
    Ok(())
}

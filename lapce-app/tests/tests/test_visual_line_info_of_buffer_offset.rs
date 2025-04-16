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
fn test_visual_line_info_of_buffer_offset() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_visual_line_info_of_buffer_offset()?;
    Ok(())
}

pub fn _test_visual_line_info_of_buffer_offset() -> Result<()> {
    // let file: PathBuf = "resources/test_code/main.rs".into();
    let mut lines = init_main()?;
    assert_eq!(lines.config.line_height, 20);

    let screen_lines = lines
        .compute_screen_lines_new(
            Rect::from_origin_size((0.0, 0.0), Size::new(1000., 1000.)),
            EditorViewKind::Normal,
        )?
        .0;
    screen_lines.log();
    //below end of buffer
    {
        let Some((_vl, final_cal)) =
            screen_lines.visual_line_info_of_buffer_offset(124)?
        else {
            panic!("should not be none");
        };
        assert_eq!(final_cal, 13);
        // info!("final_cal={final_cal}");
    }
    // lines._log_folded_lines();
    Ok(())
}

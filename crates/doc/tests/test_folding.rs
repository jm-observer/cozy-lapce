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
fn test_debug() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut _lines = init_main_2()?;
    _lines.update_folding_ranges(init()?.into())?;
    _lines._log_folded_lines();
    _lines._log_folding_ranges();
    // _lines._log_visual_lines();
    // _lines._log_screen_lines();
    Ok(())
}

fn init() -> Result<FoldingDisplayItem> {
    Ok(serde_json::from_str(
        r#"{"position":{"line":1,"character":12},"y":23,"ty":"UnfoldStart"}"#
    )?)
}

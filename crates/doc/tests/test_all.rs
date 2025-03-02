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
mod tests;

#[test]
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    tests::test_folded_line_click::_test_buffer_offset_of_click()?;
    tests::test_folded_line_click::_test_buffer_offset_of_click_2()?;
    tests::test_folded_line_click::_test_buffer_offset_of_click_3()?;

    tests::test_line::test_folded()?;

    tests::test_lines_move::test_move_up()?;
    tests::test_lines_move::test_move_right()?;
    tests::test_lines_move::test_move_left()?;

    tests::test_phantom_merge::_test_merge()?;

    Ok(())
}


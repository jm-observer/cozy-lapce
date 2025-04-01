#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{
    buffer::rope_text::RopeText,
    cursor::{ColPosition, Cursor, CursorAffinity, CursorMode},
    fold::{FoldingDisplayItem, FoldingDisplayType},
    mode::Mode,
    selection::Selection,
    word::WordCursor,
};
use floem::{
    kurbo::{Point, Rect},
    reactive::SignalUpdate,
};
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::info;
use lsp_types::Position;

use crate::tests::test_folded_line_click::_test_main_3_buffer_offset_of_click;

mod tests;

#[test]
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    tests::test_folded_line_click::_test_buffer_offset_of_click()?;
    tests::test_folded_line_click::_test_buffer_offset_of_click_2()?;
    tests::test_folded_line_click::_test_buffer_offset_of_click_3()?;
    tests::test_folded_line_click::_test_main_3_buffer_offset_of_click()?;

    tests::test_lines_move::test_move_up()?;
    tests::test_lines_move::test_move_right()?;
    tests::test_lines_move::test_move_left()?;

    tests::test_phantom_merge::_test_merge()?;

    tests::test_diff::_test_screen()?;
    tests::test_diff::_test_changes()?;
    tests::test_diff::_test_1_screen()?;
    tests::test_diff::_test_1_changes()?;

    tests::test_document_symbol::_test_symbol()?;

    tests::test_get_folded_index::_test_folding()?;

    tests::test_cursor_position::_test_cursor_position()?;
    Ok(())
}

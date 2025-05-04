use anyhow::Result;
use doc::{
    EditorViewKind,
    lines::{
        buffer::diff::DiffLines,
        cursor::CursorAffinity,
        diff::{DiffInfo, DiffResult},
    },
};
use floem::{
    kurbo::{Rect, Size},
    prelude::SignalWith,
    reactive::Scope,
};
use lapce_app::panel::document_symbol::{SymbolData, SymbolInformationItemData};
use log::debug;

use crate::tests::lines_util::*;

#[test]
fn test_cursor_selection() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_cursor_selection()?;
    Ok(())
}

pub fn _test_cursor_selection() -> Result<()> {
    let mut lines = init_main()?;
    let screen_lines = lines
        .compute_screen_lines_new(
            Rect::from_origin_size((0.0, 0.0), Size::new(1000., 1000.)),
            EditorViewKind::Normal,
        )?
        .0;

    let x1 = screen_lines
        .normal_selection(
            134,
            143,
            Some(CursorAffinity::Backward),
            Some(CursorAffinity::Backward),
        )?
        .first()
        .unwrap()
        .x1;
    assert_eq!(x1.round(), 68.55f64.round());
    Ok(())
}

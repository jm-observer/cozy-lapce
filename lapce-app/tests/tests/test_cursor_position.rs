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
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    Ok(())
}

pub fn _test_cursor_position() -> Result<()> {
    let mut lines = init_main_2()?;
    let screen_lines = lines
        .compute_screen_lines_new(
            Rect::from_origin_size((0.0, 0.0), Size::new(1000., 1000.)),
            EditorViewKind::Normal,
        )?
        .0;
    {
        // |...
        // |}|
        assert_eq!(lines.buffer().char_at_offset(126), Some('\r'));
        let (text, final_col) = screen_lines
            .cursor_info_of_buffer_offset(126, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(final_col, 2);
    }

    Ok(())
}

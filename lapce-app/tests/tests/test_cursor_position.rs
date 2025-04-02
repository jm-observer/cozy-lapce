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
    _test_cursor_position()?;
    Ok(())
}

pub fn _test_cursor_position() -> Result<()> {
    let mut lines = init_main_2()?;

    let items = init_main_folded_item_2()?;

    let offset = 107;
    {
        let screen_lines = lines
            .compute_screen_lines_new(
                Rect::from_origin_size((0.0, 0.0), Size::new(1000., 1000.)),
                EditorViewKind::Normal,
            )?
            .0;
        // |...
        // |}|
        assert_eq!(lines.buffer().char_at_offset(offset), Some('\r'));
        let (_text, final_col) = screen_lines
            .cursor_info_of_buffer_offset(offset, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(final_col, 5);
    }
    lines.update_folding_ranges(items.get(1).unwrap().clone().into())?;
    {
        let screen_lines = lines
            .compute_screen_lines_new(
                Rect::from_origin_size((0.0, 0.0), Size::new(1000., 1000.)),
                EditorViewKind::Normal,
            )?
            .0;
        // {...}|
        assert_eq!(lines.buffer().char_at_offset(offset), Some('\r'));
        let (_text, final_col) = screen_lines
            .cursor_info_of_buffer_offset(offset, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        assert_eq!(final_col, 16);

        let point = screen_lines
            .cursor_position_of_buffer_offset(offset, CursorAffinity::Backward)
            .unwrap()
            .unwrap();
        println!("{:?}", point);
    }
    Ok(())
}

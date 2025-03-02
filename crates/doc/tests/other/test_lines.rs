#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{
    buffer::rope_text::RopeText,
    command::EditCommand,
    cursor::{Cursor, CursorAffinity, CursorMode},
    fold::{FoldingDisplayItem, FoldingDisplayType},
    register::Register,
    selection::Selection,
    word::WordCursor
};
use floem::{
    kurbo::{Point, Rect},
    reactive::SignalUpdate
};
use floem::kurbo::Size;
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::info;
use lsp_types::Position;

use super::lines_util::{
    cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2,
    init_semantic_2
};

#[test]
fn test_performance() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let _file: PathBuf = "../../resources/test_code/empty.rs".into();
    let editor: PathBuf = "../../resources/test_code/editor.rs".into();
    let editor_code = std::fs::read_to_string(editor).unwrap();
    let mut lines = init_empty()?;

    lines.init_buffer(editor_code.into())?;
    Ok(())
}

// #[test]
// fn test_debug() -> Result<()> {
//     custom_utils::logger::logger_stdout_debug();
//     let _lines = init_main_2()?;
//     // let text = lines.buffer().text();
//     // let mut cursor = WordCursor::new(text, 5);
//     // let (start, end) = cursor.select_word();
//     //
//     // assert_eq!(text.slice_to_cow(Interval::new(start, end)),
//     // "main");
//     Ok(())
// }

#[test]
fn test_semantic_2() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;

    { // let grammars_dir: PathBuf = "C:\\Users\\36225\\AppData\\Local\\lapce\\Lapce-Debug\\data\\grammars".into();
        // let queries_directory: PathBuf =
        // "C:\\Users\\36225\\AppData\\Roaming\\lapce\\Lapce-Debug\\
        // config\\queries".into(); lines.syntax.parse(lines.
        // buffer().rev(), lines.buffer().text().clone(), None,
        // &grammars_dir, &queries_directory);
        //
        // for style in lines.syntax.styles.as_ref() {
        //     info!("{:?}", style);
        // }
    }
    {
        let line = &lines.origin_lines[1];
        assert!(line.diagnostic_styles.is_empty());
        assert_eq!(line.semantic_styles.len(), 2);

        let line = &lines.origin_lines[3];
        assert_eq!(line.semantic_styles.len(), 1);

        let line = &lines.origin_lines[6];
        assert_eq!(line.diagnostic_styles.len(), 1);
        // for style in &line.semantic_styles {
        //     info!("{:?}", style);
        // }
        assert_eq!(line.semantic_styles[0].origin_line_offset_start, 4);
        assert_eq!(line.semantic_styles[1].origin_line_offset_start, 8);
        assert_eq!(line.semantic_styles[2].origin_line_offset_start, 12);

        assert_eq!(line.semantic_styles.len(), 3);
    }
    {
        //  2|   if true {...} else {\r\n
        lines.update_folding_ranges(folded_v1().into())?;
        let line = &lines.origin_folded_lines[1];
        assert_eq!(line.semantic_styles[0].origin_line_offset_start, 4);
        assert_eq!(line.semantic_styles[1].origin_line_offset_start, 7);
        assert_eq!(line.semantic_styles[2].origin_line_offset_start, 21);
    }
    {
        //  2|   if true {...} else {...}\r\n
        lines.update_folding_ranges(folded_v2().into())?;
        let line = &lines.origin_folded_lines[1];
        assert_eq!(line.semantic_styles[0].origin_line_offset_start, 4);
        assert_eq!(line.semantic_styles[1].origin_line_offset_start, 7);
        assert_eq!(line.semantic_styles[2].origin_line_offset_start, 21);
    }
    Ok(())
}

#[test]
fn test_buffer_edit() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut lines = init_main_2()?;

    // info!("enable_error_lens {}", lines.config.enable_error_lens);
    // let start_offset = lines.buffer().offset_of_line(10);
    // let end_offset = lines.buffer().offset_of_line(11);
    // let styles = lines.get_line_diagnostic_styles(start_offset,
    // end_offset, &mut None, 0); info!("{:?}", styles);
    // lines.log();

    info!(
        "{:?} {:?} {:?} {:?}",
        lines.buffer().char_at_offset(181),
        lines.buffer().char_at_offset(182),
        lines.buffer().char_at_offset(183),
        lines.buffer().char_at_offset(184)
    );
    let mut cursor = cursor_insert(139, 139);
    let mut register = Register::default();
    let deltas = lines.do_edit_buffer(
        &mut cursor,
        &EditCommand::InsertNewLine,
        false,
        &mut register,
        true
    )?;

    let mut change_start = usize::MAX;
    let mut change_end = 0;
    for (rope, delta, _inval) in &deltas {
        let mut single_change_start = 0;
        let mut single_change_end = rope.len();
        if let Some(first) = delta.els.first() {
            match first {
                DeltaElement::Copy(start, end) => {
                    if *start == 0 {
                        single_change_start = *end;
                    }
                },
                DeltaElement::Insert(_) => {}
            }
        }
        if let Some(last) = delta.els.last() {
            match last {
                DeltaElement::Copy(start, end) => {
                    if *end == single_change_end {
                        single_change_end = *start;
                    }
                },
                DeltaElement::Insert(_) => {}
            }
        }
        if single_change_start < change_start {
            change_start = single_change_start
        }
        if single_change_end > change_end {
            change_end = single_change_end
        }
    }
    Ok(())
}

fn cursor_normal() -> Cursor {
    let mode = CursorMode::Normal(183);
    Cursor::new(mode, None, None)
}

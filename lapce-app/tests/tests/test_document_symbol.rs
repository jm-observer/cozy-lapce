use anyhow::Result;
use doc::lines::{
    buffer::diff::DiffLines,
    diff::{DiffInfo, DiffResult},
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
fn test_symbol() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_symbol()?;
    Ok(())
}

pub fn _test_symbol() -> Result<()> {
    let symbols = _init_lsp_document_symbol();
    let cx = Scope::new();
    for symbol in &symbols {
        debug!("{symbol:?}");
    }
    let items = symbols
        .into_iter()
        .map(|x| cx.create_rw_signal(SymbolInformationItemData::from((x, cx))))
        .collect();
    let symbol_new = SymbolData::new(items, "path".into(), cx);
    // let a = symbol_new
    //     .file
    //     .with_untracked(|x| x.find_by_name("B"))
    //     .unwrap();
    // debug!("{:?}", a);
    assert_eq!(Some(2), symbol_new.match_line_with_children(1).line_index());
    assert_eq!(
        Some(3),
        symbol_new.match_line_with_children(12).line_index()
    );
    assert_eq!(
        Some(4),
        symbol_new.match_line_with_children(18).line_index()
    );

    // will open A symbol
    assert_eq!(Some(6), symbol_new.match_line_with_children(5).line_index());

    assert_eq!(
        Some(12),
        symbol_new.match_line_with_children(12).line_index()
    );

    Ok(())
}

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
    Ok(())
}

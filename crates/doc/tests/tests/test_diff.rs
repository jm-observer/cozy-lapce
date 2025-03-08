use anyhow::Result;
use floem::kurbo::{Rect, Size};
use log::debug;
use doc::lines::buffer::diff::DiffLines;
use doc::lines::diff::{DiffInfo, DiffResult};
use crate::tests::lines_util::{init_diff, init_test};

#[test]
fn test_changes() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_changes()?;
    Ok(())
}

#[test]
fn test_screen() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_screen()?;
    Ok(())
}

pub fn _test_screen() -> Result<()> {
    let mut lines = init_test()?;
    let screen_lines = lines
        ._compute_screen_lines(Rect::from_origin_size(
            (0.0, 0.0),
            Size::new(1000., 800.)
        ))
        .0;

    for line in screen_lines.visual_lines {
        debug!("{:?}", line);
    }

    Ok(())
}

pub fn _test_changes() -> Result<()> {
    let diff = init_diff()?;
    let left_changes: Vec<DiffResult> = serde_json::from_str(r#"[{"Changed":{"lines":{"start":6,"end":10}}},{"Changed":{"lines":{"start":11,"end":13}}},{"Empty":{"lines":{"start":15,"end":19}}},{"Changed":{"lines":{"start":18,"end":19}}}]"#)?;
    let right_changes: Vec<DiffResult> = serde_json::from_str(r#"[{"Empty":{"lines":{"start":6,"end":10}}},{"Changed":{"lines":{"start":7,"end":9}}},{"Changed":{"lines":{"start":11,"end":15}}},{"Empty":{"lines":{"start":18,"end":19}}}]"#)?;

    let tys = diff.left_changes();
    // debug!("{}", serde_json::to_string(&tys)?);
    assert_eq!(tys, left_changes);
    let tys = diff.right_changes();
    debug!("{}", serde_json::to_string(&tys)?);
    assert_eq!(tys, right_changes);
    Ok(())
}





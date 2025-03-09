use anyhow::Result;
use floem::kurbo::{Rect, Size};
use log::debug;
use doc::lines::buffer::diff::DiffLines;
use doc::lines::diff::{DiffInfo, DiffResult};
use crate::tests::lines_util::{init_diff, init_test, init_test_diff};

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
    let  (mut left_lines, _) = init_test()?;
    let screen_lines = left_lines
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
    let diff = init_test_diff();
    debug!("{}", serde_json::to_string(&diff)?);
    let diff = DiffInfo {
        is_right: false,
        changes: diff.clone(),
    };
    let left_changes: Vec<DiffResult> = serde_json::from_str(r#"[{"Changed":{"lines":{"start":3,"end":10}}},{"Changed":{"lines":{"start":17,"end":18}}},{"Empty":{"lines":{"start":18,"end":20}}}]"#)?;
    let right_changes: Vec<DiffResult> = serde_json::from_str(r#"[{"Changed":{"lines":{"start":3,"end":4}}},{"Empty":{"lines":{"start":4,"end":10}}},{"Changed":{"lines":{"start":11,"end":14}}}]"#)?;

    let tys = diff.left_changes();
    // debug!("{}", serde_json::to_string(&tys)?);
    assert_eq!(tys, left_changes);
    let tys = diff.right_changes();
    debug!("{}", serde_json::to_string(&tys)?);
    assert_eq!(tys, right_changes);
    Ok(())
}





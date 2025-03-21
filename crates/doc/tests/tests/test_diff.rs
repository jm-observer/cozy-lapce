use anyhow::Result;
use doc::lines::{
    buffer::diff::DiffLines,
    diff::{DiffInfo, DiffResult},
};
use floem::kurbo::{Rect, Size};
use log::debug;

use crate::tests::lines_util::*;

#[test]
fn test_changes() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_changes()?;
    _test_1_changes()?;
    Ok(())
}

#[test]
fn test_screen() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_screen()?;
    _test_1_screen()?;
    Ok(())
}

pub fn _test_1_screen() -> Result<()> {
    let (_left_lines, mut right_lines, _left_kind, right_kind) = init_test_1()?;

    let screen_lines = right_lines
        .compute_screen_lines_new(
            Rect::from_origin_size((0.0, 0.0), Size::new(1000., 800.)),
            right_kind.clone(),
        )?
        .0;
    let visual_lines = &screen_lines.visual_lines;
    // for line in screen_lines.visual_lines {
    //     debug!("{:?}", line);
    // }
    assert!(
        visual_lines[1].is_diff_delete()
            && !visual_lines[2].is_diff_delete()
            && visual_lines[3].is_diff_delete()
            && visual_lines[14].is_diff_delete()
            && !visual_lines[15].is_diff_delete()
    );
    Ok(())
}

pub fn _test_screen() -> Result<()> {
    let (mut left_lines, _, left_kind, _) = init_test()?;

    let screen_lines = left_lines
        .compute_screen_lines_new(
            Rect::from_origin_size((0.0, 0.0), Size::new(1000., 800.)),
            left_kind.clone(),
        )?
        .0;
    let visual_lines = &screen_lines.visual_lines;
    assert!(
        visual_lines[3].is_diff()
            && visual_lines[9].is_diff()
            && visual_lines[17].is_diff()
            && !visual_lines[10].is_diff()
    );
    assert!(
        visual_lines[18].is_diff_delete()
            && visual_lines[19].is_diff_delete()
            && !visual_lines[20].is_diff_delete()
    );
    let screen_lines = left_lines
        .compute_screen_lines_new(
            Rect::from_origin_size((0.0, 60.0), Size::new(1000., 800.)),
            left_kind,
        )?
        .0;
    let visual_lines = &screen_lines.visual_lines;
    assert!(
        visual_lines[0].is_diff()
            && visual_lines[6].is_diff()
            && visual_lines[14].is_diff()
            && !visual_lines[7].is_diff()
    );
    assert!(
        visual_lines[15].is_diff_delete()
            && visual_lines[16].is_diff_delete()
            && !visual_lines[17].is_diff_delete()
    );
    for line in &screen_lines.visual_lines {
        debug!("{:?}", line);
    }

    Ok(())
}

pub fn _test_changes() -> Result<()> {
    let diff = init_test_diff();
    debug!("{}", serde_json::to_string(&diff)?);
    let diff = DiffInfo {
        is_right: false,
        changes:  diff.clone(),
    };
    let left_changes: Vec<DiffResult> = serde_json::from_str(
        r#"[{"Changed":{"lines":{"start":3,"end":10}}},{"Changed":{"lines":{"start":17,"end":18}}},{"Empty":{"lines":{"start":18,"end":20}}}]"#,
    )?;
    let right_changes: Vec<DiffResult> = serde_json::from_str(
        r#"[{"Changed":{"lines":{"start":3,"end":4}}},{"Empty":{"lines":{"start":4,"end":10}}},{"Changed":{"lines":{"start":11,"end":14}}}]"#,
    )?;

    let tys = diff.left_changes();
    // debug!("{}", serde_json::to_string(&tys)?);
    assert_eq!(tys, left_changes);
    let tys = diff.right_changes();
    debug!("{}", serde_json::to_string(&tys)?);
    assert_eq!(tys, right_changes);
    Ok(())
}

pub fn _test_1_changes() -> Result<()> {
    let diff = init_test_1_diff();
    debug!("{}", serde_json::to_string(&diff)?);
    let diff = DiffInfo {
        is_right: false,
        changes:  diff.clone(),
    };
    let tys = diff.left_changes();
    debug!("{}", serde_json::to_string(&tys)?);
    let left_changes: Vec<DiffResult> = serde_json::from_str(
        r#"[{"Changed":{"lines":{"start":1,"end":2}}},{"Changed":{"lines":{"start":3,"end":15}}},{"Empty":{"lines":{"start":19,"end":23}}}]"#,
    )?;
    assert_eq!(tys, left_changes);
    let tys = diff.right_changes();
    debug!("{}", serde_json::to_string(&tys)?);
    let right_changes: Vec<DiffResult> = serde_json::from_str(
        r#"[{"Empty":{"lines":{"start":1,"end":2}}},{"Empty":{"lines":{"start":2,"end":14}}},{"Changed":{"lines":{"start":6,"end":10}}}]"#,
    )?;
    assert_eq!(tys, right_changes);
    Ok(())
}

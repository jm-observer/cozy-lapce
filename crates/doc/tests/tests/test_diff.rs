use anyhow::Result;
use log::debug;
use doc::lines::buffer::diff::DiffLines;
use doc::lines::diff::DiffInfo;
use crate::tests::lines_util::init_test;

#[test]
fn test_left_changes() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    _test_left_changes()?;
    Ok(())
}

fn _test_left_changes() -> Result<()> {

    let diff = init_diff()?;
    // let lines = init_test()?;
    let tys = diff.left_changes();
    // [{"Delete":{"line":0}},{"Delete":{"line":0}}]
    debug!("{tys:?}");

    let tys = diff.right_changes();
    debug!("{tys:?}");
    Ok(())
}



fn init_diff() -> Result<DiffInfo> {
    let changes = r#"[{"Both":{"left":{"start":0,"end":6},"right":{"start":0,"end":6},"skip":{"start":0,"end":3}}},{"Left":{"start":6,"end":10}},{"Both":{"left":{"start":10,"end":11},"right":{"start":6,"end":7},"skip":null}},{"Left":{"start":11,"end":13}},{"Right":{"start":7,"end":9}},{"Both":{"left":{"start":13,"end":15},"right":{"start":9,"end":11},"skip":null}},{"Right":{"start":11,"end":15}},{"Both":{"left":{"start":15,"end":18},"right":{"start":15,"end":18},"skip":null}},{"Left":{"start":18,"end":19}}]"#;
    let changes: Vec<DiffLines> = serde_json::from_str(changes)?;
    Ok(DiffInfo {
        is_right: false,
        changes,
    })
}
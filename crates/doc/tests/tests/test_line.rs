#![allow(unused_imports, dead_code, unused_mut)]

use std::{path::PathBuf, sync::atomic};

use anyhow::Result;
use doc::lines::{buffer::rope_text::RopeText, cursor::{Cursor, CursorAffinity, CursorMode}, fold::{FoldingDisplayItem, FoldingDisplayType}, selection::Selection, word::WordCursor, DocLines};
use floem::{
    kurbo::{Point, Rect},
    reactive::SignalUpdate
};
use lapce_xi_rope::{DeltaElement, Interval, RopeInfo, spans::SpansBuilder};
use log::{debug, info};
use lsp_types::Position;
use smallvec::SmallVec;
use doc::lines::cursor::ColPosition;
use doc::lines::line::OriginLine;
use doc::lines::mode::Mode;
use doc::lines::phantom_text::Text;
use super::lines_util::{cursor_insert, folded_v1, folded_v2, init_empty, init_main, init_main_2, init_main_folded_item_2, init_semantic_2};

#[test]
fn test_all() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    test_folded()
}
pub fn test_folded() -> Result<()> {
    let mut lines = init_main_2()?;
    let items = init_main_folded_item_2()?;

    {
        let line = de_serde_origin_line();
        assert_eq!(lines.origin_lines[1].phantom.texts, line[0].phantom.texts);
        assert_eq!(lines.origin_lines[3].phantom.texts, line[1].phantom.texts);
        assert_eq!(lines.origin_lines[5].phantom.texts, line[2].phantom.texts);
    }
    lines.update_folding_ranges(items.get(0).unwrap().clone().into())?;
    {
        let line = de_serde_origin_line_1();
        assert_eq!(lines.origin_lines[1].phantom.texts, line[0].phantom.texts);
        assert_eq!(lines.origin_lines[3].phantom.texts, line[1].phantom.texts);
        assert_eq!(lines.origin_lines[5].phantom.texts, line[2].phantom.texts);
    }
    {
        debug!("{}", serde_json::to_string(lines.origin_folded_lines.get(1).unwrap()).unwrap());
    }
    lines.update_folding_ranges(items.get(1).unwrap().clone().into())?;
    {
        let line = de_serde_origin_line_2();
        assert_eq!(lines.origin_lines[1].phantom.texts, line[0].phantom.texts);
        assert_eq!(lines.origin_lines[3].phantom.texts, line[1].phantom.texts);
        assert_eq!(lines.origin_lines[5].phantom.texts, line[2].phantom.texts);
    }
    {
        // debug!("{}", serde_json::to_string(lines.origin_folded_lines.get(1).unwrap()).unwrap());
        assert_eq!(lines.origin_folded_lines[1].text(), de_serde_folded_text());
    }
    {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //  "    if true {nr"
        //2 "    } else {nr"
        //  "    if true {...} else {"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        //     if true |{...} else {...}
        // let line = lines.origin_lines.get(1).unwrap();
    }
    Ok(())
}


fn get_origin_line(lines: &DocLines, index: usize) -> String {
    let mut origin_line = lines.origin_lines.get(index).unwrap().clone();
    origin_line.diagnostic_styles.clear();
    origin_line.semantic_styles.clear();
    serde_json::to_string(&origin_line).unwrap()
}

fn de_serde_origin_line() -> Vec<OriginLine> {
    let str =
        [r#"{"line_index":1,"start_offset":13,"len":15,"phantom":{"line":1,"offset_of_line":13,"origin_text_len":15,"final_text_len":15
            ,"texts":[{"OriginText":{"text":{"line":1,"col":{"start":0,"end":15},"visual_merge_col":{"start":0,"end":15},"origin_merge_col":{"start":0,"end":15},"final_col":{"start":0,"end":15}}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#,
        r#"{"line_index":3,"start_offset":58,"len":14,"phantom":{"line":3,"offset_of_line":58,"origin_text_len":14,"final_text_len":14
            ,"texts":[{"OriginText":{"text":{"line":3,"col":{"start":0,"end":14},"visual_merge_col":{"start":0,"end":14},"origin_merge_col":{"start":0,"end":14},"final_col":{"start":0,"end":14}}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#,
        r#"{"line_index":5,"start_offset":102,"len":7,"phantom":{"line":5,"offset_of_line":102,"origin_text_len":7,"final_text_len":7
            ,"texts":[{"OriginText":{"text":{"line":5,"col":{"start":0,"end":7},"visual_merge_col":{"start":0,"end":7},"origin_merge_col":{"start":0,"end":7},"final_col":{"start":0,"end":7}}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#];
    let mut lines = vec![];
    for str in str {
        lines.push(serde_json::from_str::<OriginLine>(str).unwrap());
    }
    lines
}

fn de_serde_origin_line_1() -> Vec<OriginLine> {
    let str =
        [r#"{"line_index":1,"start_offset":13,"len":15,"phantom":{"line":1,"offset_of_line":13,"origin_text_len":15,"final_text_len":17,"texts":[
                {"OriginText":{"text":{"line":1,"col":{"start":0,"end":12},"visual_merge_col":{"start":0,"end":12},"origin_merge_col":{"start":0,"end":12},"final_col":{"start":0,"end":12}}}}
                ,{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":3,"len":3,"start_position":{"line":1,"character":12}}},"line":1,"col":12,"visual_merge_col":12,"origin_merge_col":12,"final_col":12,"affinity":null,"text":"{...}","font_size":13,"fg":{"components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"bg":{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"under_line":null}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#,
            r#"{"line_index":3,"start_offset":58,"len":14,"phantom":{"line":3,"offset_of_line":58,"origin_text_len":14,"final_text_len":9,"texts":[
                {"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":null,"len":5,"start_position":{"line":1,"character":12}}},"line":3,"col":0,"visual_merge_col":0,"origin_merge_col":0,"final_col":0,"affinity":null,"text":"","font_size":null,"fg":null,"bg":null,"under_line":null}}}
                ,{"OriginText":{"text":{"line":3,"col":{"start":5,"end":14},"visual_merge_col":{"start":5,"end":14},"origin_merge_col":{"start":5,"end":14},"final_col":{"start":0,"end":9}}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#,
            r#"{"line_index":5,"start_offset":102,"len":7,"phantom":{"line":5,"offset_of_line":102,"origin_text_len":7,"final_text_len":7,"texts":[
                {"OriginText":{"text":{"line":5,"col":{"start":0,"end":7},"visual_merge_col":{"start":0,"end":7},"origin_merge_col":{"start":0,"end":7},"final_col":{"start":0,"end":7}}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#];
    let mut lines = vec![];
    for str in str {
        lines.push(serde_json::from_str::<OriginLine>(str).unwrap());
    }
    lines
}

fn de_serde_origin_line_2() -> Vec<OriginLine> {
    let str =
        [r#"{"line_index":1,"start_offset":13,"len":15,"phantom":{"line":1,"offset_of_line":13,"origin_text_len":15,"final_text_len":17
                ,"texts":[{"OriginText":{"text":{"line":1,"col":{"start":0,"end":12},"visual_merge_col":{"start":0,"end":12},"origin_merge_col":{"start":0,"end":12},"final_col":{"start":0,"end":12}}}}
                ,{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":3,"len":3,"start_position":{"line":1,"character":12}}},"line":1,"col":12,"visual_merge_col":12,"origin_merge_col":12,"final_col":12,"affinity":null,"text":"{...}","font_size":13,"fg":{"components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"bg":{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"under_line":null}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#,
            r#"{"line_index":3,"start_offset":58,"len":14,"phantom":{"line":3,"offset_of_line":58,"origin_text_len":14,"final_text_len":11
                ,"texts":[{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":null,"len":5,"start_position":{"line":1,"character":12}}},"line":3,"col":0,"visual_merge_col":0,"origin_merge_col":0,"final_col":0,"affinity":null,"text":"","font_size":null,"fg":null,"bg":null,"under_line":null}}}
                ,{"OriginText":{"text":{"line":3,"col":{"start":5,"end":11},"visual_merge_col":{"start":5,"end":11},"origin_merge_col":{"start":5,"end":11},"final_col":{"start":0,"end":6}}}}
                ,{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":5,"len":3,"start_position":{"line":3,"character":11}}},"line":3,"col":11,"visual_merge_col":11,"origin_merge_col":11,"final_col":6,"affinity":null,"text":"{...}","font_size":13,"fg":{"components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"bg":{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"under_line":null}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#,
            r#"{"line_index":5,"start_offset":102,"len":7,"phantom":{"line":5,"offset_of_line":102,"origin_text_len":7,"final_text_len":2
                ,"texts":[{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":null,"len":5,"start_position":{"line":3,"character":11}}},"line":5,"col":0,"visual_merge_col":0,"origin_merge_col":0,"final_col":0,"affinity":null,"text":"","font_size":null,"fg":null,"bg":null,"under_line":null}}}
                ,{"OriginText":{"text":{"line":5,"col":{"start":5,"end":7},"visual_merge_col":{"start":5,"end":7},"origin_merge_col":{"start":5,"end":7},"final_col":{"start":0,"end":2}}}}]},"semantic_styles":[],"diagnostic_styles":[]}"#];
    let mut lines = vec![];
    for str in str {
        lines.push(serde_json::from_str::<OriginLine>(str).unwrap());
    }
    lines
}

fn de_serde_folded_text() -> Vec<Text> {
    let str = r#"[{"OriginText":{"text":{"line":1,"col":{"start":0,"end":12},"visual_merge_col":{"start":0,"end":12},"origin_merge_col":{"start":0,"end":12},"final_col":{"start":0,"end":12}}}},{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":3,"len":3,"start_position":{"line":1,"character":12}}},"line":1,"col":12,"visual_merge_col":12,"origin_merge_col":12,"final_col":12,"affinity":null,"text":"{...}","font_size":13,"fg":{"components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"bg":{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"under_line":null}}},{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":null,"len":5,"start_position":{"line":1,"character":12}}},"line":3,"col":0,"visual_merge_col":15,"origin_merge_col":45,"final_col":17,"affinity":null,"text":"","font_size":null,"fg":null,"bg":null,"under_line":null}}},{"OriginText":{"text":{"line":3,"col":{"start":5,"end":11},"visual_merge_col":{"start":20,"end":26},"origin_merge_col":{"start":50,"end":56},"final_col":{"start":17,"end":23}}}},{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":5,"len":3,"start_position":{"line":3,"character":11}}},"line":3,"col":11,"visual_merge_col":26,"origin_merge_col":56,"final_col":23,"affinity":null,"text":"{...}","font_size":13,"fg":{"components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"bg":{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"under_line":null}}},{"Phantom":{"text":{"kind":{"LineFoldedRang":{"next_line":null,"len":5,"start_position":{"line":3,"character":11}}},"line":5,"col":0,"visual_merge_col":29,"origin_merge_col":89,"final_col":28,"affinity":null,"text":"","font_size":null,"fg":null,"bg":null,"under_line":null}}},{"OriginText":{"text":{"line":5,"col":{"start":5,"end":7},"visual_merge_col":{"start":34,"end":36},"origin_merge_col":{"start":94,"end":96},"final_col":{"start":28,"end":30}}}}]"#;
    serde_json::from_str::<Vec<Text>>(str).unwrap()
}
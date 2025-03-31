use std::path::PathBuf;

use doc::lines::{
    DocLines,
    buffer::rope_text::RopeText,
    fold::{FoldingDisplayItem, FoldingDisplayType},
};
use floem::prelude::SignalUpdate;
use lapce_xi_rope::{Interval, spans::SpansBuilder};
use lsp_types::{Diagnostic, FoldingRange};

use super::{_init_code, _init_inlay_hint, _init_lines};
use crate::tests::lines_util::{_init, SemanticStyles};

fn _init_lsp_folding_range_2() -> Vec<FoldingRange> {
    // let folding_range =
    // r#"[{"startLine":0,"startCharacter":10,"endLine":7,"endCharacter":1},{"
    // startLine":1,"startCharacter":12,"endLine":3,"endCharacter":5}
    //  ,{"startLine":3,"startCharacter":11,"endLine":5,"endCharacter":5},{"
    // startLine":10,"startCharacter":10,"endLine":27,"endCharacter":1}]"#;
    // let folding_range: Vec<lsp_types::FoldingRange> =
    //     serde_json::from_str(folding_range).unwrap();

    // folding_range
    //     .into_iter()
    //     .map(FoldingRange::from_lsp)
    //     .sorted_by(|x, y| x.start.line.cmp(&y.start.line))
    //     .collect()
    vec![
        FoldingRange {
            start_line: 0,
            start_character: Some(10),
            end_line: 7,
            end_character: Some(1),
            ..Default::default()
        },
        FoldingRange {
            start_line: 1,
            start_character: Some(12),
            end_line: 3,
            end_character: Some(5),
            ..Default::default()
        },
        FoldingRange {
            start_line: 3,
            start_character: Some(11),
            end_line: 5,
            end_character: Some(5),
            ..Default::default()
        },
        FoldingRange {
            start_line: 10,
            start_character: Some(10),
            end_line: 27,
            end_character: Some(1),
            ..Default::default()
        },
    ]
}

///  2|   if true {...} else {...}\r\n
pub fn folded_v2() -> FoldingDisplayItem {
    FoldingDisplayItem {
        // position: Position {
        //     line:      5,
        //     character: 5,
        // },
        y:  0,
        ty: FoldingDisplayType::UnfoldEnd,
        iv: Interval::new(73, 111),
    }
}

/// main_2.rs
fn init_diag_2() -> im::Vector<Diagnostic> {
    let mut diags = im::Vector::new();
    diags.push_back(serde_json::from_str(r#"{"range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}},"severity":2,"code":"unused_variables","source":"rustc","message":"unused variable: `a`\n`#[warn(unused_variables)]` on by default","relatedInformation":[{"location":{"uri":"file:///d:/git/check/src/simple-ansi-to-style","range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}}},"message":"if this is intentional, prefix it with an underscore: `_a`"}],"tags":[1],"data":{"rendered":"warning: unused variable: `a`\n --> src/simple-ansi-to-style:7:9\n  |\n7 |     let a = A;\n  |         ^ help: if this is intentional, prefix it with an underscore: `_a`\n  |\n  = note: `#[warn(unused_variables)]` on by default\n\n"}}"#).unwrap());
    diags.push_back(serde_json::from_str(r#"{"range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}},"severity":4,"code":"unused_variables","source":"rustc","message":"if this is intentional, prefix it with an underscore: `_a`","relatedInformation":[{"location":{"uri":"file:///d:/git/check/src/simple-ansi-to-style","range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}}},"message":"original diagnostic"}]}"#).unwrap());
    diags.push_back(serde_json::from_str(r#"{"range":{"start":{"line":10,"character":3},"end":{"line":10,"character":7}},"severity":2,"code":"dead_code","source":"rustc","message":"function `test` is never used\n`#[warn(dead_code)]` on by default","tags":[1],"data":{"rendered":"warning: function `test` is never used\n  --> src/simple-ansi-to-style:11:4\n   |\n11 | fn test() {\n   |    ^^^^\n   |\n   = note: `#[warn(dead_code)]` on by default\n\n"}}"#).unwrap());
    diags
}

pub fn init_main_2() -> anyhow::Result<DocLines> {
    let file: PathBuf = "../../resources/test_code/main_2.rs".into();

    let folding = _init_lsp_folding_range_2();
    let rs = _init_code(file);
    let hints = r#"[{"position":{"line":6,"character":9},"label":[{"value":": "},{"value":"A","location":{"uri":"file:///d:/git/check/src/main.rs","range":{"start":{"line":8,"character":7},"end":{"line":8,"character":8}}}}],"kind":1,"textEdits":[{"range":{"start":{"line":6,"character":9},"end":{"line":6,"character":9}},"newText":": A"}],"paddingLeft":false,"paddingRight":false}]"#;
    let hints = _init_inlay_hint(&rs.1, hints)?;
    let (mut lines, _) = _init_lines(None, rs, folding, Some(hints))?;
    let diags = init_diag_2();
    let semantic = init_semantic_2();

    lines.diagnostics.diagnostics.update(|x| *x = diags);
    lines.init_diagnostics()?;

    let mut styles_span = SpansBuilder::new(lines.buffer().len());
    for style in semantic.styles {
        if let Some(fg) = style.style.fg_color {
            styles_span.add_span(Interval::new(style.start, style.end), fg);
        }
    }
    let styles = styles_span.build();

    lines.update_semantic_styles_from_lsp((None, styles), lines.buffer().rev())?;

    Ok(lines)
}

/// main_2.rs
pub fn init_semantic_2() -> SemanticStyles {
    serde_json::from_str(r#"{"rev":1,"path":"D:\\git\\check\\src\\simple-ansi-to-style","len":461,"styles":[{"start":0,"end":2,"text":null,"style":{"fg_color":"keyword"}},{"start":3,"end":7,"text":null,"style":{"fg_color":"function"}},{"start":17,"end":19,"text":null,"style":{"fg_color":"keyword"}},{"start":20,"end":24,"text":null,"style":{"fg_color":"boolean"}},{"start":36,"end":43,"text":null,"style":{"fg_color":"macro"}},{"start":43,"end":44,"text":null,"style":{"fg_color":"macro"}},{"start":45,"end":54,"text":null,"style":{"fg_color":"string"}},{"start":64,"end":68,"text":null,"style":{"fg_color":"keyword"}},{"start":80,"end":87,"text":null,"style":{"fg_color":"macro"}},{"start":87,"end":88,"text":null,"style":{"fg_color":"macro"}},{"start":89,"end":98,"text":null,"style":{"fg_color":"string"}},{"start":113,"end":116,"text":null,"style":{"fg_color":"keyword"}},{"start":117,"end":118,"text":null,"style":{"fg_color":"variable"}},{"start":119,"end":120,"text":null,"style":{"fg_color":"operator"}},{"start":121,"end":122,"text":null,"style":{"fg_color":"struct"}},{"start":128,"end":134,"text":null,"style":{"fg_color":"keyword"}},{"start":135,"end":136,"text":null,"style":{"fg_color":"struct"}},{"start":141,"end":143,"text":null,"style":{"fg_color":"keyword"}},{"start":144,"end":148,"text":null,"style":{"fg_color":"function"}},{"start":158,"end":165,"text":null,"style":{"fg_color":"macro"}},{"start":165,"end":166,"text":null,"style":{"fg_color":"macro"}},{"start":167,"end":169,"text":null,"style":{"fg_color":"string"}},{"start":177,"end":184,"text":null,"style":{"fg_color":"macro"}},{"start":184,"end":185,"text":null,"style":{"fg_color":"macro"}},{"start":186,"end":188,"text":null,"style":{"fg_color":"string"}},{"start":196,"end":203,"text":null,"style":{"fg_color":"macro"}},{"start":203,"end":204,"text":null,"style":{"fg_color":"macro"}},{"start":205,"end":207,"text":null,"style":{"fg_color":"string"}},{"start":215,"end":222,"text":null,"style":{"fg_color":"macro"}},{"start":222,"end":223,"text":null,"style":{"fg_color":"macro"}},{"start":224,"end":226,"text":null,"style":{"fg_color":"string"}},{"start":234,"end":241,"text":null,"style":{"fg_color":"macro"}},{"start":241,"end":242,"text":null,"style":{"fg_color":"macro"}},{"start":243,"end":245,"text":null,"style":{"fg_color":"string"}},{"start":253,"end":260,"text":null,"style":{"fg_color":"macro"}},{"start":260,"end":261,"text":null,"style":{"fg_color":"macro"}},{"start":262,"end":264,"text":null,"style":{"fg_color":"string"}},{"start":272,"end":279,"text":null,"style":{"fg_color":"macro"}},{"start":279,"end":280,"text":null,"style":{"fg_color":"macro"}},{"start":281,"end":283,"text":null,"style":{"fg_color":"string"}},{"start":291,"end":298,"text":null,"style":{"fg_color":"macro"}},{"start":298,"end":299,"text":null,"style":{"fg_color":"macro"}},{"start":300,"end":302,"text":null,"style":{"fg_color":"string"}},{"start":310,"end":317,"text":null,"style":{"fg_color":"macro"}},{"start":317,"end":318,"text":null,"style":{"fg_color":"macro"}},{"start":319,"end":321,"text":null,"style":{"fg_color":"string"}},{"start":329,"end":336,"text":null,"style":{"fg_color":"macro"}},{"start":336,"end":337,"text":null,"style":{"fg_color":"macro"}},{"start":338,"end":340,"text":null,"style":{"fg_color":"string"}},{"start":348,"end":355,"text":null,"style":{"fg_color":"macro"}},{"start":355,"end":356,"text":null,"style":{"fg_color":"macro"}},{"start":357,"end":359,"text":null,"style":{"fg_color":"string"}},{"start":367,"end":374,"text":null,"style":{"fg_color":"macro"}},{"start":374,"end":375,"text":null,"style":{"fg_color":"macro"}},{"start":376,"end":378,"text":null,"style":{"fg_color":"string"}},{"start":386,"end":393,"text":null,"style":{"fg_color":"macro"}},{"start":393,"end":394,"text":null,"style":{"fg_color":"macro"}},{"start":395,"end":397,"text":null,"style":{"fg_color":"string"}},{"start":405,"end":412,"text":null,"style":{"fg_color":"macro"}},{"start":412,"end":413,"text":null,"style":{"fg_color":"macro"}},{"start":414,"end":416,"text":null,"style":{"fg_color":"string"}},{"start":424,"end":431,"text":null,"style":{"fg_color":"macro"}},{"start":431,"end":432,"text":null,"style":{"fg_color":"macro"}},{"start":433,"end":435,"text":null,"style":{"fg_color":"string"}},{"start":443,"end":450,"text":null,"style":{"fg_color":"macro"}},{"start":450,"end":451,"text":null,"style":{"fg_color":"macro"}},{"start":452,"end":454,"text":null,"style":{"fg_color":"string"}}]}"#).unwrap()
}

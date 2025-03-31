use std::path::PathBuf;

use doc::lines::{
    DocLines,
    buffer::rope_text::RopeText,
    fold::{FoldingDisplayItem, FoldingDisplayType},
};
use floem::prelude::SignalUpdate;
use jsonrpc_lite::JsonRpc;
use lapce_xi_rope::{Interval, spans::SpansBuilder};
use lsp_types::{Diagnostic, DocumentSymbol, DocumentSymbolResponse, FoldingRange};

use super::{_init_code, _init_inlay_hint, _init_lines};
use crate::tests::lines_util::{_init, SemanticStyles};

fn _init_lsp_folding_range_4() -> Vec<FoldingRange> {
    let folding_range = r#"{"jsonrpc":"2.0","id":21,"result":[{"startLine":1,"startCharacter":9,"endLine":11,"endCharacter":1},{"startLine":12,"startCharacter":9,"endLine":17,"endCharacter":1},{"startLine":18,"startCharacter":10,"endLine":24,"endCharacter":1},{"startLine":19,"startCharacter":12,"endLine":21,"endCharacter":5},{"startLine":21,"startCharacter":11,"endLine":23,"endCharacter":5}]}"#;
    if let Ok(value @ JsonRpc::Success(_)) = JsonRpc::parse(folding_range) {
        serde_json::from_value::<Vec<FoldingRange>>(
            value.get_result().unwrap().clone(),
        )
        .unwrap()
    } else {
        panic!();
    }
}

pub fn _init_lsp_document_symbol() -> Vec<DocumentSymbol> {
    let folding_range = r#"{"jsonrpc":"2.0","id":19,"result":[{"name":"A","kind":23,"tags":[],"deprecated":false,"range":{"start":{"line":1,"character":0},"end":{"line":11,"character":1}},"selectionRange":{"start":{"line":1,"character":7},"end":{"line":1,"character":8}},"children":[{"name":"a1","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":2,"character":4},"end":{"line":2,"character":12}},"selectionRange":{"start":{"line":2,"character":4},"end":{"line":2,"character":6}}},{"name":"a2","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":3,"character":4},"end":{"line":3,"character":12}},"selectionRange":{"start":{"line":3,"character":4},"end":{"line":3,"character":6}}},{"name":"a3","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":4,"character":4},"end":{"line":4,"character":12}},"selectionRange":{"start":{"line":4,"character":4},"end":{"line":4,"character":6}}},{"name":"a4","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":5,"character":4},"end":{"line":5,"character":12}},"selectionRange":{"start":{"line":5,"character":4},"end":{"line":5,"character":6}}},{"name":"a5","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":6,"character":4},"end":{"line":6,"character":12}},"selectionRange":{"start":{"line":6,"character":4},"end":{"line":6,"character":6}}},{"name":"a6","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":7,"character":4},"end":{"line":7,"character":12}},"selectionRange":{"start":{"line":7,"character":4},"end":{"line":7,"character":6}}},{"name":"a7","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":8,"character":4},"end":{"line":8,"character":12}},"selectionRange":{"start":{"line":8,"character":4},"end":{"line":8,"character":6}}},{"name":"a8","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":9,"character":4},"end":{"line":9,"character":12}},"selectionRange":{"start":{"line":9,"character":4},"end":{"line":9,"character":6}}},{"name":"a9","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":10,"character":4},"end":{"line":10,"character":12}},"selectionRange":{"start":{"line":10,"character":4},"end":{"line":10,"character":6}}}]},{"name":"B","kind":23,"tags":[],"deprecated":false,"range":{"start":{"line":12,"character":0},"end":{"line":17,"character":1}},"selectionRange":{"start":{"line":12,"character":7},"end":{"line":12,"character":8}},"children":[{"name":"a1","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":13,"character":4},"end":{"line":13,"character":12}},"selectionRange":{"start":{"line":13,"character":4},"end":{"line":13,"character":6}}},{"name":"a2","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":14,"character":4},"end":{"line":14,"character":12}},"selectionRange":{"start":{"line":14,"character":4},"end":{"line":14,"character":6}}},{"name":"a3","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":15,"character":4},"end":{"line":15,"character":12}},"selectionRange":{"start":{"line":15,"character":4},"end":{"line":15,"character":6}}},{"name":"a4","detail":"bool","kind":8,"tags":[],"deprecated":false,"range":{"start":{"line":16,"character":4},"end":{"line":16,"character":12}},"selectionRange":{"start":{"line":16,"character":4},"end":{"line":16,"character":6}}}]},{"name":"main","detail":"fn()","kind":12,"tags":[],"deprecated":false,"range":{"start":{"line":18,"character":0},"end":{"line":24,"character":1}},"selectionRange":{"start":{"line":18,"character":3},"end":{"line":18,"character":7}}}]}"#;
    if let Ok(value @ JsonRpc::Success(_)) = JsonRpc::parse(folding_range) {
        if let DocumentSymbolResponse::Nested(val) =
            serde_json::from_value::<DocumentSymbolResponse>(
                value.get_result().unwrap().clone(),
            )
            .unwrap()
        {
            return val;
        }
    }
    panic!();
}

///  2|   if true {...} else {...}\r\n
pub fn folded_v4() -> FoldingDisplayItem {
    FoldingDisplayItem {
        y:  0,
        ty: FoldingDisplayType::UnfoldEnd,
        iv: Interval::new(73, 111),
    }
}

/// main_2.rs
fn init_diag_4() -> im::Vector<Diagnostic> {
    im::Vector::new()
}

pub fn init_main_4() -> anyhow::Result<DocLines> {
    let file: PathBuf = "../resources/test_code/main_2.rs".into();

    let folding = _init_lsp_folding_range_4();
    let rs = _init_code(file);
    let hints = r#"[]"#;
    let hints = _init_inlay_hint(&rs.1, hints)?;
    let (mut lines, _) = _init_lines(None, rs, folding, Some(hints))?;
    let diags = init_diag_4();
    let semantic = init_semantic_4();

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

/// main_4.rs
pub fn init_semantic_4() -> SemanticStyles {
    SemanticStyles {
        rev:    1,
        path:   "".into(),
        len:    1,
        styles: vec![],
    }
    // serde_json::from_str(r#"{"rev":1,"path":"D:\\git\\check\\src\\
    // simple-ansi-to-style","len":461,"styles":[{"start":0,"end":2,"text":null,
    // "style":{"fg_color":"keyword"}},{"start":3,"end":7,"text":null,"style":{"
    // fg_color":"function"}},{"start":17,"end":19,"text":null,"style":{"
    // fg_color":"keyword"}},{"start":20,"end":24,"text":null,"style":{"
    // fg_color":"boolean"}},{"start":36,"end":43,"text":null,"style":{"
    // fg_color":"macro"}},{"start":43,"end":44,"text":null,"style":{"fg_color":
    // "macro"}},{"start":45,"end":54,"text":null,"style":{"fg_color":"string"
    // }},{"start":64,"end":68,"text":null,"style":{"fg_color":"keyword"}},{"
    // start":80,"end":87,"text":null,"style":{"fg_color":"macro"}},{"start":87,
    // "end":88,"text":null,"style":{"fg_color":"macro"}},{"start":89,"end":98,"
    // text":null,"style":{"fg_color":"string"}},{"start":113,"end":116,"text":
    // null,"style":{"fg_color":"keyword"}},{"start":117,"end":118,"text":null,"
    // style":{"fg_color":"variable"}},{"start":119,"end":120,"text":null,"
    // style":{"fg_color":"operator"}},{"start":121,"end":122,"text":null,"
    // style":{"fg_color":"struct"}},{"start":128,"end":134,"text":null,"style":
    // {"fg_color":"keyword"}},{"start":135,"end":136,"text":null,"style":{"
    // fg_color":"struct"}},{"start":141,"end":143,"text":null,"style":{"
    // fg_color":"keyword"}},{"start":144,"end":148,"text":null,"style":{"
    // fg_color":"function"}},{"start":158,"end":165,"text":null,"style":{"
    // fg_color":"macro"}},{"start":165,"end":166,"text":null,"style":{"
    // fg_color":"macro"}},{"start":167,"end":169,"text":null,"style":{"
    // fg_color":"string"}},{"start":177,"end":184,"text":null,"style":{"
    // fg_color":"macro"}},{"start":184,"end":185,"text":null,"style":{"
    // fg_color":"macro"}},{"start":186,"end":188,"text":null,"style":{"
    // fg_color":"string"}},{"start":196,"end":203,"text":null,"style":{"
    // fg_color":"macro"}},{"start":203,"end":204,"text":null,"style":{"
    // fg_color":"macro"}},{"start":205,"end":207,"text":null,"style":{"
    // fg_color":"string"}},{"start":215,"end":222,"text":null,"style":{"
    // fg_color":"macro"}},{"start":222,"end":223,"text":null,"style":{"
    // fg_color":"macro"}},{"start":224,"end":226,"text":null,"style":{"
    // fg_color":"string"}},{"start":234,"end":241,"text":null,"style":{"
    // fg_color":"macro"}},{"start":241,"end":242,"text":null,"style":{"
    // fg_color":"macro"}},{"start":243,"end":245,"text":null,"style":{"
    // fg_color":"string"}},{"start":253,"end":260,"text":null,"style":{"
    // fg_color":"macro"}},{"start":260,"end":261,"text":null,"style":{"
    // fg_color":"macro"}},{"start":262,"end":264,"text":null,"style":{"
    // fg_color":"string"}},{"start":272,"end":279,"text":null,"style":{"
    // fg_color":"macro"}},{"start":279,"end":280,"text":null,"style":{"
    // fg_color":"macro"}},{"start":281,"end":283,"text":null,"style":{"
    // fg_color":"string"}},{"start":291,"end":298,"text":null,"style":{"
    // fg_color":"macro"}},{"start":298,"end":299,"text":null,"style":{"
    // fg_color":"macro"}},{"start":300,"end":302,"text":null,"style":{"
    // fg_color":"string"}},{"start":310,"end":317,"text":null,"style":{"
    // fg_color":"macro"}},{"start":317,"end":318,"text":null,"style":{"
    // fg_color":"macro"}},{"start":319,"end":321,"text":null,"style":{"
    // fg_color":"string"}},{"start":329,"end":336,"text":null,"style":{"
    // fg_color":"macro"}},{"start":336,"end":337,"text":null,"style":{"
    // fg_color":"macro"}},{"start":338,"end":340,"text":null,"style":{"
    // fg_color":"string"}},{"start":348,"end":355,"text":null,"style":{"
    // fg_color":"macro"}},{"start":355,"end":356,"text":null,"style":{"
    // fg_color":"macro"}},{"start":357,"end":359,"text":null,"style":{"
    // fg_color":"string"}},{"start":367,"end":374,"text":null,"style":{"
    // fg_color":"macro"}},{"start":374,"end":375,"text":null,"style":{"
    // fg_color":"macro"}},{"start":376,"end":378,"text":null,"style":{"
    // fg_color":"string"}},{"start":386,"end":393,"text":null,"style":{"
    // fg_color":"macro"}},{"start":393,"end":394,"text":null,"style":{"
    // fg_color":"macro"}},{"start":395,"end":397,"text":null,"style":{"
    // fg_color":"string"}},{"start":405,"end":412,"text":null,"style":{"
    // fg_color":"macro"}},{"start":412,"end":413,"text":null,"style":{"
    // fg_color":"macro"}},{"start":414,"end":416,"text":null,"style":{"
    // fg_color":"string"}},{"start":424,"end":431,"text":null,"style":{"
    // fg_color":"macro"}},{"start":431,"end":432,"text":null,"style":{"
    // fg_color":"macro"}},{"start":433,"end":435,"text":null,"style":{"
    // fg_color":"string"}},{"start":443,"end":450,"text":null,"style":{"
    // fg_color":"macro"}},{"start":450,"end":451,"text":null,"style":{"
    // fg_color":"macro"}},{"start":452,"end":454,"text":null,"style":{"
    // fg_color":"string"}}]}"#).unwrap()
}

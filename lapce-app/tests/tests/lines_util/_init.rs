use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicU64},
};

use anyhow::Result;
use doc::{
    EditorViewKind,
    config::EditorConfig,
    diagnostic::DiagnosticData,
    language::LapceLanguage,
    lines::{
        DocLines, RopeTextPosition,
        buffer::{
            Buffer,
            diff::{DiffLines, rope_diff},
            rope_text::RopeText,
        },
        cursor::{Cursor, CursorMode},
        diff::{DiffInfo, DiffResult},
        fold::{FoldingDisplayItem, FoldingDisplayType},
        selection::Selection,
        style::EditorStyle,
    },
    syntax::{BracketParser, Syntax},
};
use floem::{
    kurbo::Rect,
    reactive::{RwSignal, Scope, SignalUpdate},
};
use itertools::Itertools;
use lapce_xi_rope::{
    Interval,
    spans::{Spans, SpansBuilder},
};
use log::info;
use lsp_types::{Diagnostic, FoldingRange, InlayHint, Position};

use crate::tests::lines_util::init_main_2::init_semantic_2;

fn _init_lsp_folding_range() -> Vec<FoldingRange> {
    // let folding_range =
    // r#"[{"start":{"line":1,"character":14},"end":{"line":8,"character":1},"
    // status":"Unfold","collapsed_text":null},{"start":{"line":2,"character":12},"
    // end":{"line":4,"character":5},"status":"Unfold","collapsed_text":null},{"
    // start":{"line":4,"character":11},"end":{"line":6,"character":5},"status":"
    // Unfold","collapsed_text":null}]"#;
    // let folding_range: Vec<FoldingRange> =
    //     serde_json::from_str(folding_range).unwrap();
    vec![
        FoldingRange {
            start_line: 1,
            start_character: Some(14),
            end_line: 8,
            end_character: Some(1),
            ..Default::default()
        },
        FoldingRange {
            start_line: 2,
            start_character: Some(12),
            end_line: 4,
            end_character: Some(5),
            ..Default::default()
        },
        FoldingRange {
            start_line: 4,
            start_character: Some(11),
            end_line: 6,
            end_character: Some(5),
            ..Default::default()
        },
    ]
}

fn _init_lsp_folding_range_3() -> Vec<FoldingRange> {
    // let folding_range =
    // r#"[{"start":{"line":0,"character":14},"end":{"line":6,"character":1},"
    // status":"Unfold","collapsed_text":null},{"start":{"line":1,"character":12},"
    // end":{"line":3,"character":5} ,"status":"Unfold","collapsed_text":null},
    // {"start":{"line":3,"character":11},"end":{"line":5,"character":5},"status":"
    // Unfold","collapsed_text":null}]"#;
    vec![
        FoldingRange {
            start_line: 0,
            start_character: Some(14),
            end_line: 6,
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
    ]
}

pub fn _init_inlay_hint(buffer: &Buffer, hints: &str) -> Result<Spans<InlayHint>> {
    // let hints = r#"[{"position":{"line":6,"character":9},"label":[{"value":": "},{"value":"A","location":{"uri":"file:///d:/git/check/src/simple-ansi-to-style","range":{"start":{"line":8,"character":7},"end":{"line":8,"character":8}}}}],"kind":1,"textEdits":[{"range":{"start":{"line":6,"character":9},"end":{"line":6,"character":9}},"newText":": A"}],"paddingLeft":false,"paddingRight":false}]"#;
    let mut hints: Vec<InlayHint> = serde_json::from_str(hints).unwrap();
    let len = buffer.len();
    hints.sort_by(|left, right| left.position.cmp(&right.position));
    let mut hints_span = SpansBuilder::new(len);
    for hint in hints {
        let offset = buffer.offset_of_position(&hint.position)?.min(len);
        hints_span.add_span(Interval::new(offset, (offset + 1).min(len)), hint);
    }
    Ok(hints_span.build())
}
pub fn _init_code(file: PathBuf) -> (String, Buffer) {
    // let code = "pub fn main() {\r\n    if true {\r\n
    // println!(\"startss\");\r\n    } else {\r\n
    // println!(\"startss\");\r\n    }\r\n    let a =
    // A;\r\n}\r\nstruct A;\r\n";
    let code = load_code(&file);
    let buffer = Buffer::new(code.as_str());
    info!("line_ending {:?} len={}", buffer.line_ending(), code.len());
    (code, buffer)
}

///  2|   if true {...} else {\r\n
pub fn folded_v1() -> FoldingDisplayItem {
    FoldingDisplayItem {
        // position: Position {
        //     line:      1,
        //     character: 12,
        // },
        iv: Interval::new(29, 69),
        y:  0,
        ty: FoldingDisplayType::UnfoldStart,
    }
}

/// just for init_main_2()
pub fn init_main_folded_item_2() -> Result<Vec<FoldingDisplayItem>> {
    Ok(vec![
        serde_json::from_str(
            r#"{"position":{"line":1,"character":12},"y":20,"ty":"UnfoldStart","iv":{"start":25,"end":63}}"#,
        )?,
        serde_json::from_str(
            r#"{"position":{"line":5,"character":5},"y":60,"ty":"UnfoldEnd","iv":{"start":69,"end":107}}"#,
        )?,
        serde_json::from_str(
            r#"{"position":{"line":10,"character":10},"y":120,"ty":"UnfoldStart","iv":{"start":151,"end":459}}"#,
        )?,
    ])
}

/// just for init_main_2()
pub fn init_main_folded_item_3() -> Result<Vec<FoldingDisplayItem>> {
    Ok(vec![serde_json::from_str(
        r#"{"position":{"line":0,"character":14},"y":0,"ty":"UnfoldStart","iv":{"start":14,"end":130}}"#,
    )?])
}

pub fn _init_lines(
    folded: Option<Vec<FoldingDisplayItem>>,
    (code, buffer): (String, Buffer),
    folding: Vec<FoldingRange>,
    hints: Option<Spans<InlayHint>>,
) -> Result<(DocLines, EditorConfig)> {
    // let folding = _init_lsp_folding_range();
    let config_str = r##"{"font_family":"monospace","font_size":13,"line_height":20,"enable_inlay_hints":true,"inlay_hint_font_size":0,"enable_error_lens":true,"error_lens_end_of_line":true,"error_lens_multiline":false,"error_lens_font_size":0,"enable_completion_lens":false,"enable_inline_completion":true,"completion_lens_font_size":0,"only_render_error_styling":true,"auto_closing_matching_pairs":true,"auto_surround":true,"diagnostic_error":{"components":[0.8980393,0.078431375,0.0,1.0],"cs":null},"diagnostic_warn":{"components":[0.91372555,0.654902,0.0,1.0],"cs":null},"inlay_hint_fg":{"components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"inlay_hint_bg":{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"error_lens_error_foreground":{"components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"error_lens_warning_foreground":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"error_lens_other_foreground":{"components":[0.627451,0.6313726,0.654902,1.0],"cs":null},"completion_lens_foreground":{"components":[0.627451,0.6313726,0.654902,1.0],"cs":null},"editor_foreground":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"syntax":{"markup.link.url":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"function.method":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"markup.heading":{"components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"punctuation.delimiter":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"tag":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"variable.other.member":{"components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"escape":{"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"markup.link.label":{"components":[0.6509804,0.14901961,0.6431373,1.0],"cs":null},"property":{"components":[0.53333336,0.08627451,0.5882353,1.0],"cs":null},"enum-member":{"components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"text.reference":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"text.uri":{"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"builtinType":{"components":[0.07058824,0.24705884,0.72156864,1.0],"cs":null},"enumMember":{"components":[0.57254905,0.06666667,0.654902,1.0],"cs":null},"keyword":{"components":[0.027450982,0.23529413,0.7176471,1.0],"cs":null},"markup.list":{"components":[0.8196079,0.6039216,0.40000004,1.0],"cs":null},"text.title":{"components":[0.8196079,0.6039216,0.40000004,1.0],"cs":null},"struct":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"type":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"interface":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"selfKeyword":{"components":[0.6509804,0.14901961,0.6431373,1.0],"cs":null},"type.builtin":{"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"constant":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"variable":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"attribute":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"enum":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"markup.bold":{"components":[0.8196079,0.6039216,0.40000004,1.0],"cs":null},"method":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"string.escape":{"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"embedded":{"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"markup.link.text":{"components":[0.6509804,0.14901961,0.6431373,1.0],"cs":null},"comment":{"components":[0.627451,0.6313726,0.654902,1.0],"cs":null},"typeAlias":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"function":{"components":[0.2392157,0.42352945,0.49411768,1.0],"cs":null},"string":{"components":[0.3137255,0.6313726,0.30980393,1.0],"cs":null},"constructor":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"bracket.unpaired":{"components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"field":{"components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"structure":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"markup.italic":{"components":[0.8196079,0.6039216,0.40000004,1.0],"cs":null},"number":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null}}}"##;
    // let config_str = r##"{"font_family":"JetBrains
    // Mono","font_size":13,"line_height":23,"enable_inlay_hints":true,"
    // inlay_hint_font_size":0,"enable_error_lens":true,"error_lens_end_of_line":
    // false,"error_lens_multiline":false,"error_lens_font_size":0,"
    // enable_completion_lens":false,"enable_inline_completion":true,"
    // completion_lens_font_size":0,"only_render_error_styling":true,"
    // auto_closing_matching_pairs":true,"auto_surround":true,"diagnostic_error":{"
    // components":[0.8980393,0.078431375,0.0,1.0],"cs":null},"diagnostic_warn":{"
    // components":[0.91372555,0.654902,0.0,1.0],"cs":null},"inlay_hint_fg":{"
    // components":[0.65882355,0.65882355,0.65882355,1.0],"cs":null},"inlay_hint_bg"
    // :{"components":[0.9215687,0.9215687,0.9215687,1.0],"cs":null},"
    // error_lens_error_foreground":{"components":[0.8941177,0.3372549,0.28627452,1.
    // 0],"cs":null},"error_lens_warning_foreground":{"components":[0.7568628,0.
    // 5176471,0.003921569,1.0],"cs":null},"error_lens_other_foreground":{"
    // components":[0.627451,0.6313726,0.654902,1.0],"cs":null},"
    // completion_lens_foreground":{"components":[0.627451,0.6313726,0.654902,1.0],"
    // cs":null},"editor_foreground":{"components":[0.21960786,0.227451,0.25882354,
    // 1.0],"cs":null},"syntax":{"markup.heading":{"components":[0.8941177,0.
    // 3372549,0.28627452,1.0],"cs":null},"markup.italic":{"components":[0.8196079,
    // 0.6039216,0.40000004,1.0],"cs":null},"markup.link.text":{"components":[0.
    // 6509804,0.14901961,0.6431373,1.0],"cs":null},"string.escape":{"components":
    // [0.003921569,0.5176471,0.7372549,1.0],"cs":null},"variable":{"components":[0.
    // 21960786,0.227451,0.25882354,1.0],"cs":null},"string":{"components":[0.
    // 3137255,0.6313726,0.30980393,1.0],"cs":null},"constructor":{"components":[0.
    // 7568628,0.5176471,0.003921569,1.0],"cs":null},"enum":{"components":[0.
    // 21960786,0.227451,0.25882354,1.0],"cs":null},"attribute":{"components":[0.
    // 7568628,0.5176471,0.003921569,1.0],"cs":null},"interface":{"components":[0.
    // 21960786,0.227451,0.25882354,1.0],"cs":null},"markup.bold":{"components":[0.
    // 8196079,0.6039216,0.40000004,1.0],"cs":null},"field":{"components":[0.
    // 8941177,0.3372549,0.28627452,1.0],"cs":null},"enum-member":{"components":[0.
    // 8941177,0.3372549,0.28627452,1.0],"cs":null},"text.uri":{"components":[0.
    // 003921569,0.5176471,0.7372549,1.0],"cs":null},"text.reference":{"components":
    // [0.7568628,0.5176471,0.003921569,1.0],"cs":null},"bracket.unpaired":{"
    // components":[0.8941177,0.3372549,0.28627452,1.0],"cs":null},"text.title":{"
    // components":[0.8196079,0.6039216,0.40000004,1.0],"cs":null},"selfKeyword":{"
    // components":[0.6509804,0.14901961,0.6431373,1.0],"cs":null},"keyword":{"
    // components":[0.027450982,0.23529413,0.7176471,1.0],"cs":null},"type.builtin":
    // {"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"constant":{"
    // components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"embedded":{"
    // components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"function.
    // method":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"
    // enumMember":{"components":[0.57254905,0.06666667,0.654902,1.0],"cs":null},"
    // comment":{"components":[0.627451,0.6313726,0.654902,1.0],"cs":null},"markup.
    // link.url":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"
    // escape":{"components":[0.003921569,0.5176471,0.7372549,1.0],"cs":null},"
    // markup.list":{"components":[0.8196079,0.6039216,0.40000004,1.0],"cs":null},"
    // method":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"
    // function":{"components":[0.2392157,0.42352945,0.49411768,1.0],"cs":null},"
    // number":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":null},"
    // builtinType":{"components":[0.07058824,0.24705884,0.72156864,1.0],"cs":null},
    // "markup.link.label":{"components":[0.6509804,0.14901961,0.6431373,1.0],"cs":
    // null},"property":{"components":[0.53333336,0.08627451,0.5882353,1.0],"cs":
    // null},"bracket.color.1":{"components":[0.7725491,0.882353,0.7725491,1.0],"cs"
    // :null},"struct":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":
    // null},"structure":{"components":[0.7568628,0.5176471,0.003921569,1.0],"cs":
    // null},"tag":{"components":[0.2509804,0.47058827,0.9490197,1.0],"cs":null},"
    // type":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs":null},"
    // variable.other.member":{"components":[0.8941177,0.3372549,0.28627452,1.0],"
    // cs":null},"typeAlias":{"components":[0.21960786,0.227451,0.25882354,1.0],"cs"
    // :null},"punctuation.delimiter":{"components":[0.7568628,0.5176471,0.
    // 003921569,1.0],"cs":null}}}"##;
    let config: EditorConfig = serde_json::from_str(config_str).unwrap();
    let cx = Scope::new();
    let diagnostics = DiagnosticData::new(cx);
    // { x0: 0.0, y0: 0.0, x1: 591.1680297851563, y1:
    // 538.1586303710938 }
    let view = Rect::new(0.0, 0.0, 591.0, 538.0);
    let editor_style = EditorStyle::default();
    let language = LapceLanguage::Rust;
    let grammars_dir: PathBuf = "C:\\Users\\36225\\AppData\\Local\\lapce\\\
                                 Lapce-Debug\\data\\grammars"
        .into();

    let queries_directory: PathBuf = "C:\\Users\\36225\\AppData\\Roaming\\lapce\\\
                                      Lapce-Debug\\config\\queries"
        .into();

    let syntax = Syntax::from_language(language, &grammars_dir, &queries_directory);
    let parser = BracketParser::new(code.to_string(), true, 30000);
    let mut lines = DocLines::new(
        cx,
        diagnostics,
        syntax,
        parser,
        view,
        editor_style,
        config.clone(),
        buffer,
        None,
    );
    lines.update_folding_ranges(folding.into())?;
    if let Some(hints) = hints {
        lines.set_inlay_hints(hints)?;
    }
    if let Some(folded) = folded {
        for folded in folded {
            lines.update_folding_ranges(folded.into())?;
        }
    }
    Ok((lines, config))
}

fn load_code(file: &Path) -> String {
    std::fs::read_to_string(file).unwrap()
}

pub fn init_main() -> Result<DocLines> {
    let file: PathBuf = "../resources/test_code/main.rs".into();
    let rs = _init_code(file);
    let hints = r#"[{"position":{"line":7,"character":9},"label":[{"value":": "},{"value":"A","location":{"uri":"file:///d:/git/check/src/main.rs","range":{"start":{"line":9,"character":7},"end":{"line":9,"character":8}}}}],"kind":1,"textEdits":[{"range":{"start":{"line":7,"character":9},"end":{"line":7,"character":9}},"newText":": A"}],"paddingLeft":false,"paddingRight":false}]"#;
    let hints = _init_inlay_hint(&rs.1, hints)?;
    let folding = _init_lsp_folding_range();
    let (lines, _) = _init_lines(None, rs, folding, Some(hints))?;
    Ok(lines)
}

pub fn init_main_3() -> Result<DocLines> {
    let file: PathBuf = "../resources/test_code/main_3.rs".into();
    let rs = _init_code(file);
    let folding = _init_lsp_folding_range_3();
    let (lines, _) = _init_lines(None, rs, folding, None)?;
    Ok(lines)
}

pub fn init_test_diff() -> Vec<DiffLines> {
    let file_old: PathBuf = "../resources/test_code/diff_test/test.rs".into();
    let file_new: PathBuf = "../resources/test_code/diff_test/test_new.rs".into();

    let code_old = load_code(&file_old);
    let code = load_code(&file_new);
    rope_diff(
        code_old.into(),
        code.into(),
        0,
        Arc::new(AtomicU64::new(0)),
        None,
    )
    .unwrap()
}

pub fn init_test_1_diff() -> Vec<DiffLines> {
    let file_old: PathBuf = "../resources/test_code/diff_test_1/test_1.rs".into();
    let file_new: PathBuf =
        "../resources/test_code/diff_test_1/test_1_new.rs".into();

    let code_old = load_code(&file_old);
    let code = load_code(&file_new);
    rope_diff(
        code_old.into(),
        code.into(),
        0,
        Arc::new(AtomicU64::new(0)),
        None,
    )
    .unwrap()
}

pub fn init_test() -> Result<(DocLines, DocLines, EditorViewKind, EditorViewKind)> {
    let file_old: PathBuf = "../resources/test_code/diff_test/test.rs".into();
    let file_new: PathBuf = "../resources/test_code/diff_test/test_new.rs".into();

    let diff = init_test_diff();
    let rs_new = _init_code(file_new);
    let rs_old = _init_code(file_old);

    let diff = DiffInfo {
        is_right: false,
        changes:  diff,
    };

    // let diff = init_diff()?;
    let left_kind = EditorViewKind::Diff {
        is_right: false,
        changes:  diff.left_changes(),
    };
    let right_kind = EditorViewKind::Diff {
        is_right: true,
        changes:  diff.right_changes(),
    };

    let (left_lines, _) = _init_lines(None, rs_old, vec![], None)?;
    let (right_lines, _) = _init_lines(None, rs_new, vec![], None)?;

    Ok((left_lines, right_lines, left_kind, right_kind))
}

pub fn init_test_1() -> Result<(DocLines, DocLines, EditorViewKind, EditorViewKind)>
{
    let file_old: PathBuf = "../resources/test_code/diff_test_1/test_1.rs".into();
    let file_new: PathBuf =
        "../resources/test_code/diff_test_1/test_1_new.rs".into();

    let diff = init_test_1_diff();
    let rs_new = _init_code(file_new);
    let rs_old = _init_code(file_old);

    let diff = DiffInfo {
        is_right: false,
        changes:  diff,
    };

    // let diff = init_diff()?;
    let left_kind = EditorViewKind::Diff {
        is_right: false,
        changes:  diff.left_changes(),
    };
    let right_kind = EditorViewKind::Diff {
        is_right: true,
        changes:  diff.right_changes(),
    };
    let (left_lines, _) = _init_lines(None, rs_old, vec![], None)?;
    let (right_lines, _) = _init_lines(None, rs_new, vec![], None)?;

    Ok((left_lines, right_lines, left_kind, right_kind))
}

pub fn init_empty() -> Result<DocLines> {
    custom_utils::logger::logger_stdout_debug();
    let file: PathBuf = "../resources/test_code/empty.rs".into();

    let (lines, _) =
        _init_lines(None, _init_code(file), _init_lsp_folding_range(), None)?;
    Ok(lines)
}

pub fn cursor_insert(start: usize, end: usize) -> Cursor {
    let mode = CursorMode::Insert(Selection::region(start, end));
    Cursor::new(mode, None, None)
}

pub fn init_diff() -> Result<DiffInfo> {
    let changes = r#"[{"Both":{"left":{"start":0,"end":6},"right":{"start":0,"end":6},"skip":{"start":0,"end":3}}},{"Left":{"start":6,"end":10}},{"Both":{"left":{"start":10,"end":11},"right":{"start":6,"end":7},"skip":null}},{"Left":{"start":11,"end":13}},{"Right":{"start":7,"end":9}},{"Both":{"left":{"start":13,"end":15},"right":{"start":9,"end":11},"skip":null}},{"Right":{"start":11,"end":15}},{"Both":{"left":{"start":15,"end":18},"right":{"start":15,"end":18},"skip":null}},{"Left":{"start":18,"end":19}}]"#;
    let changes: Vec<DiffLines> = serde_json::from_str(changes)?;
    Ok(DiffInfo {
        is_right: false,
        changes,
    })
}

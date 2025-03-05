use std::{
    fs::File,
    path::{Path, PathBuf}
};

use anyhow::Result;
use doc::{
    DiagnosticData, EditorViewKind,
    config::EditorConfig,
    language::LapceLanguage,
    lines::{
        DocLines, RopeTextPosition,
        buffer::{Buffer, rope_text::RopeText},
        cursor::{Cursor, CursorMode},
        fold::{FoldingDisplayItem, FoldingDisplayType, FoldingRange},
        selection::Selection,
        style::EditorStyle
    },
    syntax::{BracketParser, Syntax}
};
use floem::{
    kurbo::Rect,
    reactive::{RwSignal, Scope, SignalUpdate}
};
use itertools::Itertools;
use lapce_xi_rope::{
    Interval,
    spans::{Spans, SpansBuilder}
};
use log::info;
use lsp_types::{Diagnostic, InlayHint, Position};

use super::init_semantic_2;

fn _init_lsp_folding_range() -> Vec<FoldingRange> {
    let folding_range = r#"[{"startLine":0,"startCharacter":10,"endLine":7,"endCharacter":1},{"startLine":1,"startCharacter":12,"endLine":3,"endCharacter":5},{"startLine":3,"startCharacter":11,"endLine":5,"endCharacter":5}]"#;
    let folding_range: Vec<lsp_types::FoldingRange> =
        serde_json::from_str(folding_range).unwrap();

    folding_range
        .into_iter()
        .map(FoldingRange::from_lsp)
        .sorted_by(|x, y| x.start.line.cmp(&y.start.line))
        .collect()
}

fn _init_lsp_folding_range_2() -> Vec<FoldingRange> {
    let folding_range = r#"[{"startLine":0,"startCharacter":10,"endLine":7,"endCharacter":1},{"startLine":1,"startCharacter":12,"endLine":3,"endCharacter":5},{"startLine":3,"startCharacter":11,"endLine":5,"endCharacter":5},{"startLine":10,"startCharacter":10,"endLine":27,"endCharacter":1}]"#;
    let folding_range: Vec<lsp_types::FoldingRange> =
        serde_json::from_str(folding_range).unwrap();

    folding_range
        .into_iter()
        .map(FoldingRange::from_lsp)
        .sorted_by(|x, y| x.start.line.cmp(&y.start.line))
        .collect()
}

fn _init_inlay_hint(buffer: &Buffer, hints: &str) -> Result<Spans<InlayHint>> {
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
fn _init_code(file: PathBuf) -> (String, Buffer) {
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
        position: Position {
            line:      1,
            character: 12
        },
        y:        0,
        ty:       FoldingDisplayType::UnfoldStart
    }
}

///  2|   if true {...} else {...}\r\n
pub fn folded_v2() -> FoldingDisplayItem {
    FoldingDisplayItem {
        position: Position {
            line:      5,
            character: 5
        },
        y:        0,
        ty:       FoldingDisplayType::UnfoldEnd
    }
}

/// just for init_main_2()
pub fn init_main_folded_item_2() -> Result<Vec<FoldingDisplayItem>> {
    Ok(vec![
        serde_json::from_str(
            r#"{"position":{"line":1,"character":12},"y":20,"ty":"UnfoldStart"}"#
        )?,
        serde_json::from_str(
            r#"{"position":{"line":5,"character":5},"y":60,"ty":"UnfoldEnd"}"#
        )?,
        serde_json::from_str(
            r#"{"position":{"line":10,"character":10},"y":120,"ty":"UnfoldStart"}"#
        )?,
    ])
}

fn _init_lines(
    folded: Option<Vec<FoldingDisplayItem>>,
    (code, buffer): (String, Buffer),
    folding: Vec<FoldingRange>,
    hints: Option<Spans<InlayHint>>
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
    let diagnostics = DiagnosticData {
        expanded:         cx.create_rw_signal(false),
        diagnostics:      cx.create_rw_signal(im::Vector::new()),
        diagnostics_span: cx.create_rw_signal(Spans::default())
    };
    // { x0: 0.0, y0: 0.0, x1: 591.1680297851563, y1:
    // 538.1586303710938 }
    let view = Rect::new(0.0, 0.0, 591.0, 538.0);
    let editor_style = EditorStyle::default();
    let kind = cx.create_rw_signal(EditorViewKind::Normal);
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
        kind, None
    )?;
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

/// main_2.rs
fn init_diag_2() -> im::Vector<Diagnostic> {
    let mut diags = im::Vector::new();
    diags.push_back(serde_json::from_str(r#"{"range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}},"severity":2,"code":"unused_variables","source":"rustc","message":"unused variable: `a`\n`#[warn(unused_variables)]` on by default","relatedInformation":[{"location":{"uri":"file:///d:/git/check/src/simple-ansi-to-style","range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}}},"message":"if this is intentional, prefix it with an underscore: `_a`"}],"tags":[1],"data":{"rendered":"warning: unused variable: `a`\n --> src/simple-ansi-to-style:7:9\n  |\n7 |     let a = A;\n  |         ^ help: if this is intentional, prefix it with an underscore: `_a`\n  |\n  = note: `#[warn(unused_variables)]` on by default\n\n"}}"#).unwrap());
    diags.push_back(serde_json::from_str(r#"{"range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}},"severity":4,"code":"unused_variables","source":"rustc","message":"if this is intentional, prefix it with an underscore: `_a`","relatedInformation":[{"location":{"uri":"file:///d:/git/check/src/simple-ansi-to-style","range":{"start":{"line":6,"character":8},"end":{"line":6,"character":9}}},"message":"original diagnostic"}]}"#).unwrap());
    diags.push_back(serde_json::from_str(r#"{"range":{"start":{"line":10,"character":3},"end":{"line":10,"character":7}},"severity":2,"code":"dead_code","source":"rustc","message":"function `test` is never used\n`#[warn(dead_code)]` on by default","tags":[1],"data":{"rendered":"warning: function `test` is never used\n  --> src/simple-ansi-to-style:11:4\n   |\n11 | fn test() {\n   |    ^^^^\n   |\n   = note: `#[warn(dead_code)]` on by default\n\n"}}"#).unwrap());
    diags
}

pub fn init_main_2() -> Result<DocLines> {
    custom_utils::logger::logger_stdout_debug();
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

pub fn init_main() -> Result<DocLines> {
    custom_utils::logger::logger_stdout_debug();
    let file: PathBuf = "../../resources/test_code/main.rs".into();
    let rs = _init_code(file);
    let hints = r#"[{"position":{"line":7,"character":9},"label":[{"value":": "},{"value":"A","location":{"uri":"file:///d:/git/check/src/main.rs","range":{"start":{"line":9,"character":7},"end":{"line":9,"character":8}}}}],"kind":1,"textEdits":[{"range":{"start":{"line":7,"character":9},"end":{"line":7,"character":9}},"newText":": A"}],"paddingLeft":false,"paddingRight":false}]"#;
    let hints = _init_inlay_hint(&rs.1, hints)?;
    let (lines, _) = _init_lines(None, rs, vec![], Some(hints))?;
    Ok(lines)
}

pub fn init_empty() -> Result<DocLines> {
    custom_utils::logger::logger_stdout_debug();
    let file: PathBuf = "../../resources/test_code/empty.rs".into();

    let (lines, _) =
        _init_lines(None, _init_code(file), _init_lsp_folding_range(), None)?;
    Ok(lines)
}

pub fn cursor_insert(start: usize, end: usize) -> Cursor {
    let mode = CursorMode::Insert(Selection::region(start, end));
    Cursor::new(mode, None, None)
}

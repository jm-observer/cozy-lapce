pub mod async_command;
pub mod sync_command;

use ansi_to_style::parse_byte;
use cargo_metadata::{CompilerMessage, Message, diagnostic::DiagnosticLevel};
use cozy_floem::views::{
    panel::{ErrLevel, Hyperlink, TextSrc},
    tree_with_panel::data::{Level, StyledText},
};
use log::warn;

fn resolve_stderr(line: &str) -> StyledText {
    let styled_text = parse_byte(line.as_bytes());
    let (text_src, level) =
        if styled_text.text.as_str().trim_start().starts_with("error") {
            (
                TextSrc::StdErr {
                    level: ErrLevel::Error,
                },
                Level::Error,
            )
        } else {
            (
                TextSrc::StdErr {
                    level: ErrLevel::Other,
                },
                Level::None,
            )
        };
    StyledText {
        id: text_src,
        level,
        styled_text,
        hyperlink: vec![],
    }
}

fn resolve_stdout(line: &str) -> Option<StyledText> {
    if let Ok(parsed) = serde_json::from_str::<Message>(line) {
        match parsed {
            Message::CompilerMessage(msg) => {
                if let Some(rendered) = &msg.message.rendered {
                    let level = match msg.message.level {
                        DiagnosticLevel::Ice | DiagnosticLevel::Error => {
                            Level::Error
                        },
                        DiagnosticLevel::Warning => Level::Warn,
                        _ => Level::None,
                    };

                    let styled_text = parse_byte(rendered.as_bytes());
                    let package_id = msg.package_id.clone();
                    let hyperlink = resolve_hyperlink_from_message(
                        &msg,
                        styled_text.text.as_str(),
                    );
                    let file = hyperlink.iter().find_map(|x| match x {
                        Hyperlink::File { src, .. } => Some(src.clone()),
                        Hyperlink::Url { .. } => None,
                    });
                    let text_src = TextSrc::StdOut {
                        package_id,
                        crate_name: msg.target.name,
                        file,
                    };
                    return Some(StyledText {
                        id: text_src,
                        level,
                        styled_text,
                        hyperlink,
                    });
                }
            },
            Message::CompilerArtifact(_script) => {
                // log::debug!("Compiler Artifact: {:?}",
                // artifact);
            },
            Message::BuildScriptExecuted(_script) => {
                // log::debug!("Build Script Executed:
                // {:?}", script);
            },
            Message::BuildFinished(_script) => {
                // log::debug!("Build Finished: {:?}",
                // script);
            },
            Message::TextLine(_script) => {
                // log::debug!("TextLine: {:?}", script);
            },
            val => {
                log::debug!("??????????: {:?}", val);
            },
        }
    } else {
        log::debug!("Non-JSON stdout: {}", line);
    }
    None
}

fn resolve_hyperlink_from_message(
    msg: &CompilerMessage,
    text: &str,
) -> Vec<Hyperlink> {
    let mut file_hyper: Vec<Hyperlink> = msg
        .message
        .spans
        .iter()
        .filter_map(|x| {
            let full_info =
                format!("{}:{}:{}", x.file_name, x.line_start, x.column_start);
            if let Some(index) = text.find(full_info.as_str()) {
                Some(Hyperlink::File {
                    range:  index..index + full_info.len(),
                    src:    x.file_name.clone(),
                    line:   x.line_start,
                    column: Some(x.column_start),
                })
            } else {
                warn!("not found: {full_info}");
                None
            }
        })
        .collect();
    if let Some(code_hyper) = msg.message.code.as_ref().and_then(|x| {
        text.find(x.code.as_str()).map(|index| {
            Hyperlink::Url {
                range: index..index + x.code.len(),
                // todo
                url:   "".to_string(),
            }
        })
    }) {
        file_hyper.push(code_hyper)
    }
    file_hyper
}

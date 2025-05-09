use std::{path::PathBuf, sync::Arc};

use doc::{
    diagnostic::DiagnosticData,
    lines::{register::Clipboard, text::SystemClipboard},
};
use floem::{
    View,
    event::EventListener,
    peniko::Color,
    reactive::{
        SignalGet, SignalUpdate, SignalWith, create_effect, create_rw_signal,
    },
    style::{CursorStyle, Style},
    views::{Decorators, container, dyn_stack, label, scroll, stack, svg},
};
use lapce_core::{
    icon::LapceIcons,
    panel::{PanelContainerPosition, PanelSection},
    workspace::LapceWorkspace,
};
use lsp_types::{DiagnosticRelatedInformation, DiagnosticSeverity};

use super::view::PanelBuilder;
use crate::{
    command::InternalCommand,
    config::{WithLapceConfig, color::LapceColor},
    doc::EditorDiagnostic,
    editor::location::{EditorLocation, EditorPosition},
    listener::Listener,
    lsp::path_from_url,
    window_workspace::WindowWorkspaceData,
};

pub fn problem_panel(
    window_tab_data: WindowWorkspaceData,
    position: PanelContainerPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let is_bottom = position.is_bottom();
    PanelBuilder::new(config, position)
        .add_style(
            "Errors",
            problem_section(window_tab_data.clone(), DiagnosticSeverity::ERROR),
            window_tab_data.panel.section_open(PanelSection::Error),
            move |s| {
                s.border_color(config.with_color(LapceColor::LAPCE_BORDER))
                    .apply_if(is_bottom, |s| s.border_right(1.0))
                    .apply_if(!is_bottom, |s| s.border_bottom(1.0))
            },
        )
        .add(
            "Warnings",
            problem_section(window_tab_data.clone(), DiagnosticSeverity::WARNING),
            window_tab_data.panel.section_open(PanelSection::Warn),
        )
        .build()
        .debug_name("Problem Panel")
}

fn problem_section(
    window_tab_data: WindowWorkspaceData,
    severity: DiagnosticSeverity,
) -> impl View {
    let config = window_tab_data.common.config;
    let main_split = window_tab_data.main_split.clone();
    let internal_command = window_tab_data.common.internal_command;
    container({
        scroll(
            dyn_stack(
                move || main_split.diagnostics.get(),
                |(p, _)| p.clone(),
                move |(path, diagnostic_data)| {
                    file_view(
                        main_split.common.workspace.clone(),
                        path,
                        diagnostic_data,
                        severity,
                        internal_command,
                        config,
                    )
                },
            )
            .style(|s| s.flex_col().width_pct(100.0).line_height(1.8)),
        )
        .style(|s| s.absolute().size_pct(100.0, 100.0))
    })
    .style(|s| s.size_pct(100.0, 100.0))
}

fn file_view(
    workspace: Arc<LapceWorkspace>,
    path: PathBuf,
    diagnostic_data: DiagnosticData,
    severity: DiagnosticSeverity,
    internal_command: Listener<InternalCommand>,
    config: WithLapceConfig,
) -> impl View {
    let collpased = create_rw_signal(false);

    let diagnostics = create_rw_signal(im::Vector::new());
    create_effect(move |_| {
        let span = diagnostic_data.spans().get();
        let d = if !span.is_empty() {
            span.iter()
                .filter_map(|(iv, diag)| {
                    if diag.severity == Some(severity) {
                        Some(EditorDiagnostic {
                            range:      Some((iv.start, iv.end)),
                            diagnostic: diag.to_owned(),
                        })
                    } else {
                        None
                    }
                })
                .collect::<im::Vector<EditorDiagnostic>>()
        } else {
            let diagnostics = diagnostic_data.diagnostics.get();
            let diagnostics: im::Vector<EditorDiagnostic> = diagnostics
                .into_iter()
                .filter_map(|d| {
                    if d.severity == Some(severity) {
                        Some(EditorDiagnostic {
                            range:      None,
                            diagnostic: d,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            diagnostics
        };
        diagnostics.set(d);
    });

    let full_path = path.clone();
    let path = if let Some(workspace_path) = workspace.path() {
        path.strip_prefix(workspace_path)
            .unwrap_or(&full_path)
            .to_path_buf()
    } else {
        path
    };
    let style_path = path.clone();

    let icon = match severity {
        DiagnosticSeverity::ERROR => LapceIcons::ERROR,
        _ => LapceIcons::WARNING,
    };
    let icon_color = move || match severity {
        DiagnosticSeverity::ERROR => config.with_color(LapceColor::LAPCE_ERROR),
        _ => config.with_color(LapceColor::LAPCE_WARN),
    };

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let folder = path
        .parent()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    stack((
        stack((
            container(
                stack((
                    label(move || file_name.clone()).style(|s| {
                        s.margin_right(6.0)
                            .max_width_pct(100.0)
                            .text_ellipsis()
                            .selectable(false)
                    }),
                    label(move || folder.clone()).style(move |s| {
                        s.color(config.with_color(LapceColor::EDITOR_DIM))
                            .min_width(0.0)
                            .text_ellipsis()
                            .selectable(false)
                    }),
                ))
                .style(move |s| s.width_pct(100.0).min_width(0.0)),
            )
            .on_click_stop(move |_| {
                collpased.update(|collpased| *collpased = !*collpased);
            })
            .style(move |s| {
                let (border_color, icon_size) = config.signal(|config| {
                    (
                        config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                        config.ui.icon_size.signal(),
                    )
                });
                s.width_pct(100.0)
                    .min_width(0.0)
                    .padding_left(10.0 + (icon_size.get() as f32 + 6.0) * 2.0)
                    .padding_right(10.0)
                    .hover(|s| {
                        s.cursor(CursorStyle::Pointer)
                            .background(border_color.get())
                    })
            }),
            stack((
                svg(move || {
                    config.with_ui_svg(if collpased.get() {
                        LapceIcons::ITEM_CLOSED
                    } else {
                        LapceIcons::ITEM_OPENED
                    })
                })
                .style(move |s| {
                    let (border_color, icon_size) = config.signal(|config| {
                        (
                            config.color(LapceColor::LAPCE_ICON_ACTIVE),
                            config.ui.icon_size.signal(),
                        )
                    });
                    let size = icon_size.get() as f32;
                    s.margin_right(6.0)
                        .size(size, size)
                        .color(border_color.get())
                }),
                svg(move || config.with_file_svg(&path).0).style(move |s| {
                    // let (color, icon_size) = config.with(|config| {
                    //     (config.file_svg(&style_path).1, config.ui.icon_size())
                    // });
                    let (size, file_svg) = config.signal(|config| {
                        (config.ui.icon_size.signal(), config.icon_theme.signal())
                    });
                    let color = file_svg.with(|x| x.file_svg(&style_path).1);
                    let size = size.get() as f32;
                    s.min_width(size)
                        .size(size, size)
                        .apply_opt(color, Style::color)
                }),
                label(|| " ".to_string()).style(move |s| s.selectable(false)),
            ))
            .style(|s| s.absolute().items_center().margin_left(10.0)),
        ))
        .style(move |s| s.width_pct(100.0).min_width(0.0)),
        dyn_stack(
            move || {
                if collpased.get() {
                    im::Vector::new()
                } else {
                    diagnostics.get()
                }
            },
            |d| (d.range, d.diagnostic.range),
            move |d| {
                item_view(
                    full_path.clone(),
                    d,
                    icon,
                    icon_color,
                    internal_command,
                    config,
                )
            },
        )
        .style(|s| s.flex_col().width_pct(100.0).min_width_pct(0.0)),
    ))
    .style(move |s| {
        s.width_pct(100.0)
            .items_start()
            .flex_col()
            .apply_if(diagnostics.with(|d| d.is_empty()), |s| s.hide())
    })
    .debug_name("diagnostic file view")
}

fn item_view(
    path: PathBuf,
    d: EditorDiagnostic,
    icon: &'static str,
    icon_color: impl Fn() -> Color + 'static,
    internal_command: Listener<InternalCommand>,
    config: WithLapceConfig,
) -> impl View {
    let related = d.diagnostic.related_information.unwrap_or_default();
    let position = if let Some((start, _)) = d.range {
        EditorPosition::Offset(start)
    } else {
        EditorPosition::Position(d.diagnostic.range.start)
    };
    let location = EditorLocation {
        path,
        position: Some(position),
        scroll_offset: None,
        ignore_unconfirmed: false,
        same_editor_tab: false,
    };
    let message = d.diagnostic.message.clone();
    stack((
        container({
            stack((
                label(move || d.diagnostic.message.clone())
                    .style(move |s| {
                        s.width_pct(100.0)
                            .min_width(0.0)
                            .padding_left(
                                10.0 + (config.with_icon_size() as f32 + 6.0) * 3.0,
                            )
                            .padding_right(10.0)
                    })
                    .on_event_stop(EventListener::SecondaryClick, move |_event| {
                        let mut clipboard = SystemClipboard::new();
                        clipboard.put_string(message.clone());
                        internal_command.send(InternalCommand::ShowStatusMessage {
                            message: "copied message!".to_owned(),
                        })
                    }),
                stack((
                    svg(move || config.with_ui_svg(icon)).style(move |s| {
                        let size = config.with_icon_size() as f32;
                        s.size(size, size).color(icon_color())
                    }),
                    label(|| " ".to_string()).style(move |s| s.selectable(false)),
                ))
                .style(move |s| {
                    s.absolute().items_center().margin_left(
                        10.0 + (config.with_icon_size() as f32 + 6.0) * 2.0,
                    )
                }),
            ))
            .style(move |s| {
                s.width_pct(100.0).min_width(0.0).hover(|s| {
                    s.cursor(CursorStyle::Pointer).background(
                        config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    )
                })
            })
        })
        .on_click_stop(move |_| {
            internal_command.send(InternalCommand::JumpToLocation {
                location: location.clone(),
            });
        })
        .style(|s| s.width_pct(100.0).min_width_pct(0.0)),
        related_view(related, internal_command, config),
    ))
    .style(|s| s.width_pct(100.0).min_width_pct(0.0).flex_col())
}

fn related_view(
    related: Vec<DiagnosticRelatedInformation>,
    internal_command: Listener<InternalCommand>,
    config: WithLapceConfig,
) -> impl View {
    let is_empty = related.is_empty();
    stack((
        dyn_stack(
            move || related.clone(),
            |_| 0,
            move |related| {
                let full_path = path_from_url(&related.location.uri);
                let path = full_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| {
                        format!(
                            "{f} [{}, {}]: ",
                            related.location.range.start.line,
                            related.location.range.start.character
                        )
                    })
                    .unwrap_or_default();
                let location = EditorLocation {
                    path:               full_path,
                    position:           Some(EditorPosition::Position(
                        related.location.range.start,
                    )),
                    scroll_offset:      None,
                    ignore_unconfirmed: false,
                    same_editor_tab:    false,
                };
                let message = format!("{path}{}", related.message);
                let copy_message = message.clone();
                container(
                    label(move || message.clone())
                        .style(move |s| s.width_pct(100.0).min_width(0.0)),
                )
                .on_click_stop(move |_| {
                    internal_command.send(InternalCommand::JumpToLocation {
                        location: location.clone(),
                    });
                })
                .on_event_stop(EventListener::SecondaryClick, move |_event| {
                    let mut clipboard = SystemClipboard::new();
                    clipboard.put_string(copy_message.clone());
                    internal_command.send(InternalCommand::ShowStatusMessage {
                        message: "copied message!".to_owned(),
                    })
                })
                .style(move |s| {
                    let (icon_size, color) = config.signal(|config| {
                        (
                            config.ui.icon_size.signal(),
                            config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                        )
                    });
                    s.padding_left(10.0 + (icon_size.get() as f32 + 6.0) * 4.0)
                        .padding_right(10.0)
                        .width_pct(100.0)
                        .min_width(0.0)
                        .hover(|s| {
                            s.cursor(CursorStyle::Pointer).background(color.get())
                        })
                })
            },
        )
        .style(|s| s.width_pct(100.0).min_width(0.0).flex_col()),
        stack((
            svg(move || config.with_ui_svg(LapceIcons::LINK)).style(move |s| {
                let (size, color) = config.signal(|config| {
                    (
                        config.ui.icon_size.signal(),
                        config.color(LapceColor::EDITOR_DIM),
                    )
                });
                let size = size.get() as f32;
                s.size(size, size).color(color.get())
            }),
            label(|| " ".to_string()).style(move |s| s.selectable(false)),
        ))
        .style(move |s| {
            s.absolute()
                .items_center()
                .margin_left(10.0 + (config.with_icon_size() as f32 + 6.0) * 3.0)
        }),
    ))
    .style(move |s| {
        s.width_pct(100.0)
            .min_width(0.0)
            .items_start()
            .color(config.with_color(LapceColor::EDITOR_DIM))
            .apply_if(is_empty, |s| s.hide())
    })
}

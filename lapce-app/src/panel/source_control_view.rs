use std::path::PathBuf;

use doc::lines::buffer::rope_text::RopeText;
use floem::{
    View,
    action::show_context_menu,
    event::{Event, EventListener},
    kurbo::Affine,
    menu::{Menu, MenuItem},
    peniko::kurbo::Rect,
    reactive::{Scope, SignalGet, SignalTrack, SignalUpdate, SignalWith},
    style::{CursorStyle, Style},
    views::{Decorators, container, dyn_stack, label, scroll, stack, text}
};
use lapce_core::{
    icon::LapceIcons,
    panel::{PanelContainerPosition, PanelKind, PanelSection}
};
use lapce_rpc::source_control::FileDiff;

use super::view::foldable_panel_section;
use crate::{
    command::{CommandKind, InternalCommand, LapceCommand, LapceWorkbenchCommand},
    config::color::LapceColor,
    editor::{floem_editor::cursor_caret_v2, view::editor_view},
    settings::checkbox,
    source_control::SourceControlData,
    svg,
    window_workspace::{Focus, WindowWorkspaceData}
};
pub fn source_control_panel(
    window_tab_data: WindowWorkspaceData,
    _position: PanelContainerPosition
) -> impl View {
    let scope = window_tab_data.scope;
    let config = window_tab_data.common.config;
    let source_control = window_tab_data.source_control.clone();
    let focus = source_control.common.focus;
    let editor = source_control.editor.clone();
    let doc = editor.doc_signal();
    let cursor = editor.cursor();
    let lines = editor.editor.doc().lines;
    let window_origin = editor.window_origin();
    let editor = scope.create_rw_signal(editor);
    let is_active = move |tracked| {
        let focus = if tracked {
            focus.get()
        } else {
            focus.get_untracked()
        };
        focus == Focus::Panel(PanelKind::SourceControl)
    };
    let is_empty = scope.create_memo(move |_| {
        let doc = doc.get().lines.with_untracked(|x| x.signal_buffer());
        doc.with(|b| b.len() == 0)
    });
    let debug_breakline = scope.create_memo(move |_| None);

    stack((
        stack((
            container({
                scroll({
                    let view = stack((
                        editor_view(
                            editor.get_untracked(),
                            debug_breakline,
                            is_active,
                            "source control"
                        )
                        .style(|x| x.width_pct(100.0).min_width(100.0)),
                        label(|| "Commit Message".to_string()).style(move |s| {
                            let (caret_color, line_height) = config.with(|config| {
                                (
                                    config.color(LapceColor::EDITOR_DIM),
                                    config.editor.line_height() as f32
                                )
                            });
                            s.absolute()
                                .items_center()
                                .height(line_height)
                                .color(caret_color)
                                .apply_if(!is_empty.get(), |s| s.hide())
                                .selectable(false)
                        })
                    ))
                    .style(|s| {
                        s.absolute()
                            .min_size_pct(100.0, 100.0)
                            .padding_left(10.0)
                            .padding_vert(6.0)
                            .hover(|s| s.cursor(CursorStyle::Text))
                    });
                    let id = view.id();
                    view.on_event_cont(EventListener::PointerDown, move |event| {
                        let event =
                            event.clone().transform(Affine::translate((10.0, 6.0)));
                        if let Event::PointerDown(pointer_event) = event {
                            id.request_active();
                            editor.get_untracked().pointer_down(&pointer_event);
                        }
                    })
                    .on_event_stop(EventListener::PointerMove, move |event| {
                        let event =
                            event.clone().transform(Affine::translate((10.0, 6.0)));
                        if let Event::PointerMove(pointer_event) = event {
                            editor.get_untracked().pointer_move(&pointer_event);
                        }
                    })
                    .on_event_stop(
                        EventListener::PointerUp,
                        move |event| {
                            let event = event
                                .clone()
                                .transform(Affine::translate((10.0, 6.0)));
                            if let Event::PointerUp(pointer_event) = event {
                                editor.get_untracked().pointer_up(&pointer_event);
                            }
                        }
                    )
                })
                .on_move(move |pos| {
                    window_origin.set(pos + (10.0, 6.0));
                })
                .on_scroll(move |rect| {
                    lines.update(|x| x.update_viewport_by_scroll(rect));
                })
                .ensure_visible(move || {
                    let cursor = cursor.get();
                    let offset = cursor.offset();
                    let e_data = editor.get_untracked();
                    e_data.doc_signal().track();
                    e_data.kind().track();

                    if let Some((x, y, width, line_height)) =
                        cursor_caret_v2(&e_data.editor, offset, cursor.affinity)
                    {
                        let rect =
                            Rect::from_origin_size((x, y), (width, line_height));
                        rect.inflate(30.0, 10.0)
                    } else {
                        Rect::ZERO
                    }
                    //
                    // let LineRegion { x, width, rvline } = match cursor_caret(
                    //     &e_data.editor,
                    //     offset,
                    //     !cursor.is_insert(),
                    //     cursor.affinity,
                    // ) {
                    //     Ok(rs) => rs,
                    //     Err(err) => {
                    //         error!("{err:?}");
                    //         return Rect::ZERO;
                    //     }
                    // };
                    // let config = config.get_untracked();
                    // let line_height = config.editor.line_height();
                    // // TODO: is there a way to avoid the calculation of the
                    // vline here? let vline = match
                    // e_data.editor.vline_of_rvline(rvline) {
                    //     Ok(vline) => vline,
                    //     Err(err) => {
                    //         error!("{:?}", err);
                    //         return Rect::ZERO;
                    //     }
                    // };
                    // Rect::from_origin_size(
                    //     (x, (vline.get() * line_height) as f64),
                    //     (width, line_height as f64),
                    // )
                    // .inflate(30.0, 10.0)
                })
                .style(|s| s.absolute().size_pct(100.0, 100.0))
            })
            .style(move |s| {
                let (caret_color, bg) = config.with(|config| {
                    (
                        config.color(LapceColor::LAPCE_BORDER),
                        config.color(LapceColor::EDITOR_BACKGROUND)
                    )
                });
                s.width_pct(100.0)
                    .height(120.0)
                    .border(1.0)
                    .padding(-1.0)
                    .border_radius(6.0)
                    .border_color(caret_color)
                    .background(bg)
            }),
            {
                let source_control = source_control.clone();
                label(|| "Commit".to_string())
                    .on_click_stop(move |_| {
                        source_control.commit();
                    })
                    .style(move |s| {
                        let (caret_color, bg, abg) = config.with(|config| {
                            (
                                config.color(LapceColor::LAPCE_BORDER),
                                config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                                config.color(
                                    LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND
                                )
                            )
                        });
                        s.margin_top(10.0)
                            .line_height(1.6)
                            .width_pct(100.0)
                            .justify_center()
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(caret_color)
                            .hover(|s| s.cursor(CursorStyle::Pointer).background(bg))
                            .active(|s| s.background(abg))
                            .selectable(false)
                    })
            }
        ))
        .style(|s| s.flex_col().width_pct(100.0).padding(10.0)),
        foldable_panel_section(
            text("Changes"),
            file_diffs_view(source_control, scope),
            window_tab_data.panel.section_open(PanelSection::Changes),
            config
        )
        .style(|s| s.flex_col().size_pct(100.0, 100.0))
    ))
    .on_event_stop(EventListener::PointerDown, move |_| {
        if focus.get_untracked() != Focus::Panel(PanelKind::SourceControl) {
            focus.set(Focus::Panel(PanelKind::SourceControl));
        }
    })
    .style(|s| s.flex_col().size_pct(100.0, 100.0))
    .debug_name("Source Control Panel")
}

fn file_diffs_view(source_control: SourceControlData, scope: Scope) -> impl View {
    let file_diffs = source_control.file_diffs;
    let config = source_control.common.config;
    let workspace = source_control.common.workspace.clone();
    let panel_rect = scope.create_rw_signal(Rect::ZERO);
    let panel_width = scope.create_memo(move |_| panel_rect.get().width());
    let lapce_command = source_control.common.lapce_command;
    let internal_command = source_control.common.internal_command;

    let view_fn = move |(path, (diff, checked)): (PathBuf, (FileDiff, bool))| {
        let diff_for_style = diff.clone();
        let full_path = path.clone();
        let diff_for_menu = diff.clone();
        let path_for_click = full_path.clone();

        let path = if let Some(workspace_path) = workspace.path.as_ref() {
            path.strip_prefix(workspace_path)
                .unwrap_or(&full_path)
                .to_path_buf()
        } else {
            path
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
        let style_path = path.clone();
        stack((
            checkbox(move || checked, config)
                .style(|s| s.hover(|s| s.cursor(CursorStyle::Pointer)))
                .on_click_stop(move |_| {
                    file_diffs.update(|diffs| {
                        if let Some((_, checked)) = diffs.get_mut(&full_path) {
                            *checked = !*checked;
                        }
                    });
                }),
            svg(move || config.with_file_svg(&path).0).style(move |s| {
                let (size, color) = config.with(|config| {
                    (config.ui.icon_size() as f32, config.file_svg(&style_path).1)
                });
                s.min_width(size)
                    .size(size, size)
                    .margin(6.0)
                    .apply_opt(color, Style::color)
            }),
            label(move || file_name.clone()).style(move |s| {
                let size = config.with_icon_size() as f32;
                let max_width = panel_width.get() as f32
                    - 10.0
                    - size
                    - 6.0
                    - size
                    - 6.0
                    - 10.0
                    - size
                    - 6.0;
                s.text_ellipsis()
                    .margin_right(6.0)
                    .max_width(max_width)
                    .selectable(false)
            }),
            label(move || folder.clone()).style(move |s| {
                s.text_ellipsis()
                    .flex_grow(1.0)
                    .flex_basis(0.0)
                    .color(config.with_color(LapceColor::EDITOR_DIM))
                    .min_width(0.0)
                    .selectable(false)
            }),
            container({
                svg(move || {
                    let svg = match &diff {
                        FileDiff::Modified(_) => LapceIcons::SCM_DIFF_MODIFIED,
                        FileDiff::Added(_) => LapceIcons::SCM_DIFF_ADDED,
                        FileDiff::Deleted(_) => LapceIcons::SCM_DIFF_REMOVED,
                        FileDiff::Renamed(_, _) => LapceIcons::SCM_DIFF_RENAMED
                    };
                    config.with_ui_svg(svg)
                })
                .style(move |s| {
                    let color = match &diff_for_style {
                        FileDiff::Modified(_) => LapceColor::SOURCE_CONTROL_MODIFIED,
                        FileDiff::Added(_) => LapceColor::SOURCE_CONTROL_ADDED,
                        FileDiff::Deleted(_) => LapceColor::SOURCE_CONTROL_REMOVED,
                        FileDiff::Renamed(_, _) => {
                            LapceColor::SOURCE_CONTROL_MODIFIED
                        },
                    };
                    let (size, color) = config.with(|config| {
                        (config.ui.icon_size() as f32, config.color(color))
                    });
                    s.min_width(size).size(size, size).color(color)
                })
            })
            .style(|s| {
                s.absolute()
                    .size_pct(100.0, 100.0)
                    .padding_right(20.0)
                    .items_center()
                    .justify_end()
            })
        ))
        .on_click_stop(move |_| {
            internal_command.send(InternalCommand::OpenFileChanges {
                path: path_for_click.clone()
            });
        })
        .on_event_cont(EventListener::PointerDown, move |event| {
            let diff_for_menu = diff_for_menu.clone();

            let discard = move || {
                lapce_command.send(LapceCommand {
                    kind: CommandKind::Workbench(
                        LapceWorkbenchCommand::SourceControlDiscardTargetFileChanges
                    ),
                    data: Some(serde_json::json!(diff_for_menu.clone()))
                });
            };

            if let Event::PointerDown(pointer_event) = event {
                if pointer_event.button.is_secondary() {
                    let menu = Menu::new("")
                        .entry(MenuItem::new("Discard Changes").action(discard));
                    show_context_menu(menu, None);
                }
            }
        })
        .style(move |s| {
            let (size, color) = config.with(|config| {
                (
                    config.ui.icon_size() as f32,
                    config.color(LapceColor::PANEL_HOVERED_BACKGROUND)
                )
            });
            s.padding_left(10.0)
                .padding_right(10.0 + size + 6.0)
                .width_pct(100.0)
                .items_center()
                .hover(|s| s.background(color))
        })
    };

    container({
        scroll({
            dyn_stack(
                move || file_diffs.get(),
                |(path, (diff, checked))| {
                    (path.to_path_buf(), diff.clone(), *checked)
                },
                view_fn
            )
            .style(|s| s.line_height(1.6).flex_col().width_pct(100.0))
        })
        .style(|s| s.absolute().size_pct(100.0, 100.0))
    })
    .on_resize(move |rect| {
        panel_rect.set(rect);
    })
    .style(|s| s.size_pct(100.0, 100.0))
}

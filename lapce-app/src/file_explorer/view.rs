use std::path::Path;

use floem::{
    View,
    event::{Event, EventListener},
    keyboard::{Key, NamedKey},
    kurbo::Rect,
    peniko::{Color, color::Rgba8},
    prelude::text_input,
    reactive::{
        ReadSignal, RwSignal, SignalGet, SignalUpdate, SignalWith, create_rw_signal,
    },
    style::{AlignItems, Position, Style},
    text::Style as FontStyle,
    views::{
        Container, Decorators, container, dyn_stack, label, stack, svg,
        virtual_stack,
    },
};
use lapce_core::{
    icon::LapceIcons,
    panel::{PanelContainerPosition, PanelSection},
};
use lapce_rpc::{
    file::{FileNodeViewData, FileNodeViewKind, Naming},
    source_control::FileDiffKind,
};

use super::{data::FileExplorerData, node::FileNodeVirtualList};
use crate::{
    app::clickable_icon,
    command::InternalCommand,
    config::{WithLapceConfig, color::LapceColor},
    editor_tab::{EditorTabChildId, EditorTabManageData},
    panel::view::PanelBuilder,
    plugin::PluginData,
    source_control::SourceControlData,
    window_workspace::WindowWorkspaceData,
};

/// Blends `foreground` with `background`.
///
/// Uses the alpha channel from `foreground` - if `foreground` is opaque,
/// `foreground` will be returned unchanged.
///
/// The result is always opaque regardless of the transparency of the inputs.
fn blend_colors(background: Color, foreground: Color) -> Color {
    let Rgba8 {
        r: background_r,
        g: background_g,
        b: background_b,
        ..
    } = background.to_rgba8();
    let Rgba8 {
        r: foreground_r,
        g: foreground_g,
        b: foreground_b,
        a,
    } = foreground.to_rgba8();
    let a: u16 = a.into();

    let [r, g, b] = [
        [background_r, foreground_r],
        [background_g, foreground_g],
        [background_b, foreground_b],
    ]
    .map(|x| x.map(u16::from))
    .map(|[b, f]| (a * f + (255 - a) * b) / 255)
    .map(|x| x as u8);
    Color::from_rgb8(r, g, b)
}

pub fn file_explorer_panel(
    window_tab_data: WindowWorkspaceData,
    position: PanelContainerPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let data = window_tab_data.file_explorer.clone();
    let source_control = window_tab_data.source_control.clone();
    PanelBuilder::new(config, position)
        .add_height_style(
            "Open Editors",
            150.0,
            container(open_editors_view(window_tab_data.clone()))
                .style(|s| s.size_full()),
            window_tab_data.panel.section_open(PanelSection::OpenEditor),
            move |s| {
                s.apply_if(
                    !config.signal(|x| x.ui.open_editors_visible.signal()).get(),
                    |s| s.hide(),
                )
            },
        )
        .add(
            "File Explorer",
            container(file_explorer_view(data, source_control))
                .style(|s| s.size_full()),
            window_tab_data
                .panel
                .section_open(PanelSection::FileExplorer),
        )
        .build()
        .debug_name("File Explorer Panel")
}

/// Initialize the file explorer's naming (renaming, creating, etc.) editor with
/// the given path.
fn initialize_naming_editor_with_path(data: &FileExplorerData, path: &Path) {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    // Start with the part of the file or directory name before the extension
    // selected.
    // let selection_end = {
    //     let without_leading_dot =
    // file_name.strip_prefix('.').unwrap_or(&file_name);     let idx =
    // without_leading_dot         .find('.')
    //         .unwrap_or(without_leading_dot.len());
    //
    //     idx + file_name.len() - without_leading_dot.len()
    // };

    initialize_naming_editor(data, &file_name);
}

fn initialize_naming_editor(data: &FileExplorerData, text: &str) {
    // let text = Rope::from(text);
    // let selection_end = selection_end.unwrap_or(text.len());
    //
    // let doc = data.naming_editor_data.doc();
    // doc.reload(text, true);
    // data.naming_editor_data
    //     .cursor()
    //     .update(|cursor| cursor.set_insert(Selection::region(0, selection_end)));

    data.naming_str.set(text.to_string());

    data.naming
        .update(|naming| naming.set_editor_needs_reset(false));
}

fn file_node_text_color(
    config: WithLapceConfig,
    node: FileNodeViewData,
    source_control: SourceControlData,
) -> Color {
    let diff = source_control.file_diffs.with(|file_diffs| {
        let FileNodeViewKind::Path(path) = &node.kind else {
            return None;
        };

        if node.is_dir {
            file_diffs
                .keys()
                .find(|p| p.as_path().starts_with(path))
                .map(|_| FileDiffKind::Modified)
        } else {
            file_diffs.get(path).map(|(diff, _)| diff.kind())
        }
    });

    let color = match diff {
        Some(FileDiffKind::Modified | FileDiffKind::Renamed) => {
            LapceColor::SOURCE_CONTROL_MODIFIED
        },
        Some(FileDiffKind::Added) => LapceColor::SOURCE_CONTROL_ADDED,
        Some(FileDiffKind::Deleted) => LapceColor::SOURCE_CONTROL_REMOVED,
        None => LapceColor::PANEL_FOREGROUND,
    };

    config.with_color(color)
}

fn file_node_text_view(
    data: FileExplorerData,
    node: FileNodeViewData,
    source_control: SourceControlData,
) -> impl View {
    let config = data.common.config;
    let ui_line_height = data.common.ui_line_height;

    match node.kind.clone() {
        FileNodeViewKind::Path(path) => {
            if node.is_root {
                let file = path.clone();
                container((
                    label(move || {
                        file.file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default()
                    })
                    .style(move |s| {
                        s.height(ui_line_height.get())
                            .color(file_node_text_color(
                                config,
                                node.clone(),
                                source_control.clone(),
                            ))
                            .padding_right(5.0)
                            .selectable(false)
                    }),
                    label(move || path.to_string_lossy().to_string()).style(
                        move |s| {
                            s.height(ui_line_height.get())
                                .color(
                                    config.with_color(
                                        LapceColor::PANEL_FOREGROUND_DIM,
                                    ),
                                )
                                .selectable(false)
                        },
                    ),
                ))
            } else {
                container(
                    label(move || {
                        path.file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default()
                    })
                    .style(move |s| {
                        s.height(ui_line_height.get())
                            .color(file_node_text_color(
                                config,
                                node.clone(),
                                source_control.clone(),
                            ))
                            .selectable(false)
                    }),
                )
            }
        },
        FileNodeViewKind::Renaming { path, err } => {
            if data.naming.with_untracked(Naming::editor_needs_reset) {
                initialize_naming_editor_with_path(&data, &path);
            }

            file_node_input_view(data, err.clone())
        },
        FileNodeViewKind::Naming { err } => {
            if data.naming.with_untracked(Naming::editor_needs_reset) {
                initialize_naming_editor(&data, "");
            }

            file_node_input_view(data, err.clone())
        },
        FileNodeViewKind::Duplicating { source, err } => {
            if data.naming.with_untracked(Naming::editor_needs_reset) {
                initialize_naming_editor_with_path(&data, &source);
            }

            file_node_input_view(data, err.clone())
        },
    }
}

/// Input used for naming a file/directory
fn file_node_input_view(data: FileExplorerData, err: Option<String>) -> Container {
    let ui_line_height = data.common.ui_line_height;

    let naming_str = data.naming_str;
    let config = data.common.config;

    let data_keydown = data.clone();
    let text_input_view = text_input(naming_str)
        .on_event_stop(EventListener::KeyDown, move |event: &Event| {
            if let Event::KeyDown(key_event) = event
                && let Key::Named(NamedKey::Enter) = key_event.key.logical_key
            {
                data_keydown.finish_naming();
                data_keydown.naming.set(Naming::None);
            }
        })
        .on_event_stop(EventListener::FocusLost, move |event| {
            log::info!("FocusLost {} {:?}", naming_str.get_untracked(), event);
            data.finish_naming();
            data.naming.set(Naming::None);
        })
        .style(move |s| {
            s.width_full()
                .height(ui_line_height.get())
                .border(1.0)
                .border_color(config.with_color(LapceColor::LAPCE_BORDER))
        })
        .debug_name("");

    let text_input_id = text_input_view.id();
    text_input_id.request_focus();

    if let Some(err) = err {
        container(
            stack((
                text_input_view,
                label(move || err.clone()).style(move |s| {
                    let (error_background_color, error_fg, editor_background_color) =
                        config.signal(|config| {
                            (
                                config
                                    .color(LapceColor::ERROR_LENS_ERROR_BACKGROUND),
                                config
                                    .color(LapceColor::ERROR_LENS_ERROR_FOREGROUND),
                                config.color(LapceColor::PANEL_CURRENT_BACKGROUND),
                            )
                        });

                    let background_color = blend_colors(
                        editor_background_color.get(),
                        error_background_color.get(),
                    );

                    s.position(Position::Absolute)
                        .inset_top(ui_line_height.get())
                        .width_full()
                        .color(error_fg.get())
                        .background(background_color)
                        .z_index(100)
                }),
            ))
            .style(|s| s.flex_grow(1.0)),
        )
    } else {
        container(text_input_view)
    }
    .style(move |s| s.width_full())
}

fn file_explorer_view(
    data: FileExplorerData,
    source_control: SourceControlData,
) -> impl View {
    let root = data.root;
    let ui_line_height = data.common.ui_line_height;
    let config = data.common.config;
    let naming = data.naming;
    let scroll_to_line = data.scroll_to_line;
    let select = data.select;
    let secondary_click_data = data.clone();
    let scroll_rect = create_rw_signal(Rect::ZERO);

    scroll(
        virtual_stack(
            // VirtualDirection::Vertical,
            // VirtualItemSize::Fixed(Box::new(move || ui_line_height.get())),
            move || FileNodeVirtualList::new(root.get(), data.naming.get()),
            move |node| (node.kind.clone(), node.is_dir, node.open, node.level),
            move |node| {
                let level = node.level;
                let data = data.clone();
                let click_data = data.clone();
                let double_click_data = data.clone();
                let secondary_click_data = data.clone();
                let aux_click_data = data.clone();
                let kind = node.kind.clone();
                let open = node.open;
                let is_dir = node.is_dir;

                let svg_data = data.clone();
                let view = stack((
                    svg(move || {
                        let svg_str = match open {
                            true => LapceIcons::ITEM_OPENED,
                            false => LapceIcons::ITEM_CLOSED,
                        };
                        config.with_ui_svg(svg_str)
                    })
                    .on_click_stop({
                        let kind = kind.clone();
                        move |_| {
                            if is_dir && let FileNodeViewKind::Path(path) = &kind {
                                svg_data.toggle_expand(path);
                            }
                        }
                    })
                    .style(move |s| {
                        let (size, color) = config.signal(|config| {
                            (
                                config.ui.icon_size.signal(),
                                config.color(LapceColor::LAPCE_ICON_ACTIVE),
                            )
                        });
                        let size = size.get() as f32;
                        let color = if is_dir {
                            color.get()
                        } else {
                            Color::TRANSPARENT
                        };
                        s.size(size, size)
                            .flex_shrink(0.0)
                            .margin_left(10.0)
                            .color(color)
                    }),
                    {
                        let kind = kind.clone();
                        let kind_for_style = kind.clone();
                        // TODO: use the current naming input as the path for the
                        // file svg
                        svg(move || {
                            if is_dir {
                                let svg_str = match open {
                                    true => LapceIcons::DIRECTORY_OPENED,
                                    false => LapceIcons::DIRECTORY_CLOSED,
                                };
                                config.with_ui_svg(svg_str)
                            } else if let Some(path) = kind.path() {
                                config.with_file_svg(path).0
                            } else {
                                config.with_ui_svg(LapceIcons::FILE)
                            }
                        })
                        .style(move |s| {
                            let (size, color, file_svg) = config.signal(|config| {
                                (
                                    config.ui.icon_size.signal(),
                                    config.color(LapceColor::LAPCE_ICON_ACTIVE),
                                    config.icon_theme.signal(),
                                )
                            });
                            let file_svg = kind_for_style
                                .path()
                                .and_then(|p| file_svg.with(|x| x.file_svg(p).1));
                            let size = size.get() as f32;
                            s.size(size, size)
                                .flex_shrink(0.0)
                                .margin_horiz(6.0)
                                .apply_if(is_dir, |s| s.color(color.get()))
                                .apply_if(!is_dir, |s| {
                                    s.apply_opt(file_svg, Style::color)
                                })
                        })
                    },
                    file_node_text_view(data, node, source_control.clone()),
                ))
                .style({
                    let kind = kind.clone();
                    move |s| {
                        s.padding_right(15.0)
                            .min_width_full()
                            .padding_left((level * 10) as f32)
                            .align_items(AlignItems::Center)
                            // .hover(|s| {
                            //     s.background(config.with_color(
                            //         LapceColor::PANEL_HOVERED_BACKGROUND,
                            //     ))
                            //     .cursor(CursorStyle::Pointer)
                            // })
                            .apply_if(
                                select.get().map(|x| x == kind).unwrap_or_default(),
                                |x| {
                                    x.background(config.with_color(
                                        LapceColor::PANEL_CURRENT_BACKGROUND,
                                    ))
                                },
                            )
                    }
                })
                .debug_name("file item");

                // Only handle click events if we are not naming the file node
                if let FileNodeViewKind::Path(path) = &kind {
                    let click_path = path.clone();
                    let double_click_path = path.clone();
                    let secondary_click_path = path.clone();
                    let aux_click_path = path.clone();
                    view.on_click_stop({
                        let kind = kind.clone();
                        move |_| {
                            click_data.click(&click_path, config);
                            select.update(|x| *x = Some(kind.clone()));
                        }
                    })
                    .on_double_click({
                        move |_| {
                            double_click_data
                                .double_click(&double_click_path, config)
                        }
                    })
                    .on_secondary_click_stop(move |_| {
                        secondary_click_data.secondary_click(&secondary_click_path);
                    })
                    .on_event_stop(
                        EventListener::PointerDown,
                        move |event| {
                            if let Event::PointerDown(pointer_event) = event
                                && pointer_event.button.is_auxiliary()
                            {
                                aux_click_data.middle_click(&aux_click_path);
                            }
                        },
                    )
                } else {
                    view
                }
            },
        )
        .style(|s| s.absolute().flex_col().min_width_full()),
    )
    .style(|s| s.absolute().size_full().line_height(1.8))
    .on_secondary_click_stop(move |_| {
        if let Naming::None = naming.get_untracked()
            && let Some(path) = &secondary_click_data.common.workspace.path()
        {
            secondary_click_data.secondary_click(path);
        }
    })
    .on_resize(move |rect| {
        scroll_rect.set(rect);
    })
    .scroll_to(move || {
        if let Some(line) = scroll_to_line.get() {
            let line_height = ui_line_height.get_untracked();
            Some(
                (
                    0.0,
                    line * line_height - scroll_rect.get_untracked().height() / 2.0,
                )
                    .into(),
            )
        } else {
            None
        }
    })
}

fn open_editors_view(window_tab_data: WindowWorkspaceData) -> impl View {
    let diff_editors = window_tab_data.main_split.diff_editors;
    let editors = window_tab_data.main_split.editors;
    let editor_tabs = window_tab_data.main_split.editor_tabs;
    let config = window_tab_data.common.config;
    let internal_command = window_tab_data.common.internal_command;
    let active_editor_tab = window_tab_data.main_split.active_editor_tab;
    let plugin = window_tab_data.plugin.clone();

    let child_view = move |plugin: PluginData,
                           editor_tab: RwSignal<EditorTabManageData>,
                           child_index: ReadSignal<usize>,
                           child: EditorTabChildId| {
        let (confirmed, editor_tab_id) = editor_tab.with_untracked(|editor_tab| {
            (
                editor_tab.active_child().confirmed_mut(),
                editor_tab.editor_tab_manage_id,
            )
        });
        let child_for_close = child.clone();
        let info = child.view_info(editors, diff_editors, plugin, config, confirmed);
        let hovered = create_rw_signal(false);

        stack((
            clickable_icon(
                move || {
                    if hovered.get() || info.with(|info| info.is_pristine) {
                        LapceIcons::CLOSE
                    } else {
                        LapceIcons::UNSAVED
                    }
                },
                move || {
                    let editor_tab_id =
                        editor_tab.with_untracked(|t| t.editor_tab_manage_id);
                    internal_command.send(InternalCommand::EditorTabChildClose {
                        editor_tab_id,
                        child: child_for_close.clone(),
                    });
                },
                || false,
                || false,
                || "Close",
                config,
            )
            .on_event_stop(EventListener::PointerEnter, move |_| {
                hovered.set(true);
            })
            .on_event_stop(EventListener::PointerLeave, move |_| {
                hovered.set(false);
            })
            .on_event_stop(EventListener::PointerDown, |_| {})
            .style(|s| s.margin_left(10.0)),
            container(svg(move || info.with(|info| info.icon.clone())).style(
                move |s| {
                    let size = config.with_icon_size() as f32;
                    s.size(size, size)
                        .apply_opt(info.with(|info| info.color), |s, c| s.color(c))
                },
            ))
            .style(|s| s.padding_horiz(6.0)),
            label(move || info.with(|info| info.name.clone())).style(move |s| {
                s.apply_if(!info.with(|info| info.confirmed.get()), |s| {
                    s.font_style(FontStyle::Italic)
                })
            }),
        ))
        .style(move |s| {
            let (hbg, cbg) = config.signal(|config| {
                (
                    config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    config.color(LapceColor::PANEL_CURRENT_BACKGROUND),
                )
            });
            s.items_center()
                .width_pct(100.0)
                .apply_if(
                    active_editor_tab.get() == Some(editor_tab_id)
                        && editor_tab.with(|editor_tab| editor_tab.active)
                            == child_index.get(),
                    |s| s.background(cbg.get()),
                )
                .hover(|s| s.background(hbg.get()))
        })
        .on_event_cont(EventListener::PointerDown, move |_| {
            editor_tab.update(|editor_tab| {
                editor_tab.active = child_index.get_untracked();
            });
            active_editor_tab.set(Some(editor_tab_id));
        })
    };

    scroll(
        dyn_stack(
            move || editor_tabs.get().into_iter().enumerate(),
            move |(index, (editor_tab_id, _))| (*index, *editor_tab_id),
            move |(index, (_, editor_tab))| {
                let plugin = plugin.clone();
                stack((
                    label(move || format!("Group {}", index + 1))
                        .style(|s| s.margin_left(10.0)),
                    dyn_stack(
                        move || editor_tab.get().children,
                        move |child| child.id().id(),
                        move |child| {
                            let child_index = child.read_index();
                            let child_id = child.id().clone();
                            child_view(
                                plugin.clone(),
                                editor_tab,
                                child_index,
                                child_id,
                            )
                        },
                    )
                    .style(|s| s.flex_col().width_pct(100.0)),
                ))
                .style(|s| s.flex_col())
            },
        )
        .style(|s| s.flex_col().width_pct(100.0)),
    )
    .style(|s| s.absolute().size_full().line_height(1.8))
    .debug_name("Open Editors")
}

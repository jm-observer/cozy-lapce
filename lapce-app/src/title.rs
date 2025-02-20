use floem::{
    View,
    event::EventListener,
    menu::{Menu, MenuItem},
    peniko::Color,
    prelude::palette,
    reactive::{
        ReadSignal, RwSignal, SignalGet, SignalUpdate, SignalWith, create_memo
    },
    style::{AlignItems, CursorStyle, JustifyContent},
    views::{Decorators, container, drag_window_area, empty, label, stack}
};
use lapce_core::{icon::LapceIcons, meta, workspace::LapceWorkspace};
use lapce_rpc::proxy::ProxyStatus;

use crate::{
    app::{
        clickable_icon, clickable_icon_base_with_color, not_clickable_icon,
        tooltip_label, window_menu
    },
    command::{LapceCommand, LapceWorkbenchCommand, WindowCommand},
    config::{color::LapceColor},
    listener::Listener,
    main_split::MainSplitData,
    svg,
    update::ReleaseInfo,
    window_workspace::WindowWorkspaceData
};
use crate::config::WithLapceConfig;

fn left(
    workspace: LapceWorkspace,
    lapce_command: Listener<LapceCommand>,
    workbench_command: Listener<LapceWorkbenchCommand>,
    config: WithLapceConfig,
    proxy_status: RwSignal<Option<ProxyStatus>> // num_window_tabs: Memo<usize>,
) -> impl View {
    let is_local = workspace.kind.is_local();
    let is_macos = cfg!(target_os = "macos");
    stack((
        container(svg(move || config.with_ui_svg(LapceIcons::LOGO)).style(
            move |s| {
                s.size(16.0, 16.0)
                    .color(config.with_color(LapceColor::LAPCE_ICON_ACTIVE))
            }
        ))
        .style(move |s| s.margin_horiz(10.0).apply_if(is_macos, |s| s.hide())),
        not_clickable_icon(
            || LapceIcons::MENU,
            || false,
            || false,
            || "Menu",
            config
        )
        .popout_menu(move || window_menu(lapce_command, workbench_command))
        .style(move |s| {
            s.margin_left(4.0)
                .margin_right(6.0)
                .apply_if(is_macos, |s| s.hide())
        }),
        tooltip_label(
            config,
            container(svg(move || config.with_ui_svg(LapceIcons::REMOTE)).style(
                move |s| {
                    let (size, bg) = config.with(|config| {
                        (
                            (config.ui.icon_size() as f32 + 2.0).min(30.0), config.color(LapceColor::LAPCE_ICON_ACTIVE)
                        )
                    });
                    s.size(size, size).color(if is_local {
                        bg
                    } else {
                        match proxy_status.get() {
                            Some(_) => Color::WHITE,
                            None => bg
                        }
                    })
                }
            )),
            || "Connect to Remote"
        )
        .popout_menu(move || {
            #[allow(unused_mut)]
            let mut menu = Menu::new("").entry(
                MenuItem::new("Connect to SSH Host").action(move || {
                    workbench_command.send(LapceWorkbenchCommand::ConnectSshHost);
                })
            );
            if !is_local
                && proxy_status.get().is_some_and(|p| {
                    matches!(p, ProxyStatus::Connecting | ProxyStatus::Connected)
                })
            {
                menu = menu.entry(MenuItem::new("Disconnect remote").action(
                    move || {
                        workbench_command
                            .send(LapceWorkbenchCommand::DisconnectRemote);
                    }
                ));
            }
            #[cfg(windows)]
            {
                menu = menu.entry(MenuItem::new("Connect to WSL Host").action(
                    move || {
                        workbench_command
                            .send(LapceWorkbenchCommand::ConnectWslHost);
                    }
                ));
            }
            menu
        })
        .style(move |s| {
            let (connected, connecting, disconnected, bg, abg) = config.with(|config| {
                (
                    config.color(LapceColor::LAPCE_REMOTE_CONNECTED),
                    config.color(LapceColor::LAPCE_REMOTE_CONNECTING),
                    config.color(LapceColor::LAPCE_REMOTE_DISCONNECTED),
                    config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    config.color(LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND),
                )
            });
            let color = if is_local {
                Color::TRANSPARENT
            } else {
                match proxy_status.get() {
                    Some(ProxyStatus::Connected) => {connected
                    },
                    Some(ProxyStatus::Connecting) => {connecting
                    },
                    Some(ProxyStatus::Disconnected) => {disconnected
                    },
                    None => Color::TRANSPARENT
                }
            };
            s.height_pct(100.0)
                .padding_horiz(10.0)
                .items_center()
                .background(color)
                .hover(|s| {
                    s.cursor(CursorStyle::Pointer).background(bg
                    )
                })
                .active(|s| {
                    s.cursor(CursorStyle::Pointer).background(abg
                    )
                })
        }),
        drag_window_area(empty())
            .style(|s| s.height_pct(100.0).flex_basis(0.0).flex_grow(1.0))
    ))
    .style(move |s| {
        s.height_pct(100.0)
            .flex_basis(0.0)
            .flex_grow(1.0)
            .items_center()
    })
    .debug_name("Left Side of Top Bar")
}

fn middle(
    workspace: LapceWorkspace,
    main_split: MainSplitData,
    workbench_command: Listener<LapceWorkbenchCommand>,
    config: WithLapceConfig
) -> impl View {
    let local_workspace = workspace.clone();
    let can_jump_backward = {
        let main_split = main_split.clone();
        create_memo(move |_| main_split.can_jump_location_backward(true))
    };
    let can_jump_forward =
        create_memo(move |_| main_split.can_jump_location_forward(true));

    let jump_backward = move || {
        clickable_icon(
            || LapceIcons::LOCATION_BACKWARD,
            move || {
                workbench_command.send(LapceWorkbenchCommand::JumpLocationBackward);
            },
            || false,
            move || !can_jump_backward.get(),
            || "Jump Backward",
            config
        )
        .style(move |s| s.margin_horiz(6.0))
    };
    let jump_forward = move || {
        clickable_icon(
            || LapceIcons::LOCATION_FORWARD,
            move || {
                workbench_command.send(LapceWorkbenchCommand::JumpLocationForward);
            },
            || false,
            move || !can_jump_forward.get(),
            || "Jump Forward",
            config
        )
        .style(move |s| s.margin_right(6.0))
    };

    let open_folder = move || {
        not_clickable_icon(
            || LapceIcons::PALETTE_MENU,
            || false,
            || false,
            || "Open Folder / Recent Workspace",
            config
        )
        .popout_menu(move || {
            Menu::new("")
                .entry(MenuItem::new("Open Folder").action(move || {
                    workbench_command.send(LapceWorkbenchCommand::OpenFolder);
                }))
                .entry(MenuItem::new("Open Recent Workspace").action(move || {
                    workbench_command.send(LapceWorkbenchCommand::PaletteWorkspace);
                }))
        })
    };

    stack((
        stack((
            drag_window_area(empty())
                .style(|s| s.height_pct(100.0).flex_basis(0.0).flex_grow(1.0)),
            jump_backward(),
            jump_forward()
        ))
        .style(|s| {
            s.flex_basis(0)
                .flex_grow(1.0)
                .justify_content(Some(JustifyContent::FlexEnd))
        }),
        container(
            stack((
                svg(move || config.with_ui_svg(LapceIcons::SEARCH)).style(
                    move |s| {
                        let (caret_color, icon_size) = config.with(|config| {
                            (
                                config.color(LapceColor::LAPCE_ICON_ACTIVE), config.ui.icon_size() as f32
                            )
                        });
                        s.size(icon_size, icon_size)
                            .color(caret_color)
                    }
                ),
                label(move || {
                    if let Some(s) = local_workspace.display() {
                        s
                    } else {
                        "Open Folder".to_string()
                    }
                })
                .style(|s| s.padding_left(10).padding_right(5).selectable(false)),
                open_folder()
            ))
            .style(|s| s.align_items(Some(AlignItems::Center)))
        )
        .on_event_stop(EventListener::PointerDown, |_| {})
        .on_click_stop(move |_| {
            if workspace.clone().path.is_some() {
                workbench_command.send(LapceWorkbenchCommand::PaletteHelpAndFile);
            } else {
                workbench_command.send(LapceWorkbenchCommand::PaletteWorkspace);
            }
        })
        .style(move |s| {
            let (caret_color, bg) = config.with(|config| {
                (
                    config.color(LapceColor::LAPCE_BORDER), config.color(LapceColor::EDITOR_BACKGROUND)
                )
            });
            s.flex_basis(0)
                .flex_grow(10.0)
                .min_width(200.0)
                .max_width(500.0)
                .height(26.0)
                .justify_content(Some(JustifyContent::Center))
                .align_items(Some(AlignItems::Center))
                .border(1.0)
                .border_color(caret_color)
                .border_radius(6.0)
                .background(bg)
        }),
        stack((
            tooltip_label(
                config,
                clickable_icon_base_with_color(
                    || LapceIcons::START,
                    Some(move || {
                        workbench_command
                            .send(LapceWorkbenchCommand::PaletteRunAndDebug)
                    }),
                    || false,
                    || false,
                    config,
                    Some(palette::css::GREEN)
                ),
                || "Run and Debug"
            )
            .style(move |s| s.margin_horiz(6.0)).debug_name("Run and Debug"),
            drag_window_area(empty())
                .style(|s| s.height_pct(100.0).flex_basis(0.0).flex_grow(1.0))
        ))
        .style(move |s| {
            s.flex_basis(0)
                .flex_grow(1.0)
                .justify_content(Some(JustifyContent::FlexStart))
        })
    ))
    .style(|s| {
        s.flex_basis(0)
            .flex_grow(2.0)
            .align_items(Some(AlignItems::Center))
            .justify_content(Some(JustifyContent::Center))
    })
    .debug_name("Middle of Top Bar")
}

fn right(
    window_command: Listener<WindowCommand>,
    workbench_command: Listener<LapceWorkbenchCommand>,
    latest_release: ReadSignal<Option<ReleaseInfo>>,
    update_in_progress: RwSignal<bool>,
    // num_window_tabs: Memo<usize>,
    window_maximized: RwSignal<bool>,
    config: WithLapceConfig
) -> impl View {
    let latest_version = create_memo(move |_| {
        let latest_release = latest_release.get();
        let latest_version =
            latest_release.as_ref().as_ref().map(|r| r.version.clone());
        if latest_version.is_some()
            && latest_version.as_deref() != Some(meta::VERSION)
        {
            latest_version
        } else {
            None
        }
    });

    let has_update = move || latest_version.with(|v| v.is_some());

    stack((
        drag_window_area(empty())
            .style(|s| s.height_pct(100.0).flex_basis(0.0).flex_grow(1.0)),
        stack((
            not_clickable_icon(
                || LapceIcons::SETTINGS,
                || false,
                || false,
                || "Settings",
                config
            )
            .popout_menu(move || {
                Menu::new("")
                    .entry(MenuItem::new("Command Palette").action(move || {
                        workbench_command.send(LapceWorkbenchCommand::PaletteCommand)
                    }))
                    .separator()
                    .entry(MenuItem::new("Open Settings").action(move || {
                        workbench_command.send(LapceWorkbenchCommand::OpenSettings)
                    }))
                    .entry(MenuItem::new("Open Keyboard Shortcuts").action(
                        move || {
                            workbench_command
                                .send(LapceWorkbenchCommand::OpenKeyboardShortcuts)
                        }
                    ))
                    .entry(MenuItem::new("Open Theme Color Settings").action(
                        move || {
                            workbench_command
                                .send(LapceWorkbenchCommand::OpenThemeColorSettings)
                        }
                    ))
                    .separator()
                    .entry(if let Some(v) = latest_version.get_untracked() {
                        if update_in_progress.get_untracked() {
                            MenuItem::new(format!("Update in progress ({v})"))
                                .enabled(false)
                        } else {
                            MenuItem::new(format!("Restart to update ({v})")).action(
                                move || {
                                    workbench_command
                                        .send(LapceWorkbenchCommand::RestartToUpdate)
                                }
                            )
                        }
                    } else {
                        MenuItem::new("No update available").enabled(false)
                    })
                    .separator()
                    .entry(MenuItem::new("About Lapce").action(move || {
                        workbench_command.send(LapceWorkbenchCommand::ShowAbout)
                    }))
            }),
            container(label(|| "1".to_string()).style(move |s| {
                let (caret_color, bg) = config.with(|config| {
                    (
                        config.color(LapceColor::EDITOR_CARET), config.color(LapceColor::EDITOR_BACKGROUND)
                    )
                });
                s.font_size(10.0)
                    .color(bg)
                    .border_radius(100.0)
                    .margin_left(5.0)
                    .margin_top(10.0)
                    .background(caret_color)
            }))
            .style(move |s| {
                let has_update = has_update();
                s.absolute()
                    .size_pct(100.0, 100.0)
                    .justify_end()
                    .items_end()
                    .apply_if(!has_update, |s| s.hide())
            })
        ))
        .style(move |s| s.margin_horiz(6.0)),
        window_controls_view(window_command, window_maximized, config)
    ))
    .style(|s| {
        s.flex_basis(0)
            .flex_grow(1.0)
            .justify_content(Some(JustifyContent::FlexEnd))
    })
    .debug_name("Right of top bar")
}

pub fn title(window_tab_data: WindowWorkspaceData) -> impl View {
    let workspace = window_tab_data.workspace.clone();
    let lapce_command = window_tab_data.common.lapce_command;
    let workbench_command = window_tab_data.common.workbench_command;
    let window_command = window_tab_data.common.window_common.window_command;
    let latest_release = window_tab_data.common.window_common.latest_release;
    let proxy_status = window_tab_data.common.proxy_status;
    // let num_window_tabs = window_tab_data.common.window_common.num_window_tabs;
    let window_maximized = window_tab_data.common.window_common.window_maximized;
    let title_height = window_tab_data.title_height;
    let update_in_progress = window_tab_data.update_in_progress;
    let config = window_tab_data.common.config;
    stack((
        left(
            workspace.clone(),
            lapce_command,
            workbench_command,
            config,
            proxy_status
        ),
        middle(
            workspace,
            window_tab_data.main_split.clone(),
            workbench_command,
            config
        ),
        right(
            window_command,
            workbench_command,
            latest_release,
            update_in_progress,
            window_maximized,
            config
        )
    ))
    .on_resize(move |rect| {
        let height = rect.height();
        if height != title_height.get_untracked() {
            title_height.set(height);
        }
    })
    .style(move |s| {
        let (caret_color, bg) = config.with(|config| {
            (
                config.color(LapceColor::LAPCE_BORDER), config.color(LapceColor::PANEL_BACKGROUND)
            )
        });
        s.width_pct(100.0)
            .height(37.0)
            .items_center()
            .background(bg)
            .border_bottom(1.0)
            .border_color(caret_color)
    })
    .debug_name("Title / Top Bar")
}

pub fn window_controls_view(
    window_command: Listener<WindowCommand>,
    // is_title: bool,
    // num_window_tabs: Memo<usize>,
    window_maximized: RwSignal<bool>,
    config: WithLapceConfig
) -> impl View {
    stack((
        clickable_icon(
            || LapceIcons::WINDOW_MINIMIZE,
            || {
                floem::action::minimize_window();
            },
            || false,
            || false,
            || "Minimize",
            config
        )
        .style(|s| s.margin_right(16.0).margin_left(10.0)),
        clickable_icon(
            move || {
                if window_maximized.get() {
                    LapceIcons::WINDOW_RESTORE
                } else {
                    LapceIcons::WINDOW_MAXIMIZE
                }
            },
            move || {
                floem::action::set_window_maximized(
                    !window_maximized.get_untracked()
                );
            },
            || false,
            || false,
            || "Maximize",
            config
        )
        .style(|s| s.margin_right(16.0)),
        clickable_icon(
            || LapceIcons::WINDOW_CLOSE,
            move || {
                window_command.send(WindowCommand::CloseWindow);
            },
            || false,
            || false,
            || "Close Window",
            config
        )
        .style(|s| s.margin_right(6.0))
    ))
    .style(move |s| {
        s.apply_if(
            cfg!(target_os = "macos")
                || !config.with_untracked(|config| config.core.custom_titlebar),
            |s| s.hide()
        )
    })
}

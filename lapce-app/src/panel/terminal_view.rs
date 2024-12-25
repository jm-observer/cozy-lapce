use std::rc::Rc;

use floem::{
    action::show_context_menu,
    event::{Event, EventListener, EventPropagation},
    kurbo::Size,
    menu::{Menu, MenuItem},
    reactive::{create_rw_signal, SignalGet, SignalUpdate, SignalWith},
    views::{
        container, dyn_stack, empty, label,
        scroll::{scroll, Thickness, VerticalScrollAsHorizontal},
        stack, tab, Decorators,
    },
    View, ViewId,
};

use super::kind::PanelKind;
use crate::id::TerminalTabId;
use crate::svg;
use crate::{
    app::clickable_icon,
    command::{InternalCommand, LapceWorkbenchCommand},
    config::{color::LapceColor, icon::LapceIcons},
    debug::RunDebugMode,
    listener::Listener,
    terminal::{
        panel::TerminalPanelData, tab::TerminalTabData, view::terminal_view,
    },
    window_tab::{Focus, WindowTabData},
};
pub fn terminal_panel(window_tab_data: Rc<WindowTabData>) -> impl View {
    let focus = window_tab_data.common.focus;
    stack((
        terminal_tab_header(window_tab_data.clone()),
        terminal_tab_content(window_tab_data.clone()),
    ))
    .on_event_cont(EventListener::PointerDown, move |_| {
        if focus.get_untracked() != Focus::Panel(PanelKind::Terminal) {
            focus.set(Focus::Panel(PanelKind::Terminal));
        }
    })
    .style(|s| s.absolute().size_pct(100.0, 100.0).flex_col())
    .debug_name("Terminal Panel")
}

fn terminal_tab_header(window_tab_data: Rc<WindowTabData>) -> impl View {
    let terminal = window_tab_data.terminal.clone();
    let config = window_tab_data.common.config;
    let focus = window_tab_data.common.focus;
    let active_index = move || terminal.tab_infos.with(|info| info.active);
    let tab_info = terminal.tab_infos;
    let header_width = create_rw_signal(0.0);
    let header_height = create_rw_signal(0.0);
    let icon_width = create_rw_signal(0.0);
    let scroll_size = create_rw_signal(Size::ZERO);
    let workbench_command = window_tab_data.common.workbench_command;

    stack((
        scroll(dyn_stack(
            move || terminal.tab_infos.with(|info| info.tabs.clone()),
            |tab| tab.terminal_tab_id,
            move |tab| {
                let terminal = terminal.clone();
                let local_terminal = terminal.clone();
                let terminal_tab_id = tab.terminal_tab_id;

                let title = {
                    let tab = tab.clone();
                    move || {
                        let terminal = tab.active_terminal(true);
                        let run_debug = terminal.run_debug;
                        if let Some(name) = run_debug.with(|run_debug| {
                            run_debug.as_ref().map(|r| r.config.name.clone())
                        }) {
                            return name;
                        }
                        terminal.title.get()
                    }
                };

                let svg_string = move || {
                    let terminal = tab.active_terminal(true);
                    let run_debug = terminal.run_debug;
                    if let Some((mode, stopped)) = run_debug.with(|run_debug| {
                        run_debug.as_ref().map(|r| (r.mode, r.stopped))
                    }) {
                        let svg = match (mode, stopped) {
                            (RunDebugMode::Run, false) => LapceIcons::START,
                            (RunDebugMode::Run, true) => LapceIcons::RUN_ERRORS,
                            (RunDebugMode::Debug, false) => LapceIcons::DEBUG,
                            (RunDebugMode::Debug, true) => {
                                LapceIcons::DEBUG_DISCONNECT
                            }
                        };
                        return svg;
                    }
                    LapceIcons::TERMINAL
                };
                stack((
                    container({
                        stack((
                            container(
                                svg(move || config.get().ui_svg(svg_string()))
                                    .style(move |s| {
                                        let config = config.get();
                                        let size = config.ui.icon_size() as f32;
                                        s.size(size, size).color(
                                            config.color(
                                                LapceColor::LAPCE_ICON_ACTIVE,
                                            ),
                                        )
                                    }),
                            )
                            .style(|s| s.padding_horiz(10.0).padding_vert(12.0)),
                            label(title).style(|s| {
                                s.min_width(0.0)
                                    .flex_basis(0.0)
                                    .flex_grow(1.0)
                                    .text_ellipsis()
                                    .selectable(false)
                            }),
                            clickable_icon(
                                || LapceIcons::CLOSE,
                                move || {
                                    terminal.close_tab(Some(terminal_tab_id));
                                },
                                || false,
                                || false,
                                || "Close",
                                config,
                            )
                            .style(|s| s.margin_horiz(6.0)),
                            empty().style(move |s| {
                                s.absolute()
                                    .width_full()
                                    .height(header_height.get() - 15.0)
                                    .border_right(1.0)
                                    .border_color(
                                        config.get().color(LapceColor::LAPCE_BORDER),
                                    )
                            }),
                        ))
                        .style(move |s| {
                            s.items_center().width(200.0).border_color(
                                config.get().color(LapceColor::LAPCE_BORDER),
                            )
                        })
                    })
                    .style(|s| s.items_center()),
                    container({
                        label(|| "".to_string()).style(move |s| {
                            s.size_pct(100.0, 100.0)
                                .border_bottom(
                                    if active_index() == Some(terminal_tab_id) {
                                        2.0
                                    } else {
                                        0.0
                                    },
                                )
                                .border_color(config.get().color(
                                    if focus.get()
                                        == Focus::Panel(PanelKind::Terminal)
                                    {
                                        LapceColor::LAPCE_TAB_ACTIVE_UNDERLINE
                                    } else {
                                        LapceColor::LAPCE_TAB_INACTIVE_UNDERLINE
                                    },
                                ))
                        })
                    })
                    .style(|s| {
                        s.absolute().padding_horiz(3.0).size_pct(100.0, 100.0)
                    }),
                ))
                .on_event_cont(
                    EventListener::PointerDown,
                    move |_| {
                        if tab_info.with_untracked(|tab| tab.active)
                            != Some(terminal_tab_id)
                        {
                            tab_info.update(|tab| {
                                tab.active = Some(terminal_tab_id);
                            });
                            local_terminal.update_debug_active_term();
                        }
                    },
                )
            },
        ))
        .on_resize(move |rect| {
            if rect.size() != scroll_size.get_untracked() {
                scroll_size.set(rect.size());
            }
        })
        .style(move |s| {
            let header_width = header_width.get();
            let icon_width = icon_width.get();
            s.set(VerticalScrollAsHorizontal, true)
                .absolute()
                .max_width(header_width - icon_width)
                .set(Thickness, 3)
        }),
        empty().style(move |s| {
            let size = scroll_size.get();
            s.size(size.width, size.height)
        }),
        container(clickable_icon(
            || LapceIcons::ADD,
            move || {
                workbench_command.send(LapceWorkbenchCommand::NewTerminalTab);
            },
            || false,
            || false,
            || "New Terminal",
            config,
        ))
        .on_resize(move |rect| {
            let width = rect.size().width;
            if icon_width.get_untracked() != width {
                icon_width.set(width);
            }
        })
        .style(|s| s.padding_horiz(10)),
    ))
    .on_resize(move |rect| {
        let size = rect.size();
        if header_width.get_untracked() != size.width {
            header_width.set(size.width);
        }
        if header_height.get_untracked() != size.height {
            header_height.set(size.height);
        }
    })
    .on_double_click(move |_| {
        window_tab_data
            .panel
            .toggle_maximize(&crate::panel::kind::PanelKind::Terminal);
        EventPropagation::Stop
    })
    .style(move |s| {
        let config = config.get();
        s.width_pct(100.0)
            .items_center()
            .border_bottom(1.0)
            .border_color(config.color(LapceColor::LAPCE_BORDER))
            .background(config.color(LapceColor::PANEL_BACKGROUND))
    })
}

fn terminal_tab_split(
    terminal_panel_data: TerminalPanelData,
    terminal_tab_data: TerminalTabData,
) -> impl View {
    let internal_command = terminal_panel_data.common.internal_command;
    let workspace = terminal_panel_data.workspace.clone();
    let terminal_panel_data = terminal_panel_data.clone();
    container({
        let terminal = terminal_tab_data.terminal.get();
        let terminal_id = terminal.term_id;
        let terminal_view = terminal_view(
            terminal.term_id,
            terminal.raw.read_only(),
            terminal.mode.read_only(),
            terminal.run_debug.read_only(),
            terminal_panel_data,
            terminal.launch_error,
            internal_command,
            workspace.clone(),
        );
        let view_id = terminal_view.id();
        let have_task = terminal.run_debug.get_untracked().is_some();
        terminal_view
            .on_secondary_click_stop(move |_| {
                if have_task {
                    tab_secondary_click(internal_command, view_id, terminal_id);
                }
            })
            .on_event(EventListener::PointerWheel, move |event| {
                if let Event::PointerWheel(pointer_event) = event {
                    terminal.clone().wheel_scroll(pointer_event.delta.y);
                    EventPropagation::Stop
                } else {
                    EventPropagation::Continue
                }
            })
            .style(|s| s.size_pct(100.0, 100.0))
    })
    .style(move |s| {
        s.size_pct(100.0, 100.0).padding_horiz(10.0)
        // .apply_if(index.get() > 0, |s| {
        //     s.border_left(1.0)
        //         .border_color(config.get().color(LapceColor::LAPCE_BORDER))
        // })
    })
}

fn terminal_tab_content(window_tab_data: Rc<WindowTabData>) -> impl View {
    let terminal = window_tab_data.terminal.clone();
    tab(
        move || {
            terminal.tab_infos.with(|info| {
                info.active_tab().map(|x| x.0).unwrap_or_default()
                // info.active.map(|x| x.to_raw() as usize).unwrap_or_default()
            })
        },
        move || terminal.tab_infos.with(|info| info.tabs.clone()),
        |tab| tab.terminal_tab_id,
        move |tab| terminal_tab_split(terminal.clone(), tab),
    )
    .style(|s| s.size_pct(100.0, 100.0))
    .debug_name("terminal_tab_content")
}

fn tab_secondary_click(
    internal_command: Listener<InternalCommand>,
    view_id: ViewId,
    terminal_id: TerminalTabId,
) {
    let mut menu = Menu::new("");
    menu = menu
        .entry(MenuItem::new("Stop").action(move || {
            internal_command.send(InternalCommand::StopTerminal { terminal_id });
        }))
        .entry(MenuItem::new("Restart").action(move || {
            internal_command.send(InternalCommand::RestartTerminal { terminal_id });
        }))
        .entry(MenuItem::new("Clear All").action(move || {
            internal_command.send(InternalCommand::ClearTerminalBuffer {
                view_id,
                terminal_id,
            });
        }));
    show_context_menu(menu, None);
}

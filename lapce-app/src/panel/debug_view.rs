use floem::{
    View,
    event::EventListener,
    peniko::Color,
    prelude::palette,
    reactive::{RwSignal, SignalGet, SignalUpdate, SignalWith, create_rw_signal},
    style::CursorStyle,
    text::Style as FontStyle,
    views::{
        Decorators, container, dyn_stack, label, scroll, stack, svg, text,
        virtual_stack,
    },
};
use lapce_core::{
    debug::{BreakpointAction, RunDebugMode},
    icon::LapceIcons,
    panel::{PanelContainerPosition, PanelSection},
};
use lapce_rpc::{
    dap_types::{DapId, ThreadId},
    terminal::TermId,
};

use super::view::PanelBuilder;
use crate::{
    app::clickable_icon,
    command::InternalCommand,
    config::{WithLapceConfig, color::LapceColor},
    debug::{DapVariable, StackTraceData, update_breakpoints},
    editor::location::{EditorLocation, EditorPosition},
    listener::Listener,
    settings::checkbox,
    terminal::panel::TerminalPanelData,
    window_workspace::WindowWorkspaceData,
};

pub fn debug_panel(
    window_tab_data: WindowWorkspaceData,
    position: PanelContainerPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let terminal = window_tab_data.terminal.clone();
    let internal_command = window_tab_data.common.internal_command;

    PanelBuilder::new(config, position)
        .add_height(
            "Processes",
            150.0,
            debug_processes(terminal.clone(), config),
            window_tab_data.panel.section_open(PanelSection::Process),
        )
        .add(
            "Variables",
            variables_view(window_tab_data.clone()),
            window_tab_data.panel.section_open(PanelSection::Variable),
        )
        .add(
            "Stack Frames",
            debug_stack_traces(terminal.clone(), internal_command, config),
            window_tab_data.panel.section_open(PanelSection::StackFrame),
        )
        .add_height(
            "Breakpoints",
            150.0,
            breakpoints_view(window_tab_data.clone()),
            window_tab_data.panel.section_open(PanelSection::Breakpoint),
        )
        .build()
        .debug_name("Debug Panel")
}

fn debug_process_icons(
    terminal: TerminalPanelData,
    term_id: TermId,
    dap_id: DapId,
    mode: RunDebugMode,
    stopped: bool,
    config: WithLapceConfig,
) -> impl View {
    let paused = move || {
        let stopped = terminal
            .debug
            .daps
            .with_untracked(|daps| daps.get(&dap_id).map(|dap| dap.stopped));
        stopped.map(|stopped| stopped.get()).unwrap_or(false)
    };
    match mode {
        RunDebugMode::Run => container(stack((
            {
                let internal_command = terminal.common.internal_command;
                clickable_icon(
                    || LapceIcons::DEBUG_RESTART,
                    move || {
                        internal_command.send(InternalCommand::RestartTerminal {
                            terminal_id: term_id,
                        });
                    },
                    || false,
                    || false,
                    || "Restart",
                    config,
                )
                .style(|s| s.margin_horiz(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_STOP,
                    move || {
                        terminal.manual_stop_run_debug(term_id);
                    },
                    || false,
                    move || stopped,
                    || "Stop",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::CLOSE,
                    move || {
                        terminal.close_terminal(&term_id);
                    },
                    || false,
                    || false,
                    || "Close",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
        ))),
        RunDebugMode::Debug => container(stack((
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_CONTINUE,
                    move || {
                        terminal.dap_continue(term_id);
                    },
                    || false,
                    move || !paused() || stopped,
                    || "Continue",
                    config,
                )
                .style(|s| s.margin_horiz(6.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_PAUSE,
                    move || {
                        terminal.dap_pause(term_id);
                    },
                    || false,
                    move || paused() || stopped,
                    || "Pause",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_STEP_OVER,
                    move || {
                        terminal.dap_step_over(term_id);
                    },
                    || false,
                    move || !paused() || stopped,
                    || "Step Over",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_STEP_INTO,
                    move || {
                        terminal.dap_step_into(term_id);
                    },
                    || false,
                    move || !paused() || stopped,
                    || "Step Into",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_STEP_OUT,
                    move || {
                        terminal.dap_step_out(term_id);
                    },
                    || false,
                    move || !paused() || stopped,
                    || "Step Out",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();

                let internal_command = terminal.common.internal_command;
                clickable_icon(
                    || LapceIcons::DEBUG_RESTART,
                    move || {
                        internal_command.send(InternalCommand::RestartTerminal {
                            terminal_id: term_id,
                        });
                    },
                    || false,
                    || false,
                    || "Restart",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::DEBUG_STOP,
                    move || {
                        terminal.manual_stop_run_debug(term_id);
                    },
                    || false,
                    move || stopped,
                    || "Stop",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
            {
                let terminal = terminal.clone();
                clickable_icon(
                    || LapceIcons::CLOSE,
                    move || {
                        terminal.close_terminal(&term_id);
                    },
                    || false,
                    || false,
                    || "Close",
                    config,
                )
                .style(|s| s.margin_right(4.0))
            },
        ))),
    }
}

fn debug_processes(
    terminal: TerminalPanelData,
    config: WithLapceConfig,
) -> impl View {
    scroll({
        let terminal = terminal.clone();
        let local_terminal = terminal.clone();
        dyn_stack(
            move || local_terminal.run_debug_process_tracked(),
            |(term_id, p)| (*term_id, p.stopped),
            move |(term_id, p)| {
                let terminal = terminal.clone();
                let is_active =
                    move || terminal.debug.active_term.get() == Some(term_id);
                let local_terminal = terminal.clone();
                let is_hovered = create_rw_signal(false);
                stack((
                    {
                        let svg_str = match (&p.mode, p.stopped) {
                            (RunDebugMode::Run, false) => LapceIcons::START,
                            (RunDebugMode::Run, true) => LapceIcons::RUN_ERRORS,
                            (RunDebugMode::Debug, false) => LapceIcons::DEBUG,
                            (RunDebugMode::Debug, true) => {
                                LapceIcons::DEBUG_DISCONNECT
                            },
                        };
                        svg(move || config.with_ui_svg(svg_str)).style(move |s| {
                            let size = config.with_icon_size() as f32;
                            s.size(size, size)
                                .margin_vert(5.0)
                                .margin_horiz(10.0)
                                .color(palette::css::GREEN)
                        })
                    },
                    label(move || p.config.name.clone()).style(|s| {
                        s.flex_grow(1.0)
                            .flex_basis(0.0)
                            .min_width(0.0)
                            .text_ellipsis()
                    }),
                    debug_process_icons(
                        terminal.clone(),
                        term_id,
                        p.config.dap_id,
                        p.mode,
                        p.stopped,
                        config,
                    )
                    .style(move |s| {
                        s.apply_if(!is_hovered.get() && !is_active(), |s| s.hide())
                    }),
                ))
                .on_click_stop(move |_| {
                    local_terminal.debug.active_term.set(Some(term_id));
                    local_terminal.focus_terminal(term_id);
                })
                .on_event_stop(EventListener::PointerEnter, move |_| {
                    is_hovered.set(true);
                })
                .on_event_stop(EventListener::PointerLeave, move |_| {
                    is_hovered.set(false);
                })
                .style(move |s| {
                    let (cbg, hbg) = config.signal(|config| {
                        (
                            config.color(LapceColor::PANEL_CURRENT_BACKGROUND),
                            config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                        )
                    });
                    s.padding_vert(6.0)
                        .width_pct(100.0)
                        .items_center()
                        .apply_if(is_active(), |s| s.background(cbg.get()))
                        .hover(|s| {
                            s.cursor(CursorStyle::Pointer)
                                .background(hbg.get().multiply_alpha(0.3))
                        })
                })
            },
        )
        .style(|s| s.width_pct(100.0).flex_col())
    })
    .debug_name("debug processes")
}

fn variables_view(window_tab_data: WindowWorkspaceData) -> impl View {
    let terminal = window_tab_data.terminal.clone();
    let local_terminal = window_tab_data.terminal.clone();
    let config = window_tab_data.common.config;
    container(
        scroll(
            virtual_stack(
                // VirtualDirection::Vertical,
                // VirtualItemSize::Fixed(Box::new(move || ui_line_height.get())),
                move || {
                    let dap = terminal.get_active_dap(true);
                    dap.map(|dap| {
                        if !dap.stopped.get() {
                            return DapVariable::default();
                        }
                        let process_stopped = dap
                            .term_id
                            .and_then(|x| {
                                terminal.get_terminal(x).and_then(|t| {
                                    t.data.with(|r| {
                                        r.run_debug.as_ref().map(|r| r.stopped)
                                    })
                                })
                            })
                            .unwrap_or(true);
                        if process_stopped {
                            return DapVariable::default();
                        }
                        dap.variables.get()
                    })
                    .unwrap_or_default()
                },
                |node| {
                    (
                        node.item.name().to_string(),
                        node.item.value().map(|v| v.to_string()),
                        node.item.reference(),
                        node.expanded,
                        node.level,
                    )
                },
                move |node| {
                    let local_terminal = local_terminal.clone();
                    let level = node.level;
                    let reference = node.item.reference();
                    let name = node.item.name();
                    let ty = node.item.ty();
                    let type_exists = ty.map(|ty| !ty.is_empty()).unwrap_or(false);
                    stack((
                        svg(move || {
                            let svg_str = match node.expanded {
                                true => LapceIcons::ITEM_OPENED,
                                false => LapceIcons::ITEM_CLOSED,
                            };
                            config.with_ui_svg(svg_str)
                        })
                        .style(move |s| {
                            let (color, size) = config.signal(|config| {
                                (
                                    config.color(LapceColor::LAPCE_ICON_ACTIVE),
                                    config.ui.icon_size.signal(),
                                )
                            });
                            let color = if reference > 0 {
                                color.get()
                            } else {
                                Color::TRANSPARENT
                            };
                            let size = size.get() as f32;
                            s.size(size, size).margin_left(10.0).color(color)
                        }),
                        text(name),
                        text(": ").style(move |s| {
                            s.apply_if(!type_exists || reference == 0, |s| s.hide())
                        }),
                        text(node.item.ty().unwrap_or("")).style(move |s| {
                            s.color(config.with_style_color("type").unwrap())
                                .apply_if(!type_exists || reference == 0, |s| {
                                    s.hide()
                                })
                        }),
                        text(format!(" = {}", node.item.value().unwrap_or("")))
                            .style(move |s| s.apply_if(reference > 0, |s| s.hide())),
                    ))
                    .on_click_stop(move |_| {
                        if reference > 0 {
                            let dap = local_terminal.get_active_dap(false);
                            if let Some(dap) = dap {
                                let process_stopped = dap
                                    .term_id
                                    .and_then(|x| {
                                        local_terminal.get_terminal(x).and_then(
                                            |t| {
                                                t.data.with(|r| {
                                                    r.run_debug
                                                        .as_ref()
                                                        .map(|r| r.stopped)
                                                })
                                            },
                                        )
                                    })
                                    .unwrap_or(true);
                                if !process_stopped {
                                    dap.toggle_expand(
                                        node.parent.clone(),
                                        reference,
                                    );
                                }
                            }
                        }
                    })
                    .style(move |s| {
                        s.items_center()
                            .padding_right(10.0)
                            .padding_left((level * 10) as f32)
                            .min_width_pct(100.0)
                            .hover(|s| {
                                s.apply_if(reference > 0, |s| {
                                    s.background(config.with_color(
                                        LapceColor::PANEL_HOVERED_BACKGROUND,
                                    ))
                                })
                            })
                    })
                },
            )
            .style(|s| s.flex_col().min_width_full()),
        )
        .style(|s| s.absolute().size_full()),
    )
    .style(|s| s.width_full().line_height(1.6).flex_grow(1.0).flex_basis(0))
    .debug_name("debug variables")
}

fn debug_stack_frames(
    dap_id: DapId,
    thread_id: ThreadId,
    stack_trace: StackTraceData,
    stopped: RwSignal<bool>,
    internal_command: Listener<InternalCommand>,
    config: WithLapceConfig,
) -> impl View {
    let expanded = stack_trace.expanded;
    stack((
        container(label(move || thread_id.to_string()))
            .on_click_stop(move |_| {
                expanded.update(|expanded| {
                    *expanded = !*expanded;
                });
            })
            .style(move |s| {
                s.padding_horiz(10.0).min_width_pct(100.0).hover(move |s| {
                    s.cursor(CursorStyle::Pointer).background(
                        config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    )
                })
            }),
        dyn_stack(
            move || {
                let expanded = stack_trace.expanded.get() && stopped.get();
                if expanded {
                    stack_trace.frames.get()
                } else {
                    im::Vector::new()
                }
            },
            |frame| frame.id,
            move |frame| {
                let full_path = frame.source.as_ref().and_then(|s| s.path.clone());
                let line = frame.line.saturating_sub(1);
                let col = frame.column.saturating_sub(1);

                let source_path = frame
                    .source
                    .as_ref()
                    .and_then(|s| s.path.as_ref())
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let has_source = !source_path.is_empty();
                let source_path = format!("{source_path}:{}", frame.line);

                container(stack((
                    label(move || frame.name.clone()).style(move |s| {
                        s.hover(|s| {
                            s.background(
                                config.with_color(
                                    LapceColor::PANEL_HOVERED_BACKGROUND,
                                ),
                            )
                        })
                    }),
                    label(move || source_path.clone()).style(move |s| {
                        s.margin_left(10.0)
                            .color(config.with_color(LapceColor::EDITOR_DIM))
                            .font_style(FontStyle::Italic)
                            .apply_if(!has_source, |s| s.hide())
                    }),
                )))
                .on_click_stop(move |_| {
                    if let Some(path) = full_path.clone() {
                        internal_command.send(InternalCommand::JumpToLocation {
                            location: EditorLocation {
                                path,
                                position: Some(EditorPosition::Position(
                                    lsp_types::Position {
                                        line:      line as u32,
                                        character: col as u32,
                                    },
                                )),
                                scroll_offset: None,
                                ignore_unconfirmed: false,
                                same_editor_tab: false,
                            },
                        });
                    }
                    internal_command.send(InternalCommand::DapFrameScopes {
                        dap_id,
                        frame_id: frame.id,
                    });
                })
                .style(move |s| {
                    let (ed, hbg) = config.signal(|config| {
                        (
                            config.color(LapceColor::EDITOR_DIM),
                            config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                        )
                    });
                    s.padding_left(20.0)
                        .padding_right(10.0)
                        .min_width_pct(100.0)
                        .apply_if(!has_source, |s| s.color(ed.get()))
                        .hover(|s| {
                            s.background(hbg.get()).apply_if(has_source, |s| {
                                s.cursor(CursorStyle::Pointer)
                            })
                        })
                })
            },
        )
        .style(|s| s.flex_col().min_width_pct(100.0)),
    ))
    .style(|s| s.flex_col().min_width_pct(100.0))
    .debug_name("debug stack frames")
}

fn debug_stack_traces(
    terminal: TerminalPanelData,
    internal_command: Listener<InternalCommand>,
    config: WithLapceConfig,
) -> impl View {
    container(
        scroll({
            let local_terminal = terminal.clone();
            dyn_stack(
                move || {
                    let dap = local_terminal.get_active_dap(true);
                    if let Some(dap) = dap {
                        let process_stopped = dap
                            .term_id
                            .and_then(|x| {
                                local_terminal.get_terminal(x).and_then(|t| {
                                    t.data.with(|r| {
                                        r.run_debug.as_ref().map(|r| r.stopped)
                                    })
                                })
                            })
                            .unwrap_or(true);
                        if process_stopped {
                            return Vec::new();
                        }
                        let main_thread = dap.thread_id.get();
                        let stack_traces = dap.stack_traces.get();
                        let mut traces = stack_traces
                            .into_iter()
                            .map(|(thread_id, stack_trace)| {
                                (dap.dap_id, dap.stopped, thread_id, stack_trace)
                            })
                            .collect::<Vec<_>>();
                        traces.sort_by_key(|(_, _, id, _)| main_thread != Some(*id));
                        traces
                    } else {
                        Vec::new()
                    }
                },
                |(dap_id, stopped, thread_id, _)| {
                    (*dap_id, *thread_id, stopped.get_untracked())
                },
                move |(dap_id, stopped, thread_id, stack_trace)| {
                    debug_stack_frames(
                        dap_id,
                        thread_id,
                        stack_trace,
                        stopped,
                        internal_command,
                        config,
                    )
                },
            )
            .style(|s| s.flex_col().min_width_pct(100.0))
        })
        .style(|s| s.absolute().size_pct(100.0, 100.0)),
    )
    .style(|s| {
        s.width_pct(100.0)
            .line_height(1.6)
            .flex_grow(1.0)
            .flex_basis(0.0)
    })
    .debug_name("debug stack traces")
}

fn breakpoints_view(window_tab_data: WindowWorkspaceData) -> impl View {
    let breakpoints = window_tab_data.common.breakpoints;
    let proxy = window_tab_data.common.proxy.clone();
    let daps = window_tab_data.terminal.debug.daps;
    let config = window_tab_data.common.config;
    let workspace = window_tab_data.common.workspace.clone();
    let available_width = create_rw_signal(0.0);
    let internal_command = window_tab_data.common.internal_command;
    container(
        scroll(
            dyn_stack(
                move || breakpoints.view_data(),
                move |(path, breakpoint)| {
                    (path.clone(), breakpoint.line, breakpoint.active)
                },
                move |(path, breakpoint)| {
                    let line = breakpoint.line;
                    let full_path = path.clone();
                    let full_path_for_jump = path.clone();
                    let full_path_for_close = path.clone();
                    let path = if let Some(workspace_path) = workspace.path() {
                        path.strip_prefix(workspace_path)
                            .unwrap_or(&full_path)
                            .to_path_buf()
                    } else {
                        path
                    };

                    let file_name =
                        path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let folder =
                        path.parent().and_then(|s| s.to_str()).unwrap_or("");
                    let folder_empty = folder.is_empty();
                    let proxy = proxy.clone();
                    let toggle_proxy = proxy.clone();
                    stack((
                        clickable_icon(
                            move || LapceIcons::CLOSE,
                            move || {
                                update_breakpoints(
                                    daps,
                                    proxy.proxy_rpc.clone(),
                                    breakpoints,
                                    BreakpointAction::Remove {
                                        path: &full_path_for_close,
                                        line,
                                    },
                                );
                                // breakpoints.update(|breakpoints| {
                                //     if let Some(breakpoints) =
                                //         breakpoints.get_mut(&
                                // full_path_for_close)
                                //     {
                                //         breakpoints.remove(&line);
                                //     }
                                // });
                            },
                            || false,
                            || false,
                            || "Remove",
                            config,
                        )
                        .on_event_stop(EventListener::PointerDown, |_| {}),
                        checkbox(move || breakpoint.active, config)
                            .style(|s| {
                                s.margin_right(6.0).cursor(CursorStyle::Pointer)
                            })
                            .on_click_stop(move |_| {
                                update_breakpoints(
                                    daps,
                                    toggle_proxy.proxy_rpc.clone(),
                                    breakpoints,
                                    BreakpointAction::Toggle {
                                        path: &full_path,
                                        line,
                                    },
                                );
                                //
                                // breakpoints.update(|breakpoints| {
                                //     if let Some(breakpoints) =
                                //         breakpoints.get_mut(&full_path)
                                //     {
                                //         if let Some(breakpoint) =
                                //             breakpoints.get_mut(&line)
                                //         {
                                //             breakpoint.active =
                                // !breakpoint.active;
                                //         }
                                //     }
                                // });
                            }),
                        text(format!("{file_name}:{}", breakpoint.line + 1)).style(
                            move |s| {
                                let size = config.with_icon_size() as f32;
                                s.text_ellipsis().max_width(
                                    available_width.get() as f32
                                        - 20.0
                                        - size
                                        - 6.0
                                        - size
                                        - 8.0,
                                )
                            },
                        ),
                        text(folder).style(move |s| {
                            s.text_ellipsis()
                                .flex_grow(1.0)
                                .flex_basis(0.0)
                                .color(config.with_color(LapceColor::EDITOR_DIM))
                                .min_width(0.0)
                                .margin_left(6.0)
                                .apply_if(folder_empty, |s| s.hide())
                        }),
                    ))
                    .style(move |s| {
                        s.items_center().padding_horiz(10.0).width_pct(100.0).hover(
                            |s| {
                                s.background(config.with_color(
                                    LapceColor::PANEL_HOVERED_BACKGROUND,
                                ))
                            },
                        )
                    })
                    .on_click_stop(move |_| {
                        internal_command.send(InternalCommand::JumpToLocation {
                            location: EditorLocation {
                                path:               full_path_for_jump.clone(),
                                position:           Some(EditorPosition::Line(line)),
                                scroll_offset:      None,
                                ignore_unconfirmed: false,
                                same_editor_tab:    false,
                            },
                        });
                    })
                },
            )
            .style(|s| s.flex_col().line_height(1.6).width_pct(100.0)),
        )
        .on_resize(move |rect| {
            let width = rect.width();
            if available_width.get_untracked() != width {
                available_width.set(width);
            }
        })
        .style(|s| s.absolute().size_pct(100.0, 100.0)),
    )
    .style(|s| s.size_pct(100.0, 100.0))
    .debug_name("debug breakpoints")
}

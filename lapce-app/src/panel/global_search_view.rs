use std::path::PathBuf;

use floem::{
    View,
    event::{Event, EventListener},
    prelude::{RwSignal, SignalWith},
    reactive::{SignalGet, SignalUpdate},
    style::{CursorStyle, Style},
    views::{
        Decorators, container, label, scroll, stack, text_input, virtual_stack,
    },
};
use lapce_core::{
    icon::LapceIcons,
    panel::{PanelContainerPosition, PanelKind},
};
use lapce_rpc::proxy::SearchMatch;
use lapce_xi_rope::find::CaseMatching;

use crate::{
    app::clickable_icon,
    command::InternalCommand,
    config::{WithLapceConfig, color::LapceColor},
    editor::location::{EditorLocation, EditorPosition},
    focus_text::focus_text,
    global_search::{GlobalSearchData, SearchItem},
    listener::Listener,
    svg,
    window_workspace::{Focus, WindowWorkspaceData},
};

pub fn global_search_panel(
    window_tab_data: WindowWorkspaceData,
    _position: PanelContainerPosition,
) -> impl View {
    let global_search = window_tab_data.global_search.clone();
    let config = global_search.common.config;
    let internal_command = global_search.common.internal_command;
    let case_matching = global_search.common.find.case_matching;
    let whole_word = global_search.common.find.whole_words;
    let is_regex = global_search.common.find.is_regex;

    let focus = global_search.common.focus;
    // let is_focused = move || focus.get() == Focus::Panel(PanelKind::Search);

    stack((
        container(
            stack((
                text_input(global_search.search_str)
                    .style(|s| s.width_pct(100.0))
                    .on_event_stop(EventListener::KeyDown, move |event| {
                        if let Event::KeyDown(_key_event) = event {
                            window_tab_data.key_down(_key_event);
                        }
                    })
                    .on_event_stop(EventListener::FocusGained, move |event| {
                        if let Event::FocusGained = event {
                            focus.set(Focus::Panel(PanelKind::Search))
                        }
                    }),
                clickable_icon(
                    || LapceIcons::SEARCH_CASE_SENSITIVE,
                    move || {
                        let new = match case_matching.get_untracked() {
                            CaseMatching::Exact => CaseMatching::CaseInsensitive,
                            CaseMatching::CaseInsensitive => CaseMatching::Exact,
                        };
                        case_matching.set(new);
                    },
                    move || case_matching.get() == CaseMatching::Exact,
                    || false,
                    || "Case Sensitive",
                    config,
                )
                .style(|s| s.padding_vert(4.0)),
                clickable_icon(
                    || LapceIcons::SEARCH_WHOLE_WORD,
                    move || {
                        whole_word.update(|whole_word| {
                            *whole_word = !*whole_word;
                        });
                    },
                    move || whole_word.get(),
                    || false,
                    || "Whole Word",
                    config,
                )
                .style(|s| s.padding_left(6.0)),
                clickable_icon(
                    || LapceIcons::SEARCH_REGEX,
                    move || {
                        is_regex.update(|is_regex| {
                            *is_regex = !*is_regex;
                        });
                    },
                    move || is_regex.get(),
                    || false,
                    || "Use Regex",
                    config,
                )
                .style(|s| s.padding_left(6.0)),
            ))
            .on_event_cont(EventListener::PointerDown, move |_| {
                focus.set(Focus::Panel(PanelKind::Search));
            })
            .style(move |s| {
                s.width_pct(100.0)
                    .padding_right(6.0)
                    .items_center()
                    .border(1.0)
                    .border_radius(6.0)
                    .border_color(config.with_color(LapceColor::LAPCE_BORDER))
            }),
        )
        .style(|s| s.width_pct(100.0).padding(10.0)),
        search_result(global_search, internal_command, config),
    ))
    .style(|s| s.absolute().size_pct(100.0, 100.0).flex_col())
    .debug_name("Global Search Panel")
}

fn search_result(
    global_search_data: GlobalSearchData,
    internal_command: Listener<InternalCommand>,
    config: WithLapceConfig,
) -> impl View {
    container({
        scroll({
            virtual_stack(
                move || global_search_data.clone(),
                move |item| item.clone(),
                move |item| match item {
                    SearchItem::Folder {
                        expanded,
                        file_name,
                        folder,
                        path,
                    } => container(result_fold(
                        config,
                        expanded,
                        file_name,
                        folder,
                        path.clone(),
                    )),
                    SearchItem::Item { path, m } => {
                        container(result_item(config, path, m, internal_command))
                    },
                },
            )
            .style(|s| s.flex_col().min_width_pct(100.0).line_height(1.8))
        })
        .style(|s| s.absolute().size_pct(100.0, 100.0))
    })
    .style(|s| s.size_pct(100.0, 100.0))
}

fn result_item(
    config: WithLapceConfig,
    path: PathBuf,
    m: SearchMatch,
    internal_command: Listener<InternalCommand>,
) -> impl View {
    let line_number = m.line;
    let start = m.start;
    let end = m.end;
    let line_content = m.line_content.clone();

    focus_text(
        move || {
            let content = if config
                .signal(|config| config.ui.trim_search_results_whitespace.signal())
                .get()
            {
                m.line_content.trim()
            } else {
                &m.line_content
            };
            format!("{}: {content}", m.line,)
        },
        move || {
            let mut offset = if config
                .signal(|config| config.ui.trim_search_results_whitespace.signal())
                .get()
            {
                line_content.trim_start().len() as i32 - line_content.len() as i32
            } else {
                0
            };
            offset += line_number.to_string().len() as i32 + 2;

            ((start as i32 + offset) as usize..(end as i32 + offset) as usize)
                .collect()
        },
        move || config.with_color(LapceColor::EDITOR_FOCUS),
    )
    .style(move |s| {
        let (hbg, icon_size) = config.signal(|config| {
            (
                config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                config.ui.icon_size.signal(),
            )
        });
        let icon_size = icon_size.get() as f32;
        s.margin_left(10.0 + icon_size + 6.0)
            .hover(|s| s.cursor(CursorStyle::Pointer).background(hbg.get()))
    })
    .on_click_stop(move |_| {
        internal_command.send(InternalCommand::JumpToLocation {
            location: EditorLocation {
                path:               path.clone(),
                position:           Some(EditorPosition::Line(
                    line_number.saturating_sub(1),
                )),
                scroll_offset:      None,
                ignore_unconfirmed: false,
                same_editor_tab:    false,
            },
        });
    })
}

fn result_fold(
    config: WithLapceConfig,
    expanded: RwSignal<bool>,
    file_name: String,
    folder: String,
    path: PathBuf,
) -> impl View {
    let style_path = path.clone();
    stack((
        svg(move || {
            config.with_ui_svg(if expanded.get() {
                LapceIcons::ITEM_OPENED
            } else {
                LapceIcons::ITEM_CLOSED
            })
        })
        .style(move |s| {
            let (border_color, size) = config.signal(|config| {
                (
                    config.color(LapceColor::LAPCE_ICON_ACTIVE),
                    config.ui.icon_size.signal(),
                )
            });
            let size = size.get() as f32;
            s.margin_left(10.0)
                .margin_right(6.0)
                .size(size, size)
                .min_size(size, size)
                .color(border_color.get())
        }),
        svg(move || config.with_file_svg(&path).0).style(move |s| {
            let (size, file_svg) = config.signal(|config| {
                (config.ui.icon_size.signal(), config.icon_theme.signal())
            });
            let color = file_svg.with(|x| x.file_svg(&style_path).1);
            let size = size.get() as f32;
            // let size = config.ui.icon_size() as f32;
            // let color = config.file_svg(&style_path).1;
            s.margin_right(6.0)
                .size(size, size)
                .min_size(size, size)
                .apply_opt(color, Style::color)
        }),
        stack((
            label(move || file_name.clone())
                .style(|s| s.margin_right(6.0).max_width_pct(100.0).text_ellipsis()),
            label(move || folder.clone()).style(move |s| {
                s.color(config.with_color(LapceColor::EDITOR_DIM))
                    .min_width(0.0)
                    .text_ellipsis()
            }),
        ))
        .style(move |s| s.min_width(0.0).items_center()),
    ))
    .on_click_stop(move |_| {
        expanded.update(|expanded| *expanded = !*expanded);
    })
    .style(move |s| {
        s.width_pct(100.0)
            .min_width_pct(100.0)
            .items_center()
            .hover(|s| {
                s.cursor(CursorStyle::Pointer).background(
                    config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                )
            })
    })
}

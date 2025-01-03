use std::rc::Rc;
use std::sync::Arc;
use floem::prelude::{clip, Color, container, Decorators, RwSignal, SignalGet, SignalWith, static_label, Svg};
use floem::reactive::{ReadSignal};
use floem::style::CursorStyle;
use floem::taffy::{AlignItems, JustifyContent};
use floem::View;
use floem::views::dyn_stack;
use log::{error, warn};
use crate::config::color::LapceColor;
use crate::config::icon::LapceIcons;
use crate::config::LapceConfig;
use crate::editor::{DocSignal, EditorData};
use crate::editor::gutter_new::{gutter_data, GutterData, GutterMarker};
use crate::svg;
use crate::window_tab::WindowTabData;

fn gutter_marker_none_svg_view(
    config: ReadSignal<Arc<LapceConfig>>
) -> Svg {
    svg(move || config.get().ui_svg(LapceIcons::EMPTY)).style(
        move |s| {
            let config = config.get();
            let size = config.ui.icon_size() as f64;
            s.size(size, size)
                .padding(2.0)
        },
    )
}

fn gutter_marker_breakpoint_svg_view(
    config: ReadSignal<Arc<LapceConfig>>
) -> Svg {
    svg(move || config.get().ui_svg(LapceIcons::DEBUG_BREAKPOINT)).style(
        move |s| {
            let config = config.get();
            let size = config.ui.icon_size() as f64;
            s.size(size, size)
                .color(config.color(LapceColor::DEBUG_BREAKPOINT_HOVER))
        },
    )
}

fn gutter_marker_code_len_svg_view(
    window_tab_data: Rc<WindowTabData>,
    line: usize,
    doc: DocSignal,
) -> Svg {
    let config = window_tab_data.common.config;
    svg(move || config.get().ui_svg(LapceIcons::START)).style(
        move |s| {
            let config = config.get();
            let size = config.ui.icon_size() as f64;
            s.size(size, size)
                .color(config.color(LapceColor::LAPCE_ICON_ACTIVE))
                .hover(|s| {
                    s.cursor(CursorStyle::Pointer)
                        .background(config.color(LapceColor::PANEL_HOVERED_BACKGROUND))
                })
                .active(|s| {
                    s.background(
                        config.color(LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND),
                    )
                })
        },
    )
        .on_click_stop({
            move |_| {
                let Some((plugin_id, offset, lens)) = doc.get_untracked().code_lens.get_untracked().get(&line).cloned() else {
                    error!("code_lens is empty: {}", line);
                    return;
                };
                window_tab_data.show_code_lens(true, plugin_id, offset, lens);
            }
        })
}

pub fn editor_gutter_new(window_tab_data: Rc<WindowTabData>,
                         e_data: RwSignal<EditorData>,
) -> impl View {
    let (doc, config) = e_data
        .with_untracked(|e| (e.doc_signal(), e.common.config));
    let window_tab_data_clone = window_tab_data.clone();
    container(
        clip(
            dyn_stack(
                move || gutter_data(window_tab_data_clone.clone(), e_data),
                |data| data.clone(),
                move |data| gutter_data_view(&data, &window_tab_data, doc, config),
            )
                .style(|style| style.height_full().width_full()),
        )
            .style(move |style| {
                let config = config.get();
                style
                    .width_full().height_pct(100.0)
                    .background(config.color(LapceColor::PANEL_BACKGROUND))
            })).style(move |style| {
        let doc = doc.get();
        let size = config.get().ui.icon_size() as f64;
        let width = doc.lines
            .with_untracked(|x| x.signal_last_line())
            .get().1 + size * 2.0;
        style
            .width(width) // 父组件宽度
            .height_full()
    }).debug_name("editor_gutter")
}


fn gutter_data_view(data: &GutterData, window_tab_data: &Rc<WindowTabData>, doc: DocSignal, config: ReadSignal<Arc<LapceConfig>>) -> impl View {
    let data = data.clone();
    container((
        static_label(data.display_line_num()).style(move |style| {
            let doc = doc.get();
            let width = doc.lines
                .with_untracked(|x| x.signal_last_line())
                .get().1;
            style
                .height_full()
                .width(width)
                .color(Color::rgb8(255, 0, 0))
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::FlexEnd)
        }),
        marker_view(&data, window_tab_data.clone(), config, doc)
    ))
        .style(move |style| {
            config.get().editor.line_height();
            style
                .absolute()
                .inset_top(data.vl_info.visual_line_y)
                .height(config.get().editor.line_height() as f64)
        })
}

fn marker_view(data: &GutterData, window_tab_data: Rc<WindowTabData>, config: ReadSignal<Arc<LapceConfig>>, doc_signal: DocSignal) -> impl View {
    let svg = match data.marker {
        GutterMarker::None => {
            gutter_marker_none_svg_view(config)
        }
        GutterMarker::CodeLen => {
            gutter_marker_code_len_svg_view(window_tab_data, 0, doc_signal)
        }
        GutterMarker::Breakpoint => {
            gutter_marker_breakpoint_svg_view(config)
        }
    };
    container(svg)
        .style(move |s| {
            // let config = config.get();
            s.padding_vert(2.0).padding_horiz(4.0)
                .border_radius(6.0)
                .justify_center()
                .items_center()
        })
        .on_click_stop(|_| {
            warn!("todo");
        })
}
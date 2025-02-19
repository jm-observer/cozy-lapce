use floem::{
    View,
    prelude::{
        Decorators, RwSignal, SignalGet, SignalWith, Svg, clip, container, palette,
        static_label
    },
    style::{CursorStyle, StyleValue},
    taffy::{AlignItems, JustifyContent},
    views::dyn_stack
};
use lapce_core::icon::LapceIcons;
use log::{error, warn};

use crate::{
    config::{color::LapceColor},
    editor::{
        DocSignal, EditorData,
        gutter_new::{GutterData, GutterMarker, gutter_data}
    },
    svg,
    window_workspace::WindowWorkspaceData
};
use crate::config::WithLapceConfig;

fn gutter_marker_none_svg_view(config: WithLapceConfig) -> Svg {
    svg(move || config.with_ui_svg(LapceIcons::EMPTY)).style(move |s| {
        let size = config.with_icon_size() as f64;
        s.size(size, size).padding(2.0)
    })
}

fn gutter_marker_breakpoint_svg_view(config: WithLapceConfig) -> Svg {
    svg(move || config.with_ui_svg(LapceIcons::DEBUG_BREAKPOINT)).style(move |s| {
        let (icon_size, color) = config.with(|config| {
            (config.ui.icon_size(), config.color(LapceColor::DEBUG_BREAKPOINT_HOVER))
        });
        let size = icon_size as f64;
        s.size(size, size)
            .color(color)
    })
}

fn gutter_marker_code_len_svg_view(
    window_tab_data: WindowWorkspaceData,
    line: usize,
    doc: DocSignal
) -> Svg {
    let config = window_tab_data.common.config;
    svg(move || config.with_ui_svg(LapceIcons::START))
        .style(move |s| {
            let size = config.with_icon_size() as f64;
            s.size(size, size)
                .color(palette::css::GREEN)
                .hover(|s| s.cursor(CursorStyle::Pointer))
        })
        .on_click_stop({
            move |_| {
                let code_lens = doc.get_untracked().code_lens.get_untracked();
                let Some((plugin_id, offset, lens)) = code_lens.get(&line).cloned()
                else {
                    error!("code_lens is empty: {} {:?}", line, code_lens);
                    return;
                };
                window_tab_data.show_code_lens(true, plugin_id, offset, lens);
            }
        })
}

pub fn editor_gutter_new(
    window_tab_data: WindowWorkspaceData,
    e_data: RwSignal<EditorData>
) -> impl View {
    let (doc, config) = e_data.with_untracked(|e| (e.doc_signal(), e.common.config));
    let window_tab_data_clone = window_tab_data.clone();
    container(
        clip(
            dyn_stack(
                move || gutter_data(window_tab_data_clone.clone(), e_data),
                |data| data.clone(),
                move |data| gutter_data_view(&data, &window_tab_data, doc, config)
            )
            .style(|style| style.height_full().width_full())
        )
        .style(move |style| {
            style
                .width_full()
                .height_pct(100.0)
                .background(config.with_color(LapceColor::PANEL_BACKGROUND))
        })
    )
    .style(move |style| {
        let doc = doc.get();
        let size = config.with_icon_size() as f64;
        let width = doc.lines.with_untracked(|x| x.signal_last_line()).get().1
            + size * 2.0
            + 8.0;
        style
            .width(width) // 父组件宽度
            .height_full()
    })
    .debug_name("editor_gutter")
}

fn gutter_data_view(
    data: &GutterData,
    window_tab_data: &WindowWorkspaceData,
    doc: DocSignal,
    config: WithLapceConfig
) -> impl View {
    let data = data.clone();
    container((
        static_label(data.display_line_num()).style(move |style| {
            let doc = doc.get();
            let width =
                doc.lines.with_untracked(|x| x.signal_last_line()).get().1 + 8.0;
            let (fg, dim, font_size, font_family) = config.with(|config| {
                (
                    config.color(LapceColor::EDITOR_FOREGROUND)
                    , config.color(LapceColor::EDITOR_DIM)
                    , config.editor.font_size()
                    , config.editor.font_family.clone()
                )
            });
            let color = if data.is_current_line {
                fg
            } else {
                dim
            };
            style
                .height_full()
                .width(width)
                .font_size(font_size as f32)
                .color(color)
                .padding_horiz(4.0)
                .font_family(StyleValue::Val(font_family))
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::FlexEnd)
        }),
        marker_view(&data, window_tab_data.clone(), config, doc)
    ))
    .style(move |style| {
        style
            .absolute()
            .inset_top(data.vl_info.visual_line_y)
            .height(config.with_line_height() as f64)
    })
}

fn marker_view(
    data: &GutterData,
    window_tab_data: WindowWorkspaceData,
    config: WithLapceConfig,
    doc_signal: DocSignal
) -> impl View {
    let svg = match data.marker {
        GutterMarker::None => gutter_marker_none_svg_view(config),
        GutterMarker::CodeLen => gutter_marker_code_len_svg_view(
            window_tab_data,
            data.vl_info.visual_line.origin_line,
            doc_signal
        ),
        GutterMarker::Breakpoint => gutter_marker_breakpoint_svg_view(config)
    };
    container(svg)
        .style(move |s| {
            let size = config.with_icon_size() as f64;
            let padding_left = 4.0;
            let padding_right = 10.0;
            let width = padding_left + padding_right + size;
            s.padding_right(padding_right)
                .padding_left(padding_left)
                .width(width)
                .border_radius(6.0)
                .justify_center()
                .items_center()
        })
        .on_click_stop(|_| {
            warn!("todo add/delete breakpoint");
        })
}

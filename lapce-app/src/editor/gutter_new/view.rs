use floem::{
    View,
    prelude::{Decorators, SignalGet, Svg, clip, container, palette, static_label},
    style::{CursorStyle, StyleValue},
    taffy::{AlignItems, JustifyContent},
    views::dyn_stack,
};
use lapce_core::icon::LapceIcons;

use crate::{
    config::{WithLapceConfig, color::LapceColor},
    editor::{
        DocSignal, EditorData,
        gutter_new::{GutterData, GutterMarker, gutter_data},
    },
    svg,
    window_workspace::WindowWorkspaceData,
};

fn gutter_marker_none_svg_view(config: WithLapceConfig) -> Svg {
    svg(move || config.with_ui_svg(LapceIcons::EMPTY)).style(move |s| {
        let size = config.with_icon_size() as f64;
        s.size(size, size).padding(2.0)
    })
}

fn gutter_marker_breakpoint_svg_view(config: WithLapceConfig) -> Svg {
    svg(move || config.with_ui_svg(LapceIcons::DEBUG_BREAKPOINT)).style(move |s| {
        let (icon_size, color) = config.signal(|config| {
            (
                config.ui.icon_size.signal(),
                config.color(LapceColor::DEBUG_BREAKPOINT),
            )
        });
        let size = icon_size.get() as f64;
        s.size(size, size).color(color.get())
    })
}

fn gutter_marker_code_len_svg_view(
    window_tab_data: WindowWorkspaceData,
    line: Option<usize>,
    doc: DocSignal,
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
                if let Some(line) = line {
                    let Some((plugin_id, offset, lens)) =
                        code_lens.get(&line).cloned()
                    else {
                        log::error!("code_lens is empty: {} {:?}", line, code_lens);
                        return;
                    };
                    window_tab_data.show_code_lens(true, plugin_id, offset, lens);
                }
            }
        })
}

pub fn editor_gutter_new(
    window_tab_data: WindowWorkspaceData,
    e_data: EditorData,
) -> impl View {
    let (doc, config) = (e_data.doc_signal(), e_data.common.config);
    let window_tab_data_clone = window_tab_data.clone();
    let e_data_gutter = e_data.clone();
    container(
        clip(
            dyn_stack(
                move || gutter_data(window_tab_data_clone.clone(), &e_data_gutter),
                |data| data.clone(),
                move |data| gutter_data_view(&data, &window_tab_data, doc, config),
            )
            .style(|style| style.height_full().width_full()),
        )
        .style(move |style| {
            style
                .width_full()
                .height_pct(100.0)
                .background(config.with_color(LapceColor::PANEL_BACKGROUND))
        }),
    )
    .style(move |style| {
        let doc = doc.get();
        let size = config.with_icon_size() as f64;
        let last_line_width =
            doc.lines.with_untracked(|x| x.signal_last_line()).get().1;
        let width = last_line_width + size * 2.0 + 8.0;
        log::info!("signal_last_line ={last_line_width} size={size}");
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
    config: WithLapceConfig,
) -> impl View {
    let data_clone = data.clone();
    let line_height = window_tab_data.common.ui_line_height;
    let paint_point_y = data.paint_point_y;
    container((
        static_label(data_clone.display_line_num())
            .style(move |style| {
                style
                    .height_full()
                    .width(data_clone.style_width)
                    .font_size(data_clone.style_font_size as f32 - 1.0)
                    .color(data_clone.style_color)
                    .padding_horiz(4.0)
                    .font_family(StyleValue::Val(
                        data_clone.style_font_family.clone(),
                    ))
                    .align_items(AlignItems::Center)
                    .justify_content(JustifyContent::FlexEnd)
            })
            .debug_name("line_num"),
        marker_view(data, window_tab_data.clone(), config, doc)
            .debug_name("break_point"),
    ))
    .style(move |style| {
        style
            .absolute()
            .inset_top(paint_point_y)
            .height(line_height.get())
    })
}

fn marker_view(
    data: &GutterData,
    window_tab_data: WindowWorkspaceData,
    config: WithLapceConfig,
    doc_signal: DocSignal,
) -> impl View {
    let window_tab_data_click = window_tab_data.clone();
    let svg = match data.marker {
        GutterMarker::None => gutter_marker_none_svg_view(config),
        GutterMarker::CodeLen => gutter_marker_code_len_svg_view(
            window_tab_data,
            data.origin_line_start,
            doc_signal,
        ),
        GutterMarker::Breakpoint => gutter_marker_breakpoint_svg_view(config),
    };
    let origin_line_start = data.origin_line_start;
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
        .on_click_stop(move |_| {
            if let Some(line) = origin_line_start {
                window_tab_data_click.common.internal_command.send(
                    crate::command::InternalCommand::AddOrRemoveBreakPoint {
                        doc:      doc_signal,
                        line_num: line,
                    },
                );
            }
        })
}

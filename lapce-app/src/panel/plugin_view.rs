use std::ops::Range;

use floem::{
    IntoView, View,
    event::EventListener,
    peniko::kurbo::{Point, Rect, Size},
    prelude::text_input,
    reactive::{
        RwSignal, SignalGet, SignalUpdate, SignalWith, create_memo, create_rw_signal,
    },
    style::CursorStyle,
    views::{
        Decorators, VirtualVector, container, dyn_container, img, label,
        scroll::scroll, stack, svg, virtual_stack,
    },
};
use indexmap::IndexMap;
use lapce_core::{
    icon::LapceIcons,
    panel::{PanelContainerPosition, PanelKind, PanelSection},
};
use lapce_rpc::{
    core::CoreRpcHandler,
    plugin::{VoltID, VoltInfo},
};

use super::view::PanelBuilder;
use crate::{
    app::not_clickable_icon,
    command::InternalCommand,
    config::color::LapceColor,
    plugin::{AvailableVoltData, InstalledVoltData, PluginData, VoltIcon},
    window_workspace::{Focus, WindowWorkspaceData},
};
pub const VOLT_DEFAULT_PNG: &[u8] = include_bytes!("../../../extra/images/volt.png");

struct IndexMapItems<K, V>(IndexMap<K, V>);

impl<K: Clone, V: Clone> IndexMapItems<K, V> {
    fn items(&self, range: Range<usize>) -> Vec<(K, V)> {
        let mut items = Vec::new();
        for i in range {
            if let Some((k, v)) = self.0.get_index(i) {
                items.push((k.clone(), v.clone()));
            }
        }
        items
    }
}

impl<K: Clone + 'static, V: Clone + 'static> VirtualVector<(usize, K, V)>
    for IndexMapItems<K, V>
{
    fn total_len(&self) -> usize {
        self.0.len()
    }

    fn slice(&mut self, range: Range<usize>) -> impl Iterator<Item = (usize, K, V)> {
        let start = range.start;
        Box::new(
            self.items(range)
                .into_iter()
                .enumerate()
                .map(move |(i, (k, v))| (i + start, k, v)),
        )
    }
}

pub fn plugin_panel(
    window_tab_data: WindowWorkspaceData,
    position: PanelContainerPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let plugin = window_tab_data.plugin.clone();
    let core_rpc = window_tab_data.proxy.core_rpc.clone();

    PanelBuilder::new(config, position)
        .add(
            "Installed",
            installed_view(plugin.clone()),
            window_tab_data.panel.section_open(PanelSection::Installed),
        )
        .add(
            "Available",
            available_view(plugin.clone(), core_rpc),
            window_tab_data.panel.section_open(PanelSection::Available),
        )
        .build()
        .debug_name("Plugin Panel")
}

fn installed_view(plugin: PluginData) -> impl View {
    let volts = plugin.installed;
    let config = plugin.common.config;
    let disabled = plugin.disabled;
    let workspace_disabled = plugin.workspace_disabled;
    let internal_command = plugin.common.internal_command;

    let view_fn = move |volt: InstalledVoltData, plugin: PluginData| {
        let meta = volt.meta.get_untracked();
        let volt_id = meta.id();
        let local_volt_id = volt_id.clone();
        let icon = volt.icon;
        stack((
            dyn_container(
                move || icon.get(),
                move |icon| match icon {
                    None => img(move || VOLT_DEFAULT_PNG.to_vec())
                        .style(|s| s.size_full())
                        .into_any(),
                    Some(VoltIcon::Svg(svg_str)) => svg(move || svg_str.clone())
                        .style(|s| s.size_full())
                        .into_any(),
                    Some(VoltIcon::Img(buf)) => {
                        img(move || buf.clone()).style(|s| s.size_full()).into_any()
                    },
                },
            )
            .style(|s| {
                s.min_size(50.0, 50.0)
                    .size(50.0, 50.0)
                    .margin_top(5.0)
                    .margin_right(10.0)
                    .padding(5)
            }),
            stack((
                label(move || meta.display_name.clone()).style(|s| {
                    s.font_bold()
                        .text_ellipsis()
                        .min_width(0.0)
                        .selectable(false)
                }),
                label(move || meta.description.clone())
                    .style(|s| s.text_ellipsis().min_width(0.0).selectable(false)),
                stack((
                    stack((
                        label(move || meta.author.clone()).style(|s| {
                            s.text_ellipsis().max_width_pct(100.0).selectable(false)
                        }),
                        label(move || {
                            if disabled.with(|d| d.contains(&volt_id))
                                || workspace_disabled.with(|d| d.contains(&volt_id))
                            {
                                "Disabled".to_string()
                            } else if volt.meta.with(|m| {
                                volt.latest.with(|i| i.version != m.version)
                            }) {
                                "Upgrade".to_string()
                            } else {
                                format!("v{}", volt.meta.with(|m| m.version.clone()))
                            }
                        })
                        .style(|s| s.text_ellipsis().selectable(false)),
                    ))
                    .style(|s| {
                        s.justify_between()
                            .flex_grow(1.0)
                            .flex_basis(0.0)
                            .min_width(0.0)
                    }),
                    not_clickable_icon(
                        || LapceIcons::SETTINGS,
                        || false,
                        || false,
                        || "Options",
                        config,
                    )
                    .style(|s| s.padding_left(6.0))
                    .popout_menu(move || {
                        plugin.plugin_controls(volt.meta.get(), volt.latest.get())
                    }),
                ))
                .style(|s| s.width_pct(100.0).items_center()),
            ))
            .style(|s| s.flex_col().flex_grow(1.0).flex_basis(0.0).min_width(0.0)),
        ))
        .on_click_stop(move |_| {
            internal_command.send(InternalCommand::OpenVoltView {
                volt_id: local_volt_id.clone(),
            });
        })
        .style(move |s| {
            s.width_pct(100.0)
                .padding_horiz(10.0)
                .padding_vert(5.0)
                .hover(|s| {
                    s.background(
                        config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    )
                })
        })
    };

    container(
        scroll(
            virtual_stack(
                // VirtualDirection::Vertical,
                // VirtualItemSize::Fixed(Box::new(move || {
                //     ui_line_height.get() * 3.0 + 10.0
                // })),
                move || IndexMapItems(volts.get()),
                move |(_, id, _)| id.clone(),
                move |(_, _, volt)| view_fn(volt, plugin.clone()),
            )
            .style(|s| s.flex_col().width_pct(100.0)),
        )
        .style(|s| s.absolute().size_pct(100.0, 100.0)),
    )
    .style(|s| {
        s.width_pct(100.0)
            .line_height(1.6)
            .flex_grow(1.0)
            .flex_basis(0.0)
    })
}

fn available_view(plugin: PluginData, core_rpc: CoreRpcHandler) -> impl View {
    let volts = plugin.available.volts;
    let installed = plugin.installed;
    let config = plugin.common.config;
    let internal_command = plugin.common.internal_command;

    let local_plugin = plugin.clone();
    let install_button =
        move |id: VoltID, info: RwSignal<VoltInfo>, installing: RwSignal<bool>| {
            let plugin = local_plugin.clone();
            let installed = create_memo(move |_| {
                installed.with(|installed| installed.contains_key(&id))
            });
            label(move || {
                if installed.get() {
                    "Installed".to_string()
                } else if installing.get() {
                    "Installing".to_string()
                } else {
                    "Install".to_string()
                }
            })
            .disabled(move || installed.get() || installing.get())
            .on_click_stop(move |_| {
                plugin.install_volt(info.get_untracked());
            })
            .style(move |s| {
                let (fg, bg, dim) = config.signal(|config| {
                    (
                        config.color(LapceColor::LAPCE_BUTTON_PRIMARY_FOREGROUND),
                        config.color(LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND),
                        config.color(LapceColor::EDITOR_DIM),
                    )
                });
                let bg = bg.get();
                s.color(fg.get())
                    .background(bg)
                    .margin_left(6.0)
                    .padding_horiz(6.0)
                    .border_radius(6.0)
                    .hover(|s| {
                        s.cursor(CursorStyle::Pointer)
                            .background(bg.multiply_alpha(0.8))
                    })
                    .active(|s| s.background(bg.multiply_alpha(0.6)))
                    .disabled(|s| s.background(dim.get()))
            })
        };

    let view_fn = move |(_, id, volt): (usize, VoltID, AvailableVoltData)| {
        let info = volt.info.get_untracked();
        let icon = volt.icon;
        let volt_id = info.id();
        stack((
            dyn_container(
                move || icon.get(),
                move |icon| match icon {
                    None => img(move || VOLT_DEFAULT_PNG.to_vec())
                        .style(|s| s.size_full())
                        .into_any(),
                    Some(VoltIcon::Svg(svg_str)) => svg(move || svg_str.clone())
                        .style(|s| s.size_full())
                        .into_any(),
                    Some(VoltIcon::Img(buf)) => {
                        img(move || buf.clone()).style(|s| s.size_full()).into_any()
                    },
                },
            )
            .style(|s| {
                s.min_size(50.0, 50.0)
                    .size(50.0, 50.0)
                    .margin_top(5.0)
                    .margin_right(10.0)
                    .padding(5)
            }),
            stack((
                label(move || info.display_name.clone()).style(|s| {
                    s.font_bold()
                        .text_ellipsis()
                        .min_width(0.0)
                        .selectable(false)
                }),
                label(move || info.description.clone())
                    .style(|s| s.text_ellipsis().min_width(0.0).selectable(false)),
                stack((
                    label(move || info.author.clone()).style(|s| {
                        s.text_ellipsis()
                            .min_width(0.0)
                            .flex_grow(1.0)
                            .flex_basis(0.0)
                            .selectable(false)
                    }),
                    install_button(id, volt.info, volt.installing),
                ))
                .style(|s| s.width_pct(100.0).items_center()),
            ))
            .style(|s| s.flex_col().flex_grow(1.0).flex_basis(0.0).min_width(0.0)),
        ))
        .on_click_stop(move |_| {
            internal_command.send(InternalCommand::OpenVoltView {
                volt_id: volt_id.clone(),
            });
        })
        .style(move |s| {
            s.width_pct(100.0)
                .padding_horiz(10.0)
                .padding_vert(5.0)
                .hover(|s| {
                    s.background(
                        config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    )
                })
        })
    };

    let content_rect = create_rw_signal(Rect::ZERO);

    let query_str = plugin.available.query_str;
    let focus = plugin.common.focus;
    let cursor_x = create_rw_signal(0.0);

    stack((
        container({
            scroll(
                text_input(query_str)
                    .placeholder("Search extensions")
                    .keyboard_navigable()
                    .style(|s| {
                        s.padding_vert(4.0).padding_horiz(10.0).min_width_pct(100.0)
                        // }).on_click_stop(move |event| {
                    }),
            )
            .ensure_visible(move || {
                Size::new(20.0, 0.0)
                    .to_rect()
                    .with_origin(Point::new(cursor_x.get(), 0.0))
            })
            .on_event_cont(EventListener::PointerDown, move |_| {
                focus.set(Focus::Panel(PanelKind::Plugin));
            })
            .scroll_style(|s| s.hide_bars(true))
            .style(move |s| {
                let (caret_color, bg) = config.signal(|config| {
                    (
                        config.color(LapceColor::EDITOR_BACKGROUND),
                        config.color(LapceColor::LAPCE_BORDER),
                    )
                });
                s.width_pct(100.0)
                    .cursor(CursorStyle::Text)
                    .items_center()
                    .background(caret_color.get())
                    .border(1.0)
                    .border_radius(6.0)
                    .border_color(bg.get())
            })
        })
        .style(|s| s.padding(10.0).width_pct(100.0)),
        container({
            scroll({
                virtual_stack(
                    // VirtualDirection::Vertical,
                    // VirtualItemSize::Fixed(Box::new(move || {
                    //     ui_line_height.get() * 3.0 + 10.0
                    // })),
                    move || IndexMapItems(volts.get()),
                    move |(_, id, _)| id.clone(),
                    view_fn,
                )
                .on_resize(move |rect| {
                    content_rect.set(rect);
                })
                .style(|s| s.flex_col().width_pct(100.0))
            })
            .on_scroll(move |rect| {
                if rect.y1 + 30.0 > content_rect.get_untracked().y1 {
                    plugin.load_more_available(core_rpc.clone());
                }
            })
            .style(|s| s.absolute().size_pct(100.0, 100.0))
        })
        .style(|s| s.size_pct(100.0, 100.0)),
    ))
    .style(|s| {
        s.width_pct(100.0)
            .line_height(1.6)
            .flex_grow(1.0)
            .flex_basis(0.0)
            .flex_col()
    })
}

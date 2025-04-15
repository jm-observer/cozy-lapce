use floem::{
    kurbo::{Rect, Size},
    reactive::*,
    style::CursorStyle,
    taffy::{
        Line,
        style_helpers::{self, auto, fr},
    },
    unit::{Auto, PxPctAuto},
    views::{scroll::VerticalScrollAsHorizontal, *},
    *,
};
use lapce_core::icon::LapceIcons;

use crate::{
    app::clickable_icon,
    common::TabHead,
    config::{
        WithLapceConfig,
        color::LapceColor,
        ui::{TabCloseButton, TabSeparatorHeight},
    },
    window_workspace::WindowWorkspaceData,
};

/// The top bar of an Editor tab. Includes the tab forward/back buttons, the tab
/// scroll bar and the new split and tab close all button.
pub fn common_tab_header<T: Clone + TabHead + 'static>(
    window_tab_data: WindowWorkspaceData,
    tabs: Tabs<T>,
) -> impl View {
    let config = window_tab_data.common.config;

    let resize_signal = create_rw_signal(());
    let scroll_offset = create_rw_signal(Rect::ZERO);
    stack((
        tabs.view_next_previoius().style(|s| s.flex_shrink(0.)),
        container(
            scroll({
                let tabs = tabs.clone();
                dyn_stack(
                    move || tabs.tabs(),
                    |(tab, _close_manager): &(Tab<T>, CloseManager<T>)| tab.key(),
                    |(tab, close_manager): (Tab<T>, CloseManager<T>)| {
                        tab.view_content(close_manager)
                    },
                )
                .debug_name("Horizontal Tab Stack")
                .style(|s| s.height_full().items_center())
            })
            .on_scroll(move |rect| {
                scroll_offset.set(rect);
            })
            .on_resize(move |_| {
                resize_signal.set(());
            })
            .ensure_visible({
                let tabs = tabs.clone();
                move || {
                    resize_signal.track();
                    if let Some(rect) = tabs.get_active_rect() {
                        rect.get()
                    } else {
                        Rect::ZERO
                    }
                }
            })
            .scroll_style(|s| s.hide_bars(true))
            .style(|s| {
                s.set(VerticalScrollAsHorizontal, true)
                    .absolute()
                    .size_full()
            }),
        )
        .style(|s| s.height_full().flex_grow(1.0).flex_basis(0.).min_width(10.))
        .debug_name("Tab scroll"),
        tabs.view_close(),
    ))
    .style(move |s| {
        let (border_color, bg, header_height) = config.signal(|config| {
            (
                config.color(LapceColor::LAPCE_BORDER),
                config.color(LapceColor::PANEL_BACKGROUND),
                config.ui.header_height.signal(),
            )
        });
        s.items_center()
            .flex_row()
            .width_full()
            .max_width_full()
            .border_bottom(1.0)
            .border_color(border_color.get())
            .background(bg.get())
            .height(header_height.get() as i32)
    })
    .debug_name("Tab Header")
}

fn tooltip_tip<V: View + 'static>(
    config: WithLapceConfig,
    child: V,
) -> impl IntoView {
    container(child).style(move |s| {
        let (border, shadow, fg, bg, font_size, font_family) =
            config.signal(|config| {
                (
                    config.color(LapceColor::LAPCE_BORDER),
                    config.color(LapceColor::LAPCE_DROPDOWN_SHADOW),
                    config.color(LapceColor::TOOLTIP_FOREGROUND),
                    config.color(LapceColor::TOOLTIP_BACKGROUND),
                    config.ui.font_size.signal(),
                    config.ui.font_family.signal(),
                )
            });
        s.padding_horiz(10.0)
            .padding_vert(5.0)
            .font_size(font_size.get() as f32)
            .font_family(font_family.get().1)
            .color(fg.get())
            .background(bg.get())
            .border(1)
            .border_radius(6)
            .border_color(border.get())
            .box_shadow_blur(3.0)
            .box_shadow_color(shadow.get())
            .margin_left(0.0)
            .margin_top(4.0)
    })
}

#[derive(Clone)]
pub struct Tabs<T: Clone + TabHead + 'static> {
    pub config:        WithLapceConfig,
    pub close_manager: CloseManager<T>,
    pub active:        RwSignal<Option<ViewId>>,
    pub tabs:          RwSignal<Vec<Tab<T>>>,
    pub cx:            Scope,
}

#[derive(Clone, Copy)]
pub struct CloseManager<T: Clone + TabHead + 'static> {
    pub tabs: RwSignal<Vec<Tab<T>>>,
}

impl<T: Clone + TabHead + 'static> CloseManager<T> {
    fn close(&self, id: ViewId) {
        self.tabs.update(|x| {
            let Some(index) = x
                .iter()
                .enumerate()
                .find_map(|item| if item.1.id == id { Some(item.0) } else { None })
            else {
                return;
            };
            x.remove(index);
        })
    }
}

#[derive(Clone)]
pub struct Tab<T: Clone + TabHead + 'static> {
    pub id:         ViewId,
    pub content:    String,
    pub active:     RwSignal<Option<ViewId>>,
    pub config:     WithLapceConfig,
    pub rect:       RwSignal<Rect>,
    pub references: RwSignal<T>,
}

impl<T: Clone + TabHead + 'static> Tab<T> {
    fn view_tab_close_button(
        &self,
        close_manager: CloseManager<T>,
    ) -> impl View + 'static {
        let config = self.config;
        let id = self.id;
        clickable_icon(
            move || LapceIcons::CLOSE,
            move || {
                close_manager.close(id);
            },
            || false,
            || false,
            || "Close",
            config,
        )
        .style(move |s| {
            let tab_close_button = config.with_tab_close_button();
            s.apply_if(tab_close_button == TabCloseButton::Left, |s| {
                s.grid_column(Line {
                    start: style_helpers::line(1),
                    end:   style_helpers::span(1),
                })
            })
            .apply_if(tab_close_button == TabCloseButton::Off, |s| s.hide())
        })
        // .on_event_stop(EventListener::PointerDown, |_| {})
        // .on_event_stop(EventListener::PointerEnter, move |_| {
        //     hovered.set(true);
        // })
        // .on_event_stop(EventListener::PointerLeave, move |_| {
        //     hovered.set(false);
        // })
    }

    fn view_tab_content(&self) -> impl View + 'static {
        let config = self.config();
        let (content, tip) = self.content_tip();
        tooltip(
            label(move || content.clone()).style(move |s| s.selectable(false)),
            move || tooltip_tip(config, text(tip.clone())),
        )
        .style(move |s| {
            let tab_close_button = config.with_tab_close_button();
            s.apply_if(tab_close_button == TabCloseButton::Left, |s| {
                s.grid_column(Line {
                    start: style_helpers::line(2),
                    end:   style_helpers::span(1),
                })
            })
            .apply_if(tab_close_button == TabCloseButton::Off, |s| {
                s.padding_right(4.)
            })
        })
    }

    fn tab_icon(&self) -> impl View + 'static {
        let config = self.config();
        let references = self.references;
        container({
            svg(move || config.with_ui_svg(references.with(|x| x.icon()))).style(
                move |s| {
                    let size = config.with_icon_size() as f32;
                    s.size(size, size)
                },
            )
        })
        .style(move |s| {
            let tab_close_button = config.with_tab_close_button();
            s.padding(4.)
                .apply_if(tab_close_button == TabCloseButton::Left, |s| {
                    s.grid_column(Line {
                        start: style_helpers::line(3),
                        end:   style_helpers::span(1),
                    })
                })
        })
    }

    fn view_content(&self, close_manager: CloseManager<T>) -> impl View + 'static {
        let config = self.config;
        let active = self.active;
        let rect = self.rect;
        let id = self.id;
        stack((
            stack((
                self.tab_icon(),
                self.view_tab_content(),
                self.view_tab_close_button(close_manager)
            ))
            .style(move |s| {
                s.items_center()
                    .justify_center()
                    // .border_left(if i.get() == 0 { 1.0 } else { 0.0 })
                    .border_right(1.0)
                    .border_color(config.with_color(LapceColor::LAPCE_BORDER))
                    .padding_horiz(6.)
                    .gap(6.)
                    .grid()
                    .grid_template_columns(vec![auto(), fr(1.), auto()])
                    .apply_if(
                        config.signal(|x| x.ui.tab_separator_height.signal()).get() == TabSeparatorHeight::Full
                            ,
                        |s| s.height_full(),
                    )
            }),
            empty()
                .style(move |s| {
                    let active = active.get().map(|x| id == x).unwrap_or_default();
                    s.size_full()
                        .border_bottom(if active { 2.0 } else { 0.0 })
                        .border_color(config.with_color(if active {
                            LapceColor::LAPCE_TAB_ACTIVE_UNDERLINE
                        } else {
                            LapceColor::LAPCE_TAB_INACTIVE_UNDERLINE
                        }))
                })
                .style(|s| s.absolute().padding_horiz(3.0).size_full())
                .debug_name("Tab Indicator"),
            empty()
                .style(move |s| {
                    s.absolute().height_full().border_color(
                        config.with_color(LapceColor::LAPCE_TAB_ACTIVE_UNDERLINE)
                            .multiply_alpha(0.5)
                    )
                })
                .debug_name("Tab Boundary")
        ))
        .on_click_stop(move |_| {
            active.set(Some(id));
        })
        .on_resize(move |x| rect.set(x))
        .style(move |s| {
            s.height_full()
                .flex_col()
                .items_center()
                .justify_center()
                .cursor(CursorStyle::Pointer)
                .hover(|s| s.background(config.with_color(LapceColor::HOVER_BACKGROUND)))
        })
    }

    fn content_tip(&self) -> (String, String) {
        (self.content.clone(), "tip".to_owned())
    }

    fn config(&self) -> WithLapceConfig {
        self.config
    }

    fn key(&self) -> ViewId {
        self.id
    }
}

impl<T: Clone + TabHead + 'static> Tabs<T> {
    pub fn new(config: WithLapceConfig, cx: Scope) -> Self {
        let active = cx.create_rw_signal(None);
        let tabs = cx.create_rw_signal(Vec::new());
        let close_manager = CloseManager { tabs };

        Self {
            config,
            tabs,
            close_manager,
            active,
            cx,
        }
    }

    pub fn push_tab(&self, content: String, references: T) {
        let id = ViewId::new();
        let active = self.active;
        let config = self.config;
        let rect = self.cx.create_rw_signal(Rect::ZERO);
        let references = self.cx.create_rw_signal(references);
        // let content = format!("{:?}{}", id, content);
        let tab = Tab {
            id,
            content,
            active,
            config,
            rect,
            references,
        };
        batch(|| {
            self.tabs.update(|x| x.push(tab));
            self.active.set(Some(id));
        });
    }

    fn tabs(&self) -> impl IntoIterator<Item = (Tab<T>, CloseManager<T>)> + 'static {
        self.tabs
            .get()
            .into_iter()
            .map(|x| (x, self.close_manager.clone()))
            .collect::<Vec<_>>()
    }

    fn view_close(&self) -> impl View + 'static {
        let config = self.config;
        let close_tabs = self.clone();
        clickable_icon(
            || LapceIcons::CLOSE,
            move || {
                close_tabs.action_close_all();
            },
            || false,
            || false,
            || "Close All",
            config,
        )
        .style(|s| {
            s.margin_horiz(6.0)
                .margin_left(Auto)
                .items_center()
                .height_full()
        })
        // .on_resize(move |rect| {
        //     size.set(rect.size());
        // }),)
        .debug_name("Close Panel Buttons")
        .style(move |s| {
            // let content_size = content_size.get();
            // let scroll_offset = scroll_offset.get();
            s.height_full().flex_shrink(0.).margin_left(PxPctAuto::Auto)
            // .apply_if(scroll_offset.x1 < content_size.width, |s| {
            //     s.margin_left(0.)
            // })
        })
    }

    pub fn action_next_tab(&self) {
        if let (_, Some(id)) = self.get_pre_next_id() {
            self.active.set(Some(id));
        }
    }

    pub fn action_pre_tab(&self) {
        if let (Some(id), _) = self.get_pre_next_id() {
            self.active.set(Some(id));
        }
    }

    pub fn action_close_all(&self) {
        batch(|| {
            self.active.set(None);
            self.tabs.update(|x| x.clear());
        });
    }

    pub fn get_active_tab(&self) -> Option<(usize, Tab<T>)> {
        self.tabs.with(|x| {
            if let Some(active) = self.active.get() {
                if x.is_empty() {
                    return None;
                }
                x.iter().enumerate().find_map(|item| {
                    if item.1.id == active {
                        Some((item.0, item.1.clone()))
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
    }

    pub fn get_active_content(&self) -> Option<T> {
        self.get_active_tab().map(|x| x.1.references.get())
    }

    pub fn get_active_rect(&self) -> Option<RwSignal<Rect>> {
        self.get_active_tab().map(|x| x.1.rect)
    }

    fn get_pre_next_id(&self) -> (Option<ViewId>, Option<ViewId>) {
        self.tabs.with_untracked(|x| {
            if x.is_empty() {
                return (None, None);
            }
            let (pre_index, next_index) =
                if let Some(active) = self.active.get_untracked() {
                    if let Some(index) = x.iter().enumerate().find_map(|item| {
                        if item.1.id == active {
                            Some(item.0)
                        } else {
                            None
                        }
                    }) {
                        (index.saturating_sub(1), (index + 1).min(x.len() - 1))
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                };
            (Some(x[pre_index].id), Some(x[next_index].id))
        })
    }

    pub fn view_next_previoius(&self) -> impl View + 'static {
        let config = self.config;
        let size = create_rw_signal(Size::ZERO);

        let pre_tabs = self.clone();
        let next_tabs = self.clone();
        stack((
            clickable_icon(
                || LapceIcons::TAB_PREVIOUS,
                move || {
                    pre_tabs.action_pre_tab();
                },
                || false,
                || false,
                || "Previous Tab",
                config,
            )
            .style(|s| s.margin_horiz(6.0).margin_vert(7.0)),
            clickable_icon(
                || LapceIcons::TAB_NEXT,
                move || {
                    next_tabs.action_next_tab();
                },
                || false,
                || false,
                || "Next Tab",
                config,
            )
            .style(|s| s.margin_right(6.0)),
            empty()
                .style(move |s| {
                    s.absolute().height_full().border_color(
                        config
                            .with_color(LapceColor::LAPCE_TAB_ACTIVE_UNDERLINE)
                            .multiply_alpha(0.5),
                    )
                })
                .debug_name("Tab Boundary"),
        ))
        .on_resize(move |rect| {
            size.set(rect.size());
        })
        .debug_name("Next/Previoius Tab Buttons")
        .style(move |s| s.items_center())
    }
}

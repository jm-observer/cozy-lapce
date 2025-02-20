use std::{fmt, rc::Rc, sync::atomic::AtomicU64};

use floem::{
    View,
    event::EventListener,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate},
    style::CursorStyle,
    views::{Decorators, container, dyn_stack, label, stack}
};
use lapce_core::icon::LapceIcons;

use crate::{
    config::{WithLapceConfig, color::LapceColor},
    svg,
    window_workspace::CommonData
};

#[derive(Clone)]
pub struct AlertButton {
    pub text:   String,
    pub action: Rc<dyn Fn()>
}

impl fmt::Debug for AlertButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("AlertButton");
        s.field("text", &self.text);
        s.finish()
    }
}

#[derive(Clone)]
pub struct AlertBoxData {
    pub active:  RwSignal<bool>,
    pub title:   RwSignal<String>,
    pub msg:     RwSignal<String>,
    pub buttons: RwSignal<Vec<AlertButton>>,
    pub config:  WithLapceConfig
}

impl AlertBoxData {
    pub fn new(cx: Scope, common: Rc<CommonData>) -> Self {
        Self {
            active:  cx.create_rw_signal(false),
            title:   cx.create_rw_signal("".to_string()),
            msg:     cx.create_rw_signal("".to_string()),
            buttons: cx.create_rw_signal(Vec::new()),
            config:  common.config
        }
    }
}

pub fn alert_box(alert_data: AlertBoxData) -> impl View {
    let config = alert_data.config;
    let active = alert_data.active;
    let title = alert_data.title;
    let msg = alert_data.msg;
    let buttons = alert_data.buttons;
    let button_id = AtomicU64::new(0);

    container({
        container({
            stack((
                svg(move || config.with_ui_svg(LapceIcons::WARNING)).style(
                    move |s| {
                        s.size(50.0, 50.0)
                            .color(config.with_color(LapceColor::LAPCE_WARN))
                    }
                ),
                label(move || title.get()).style(move |s| {
                    s.margin_top(20.0)
                        .width_pct(100.0)
                        .font_bold()
                        .font_size((config.with_font_size() + 1) as f32)
                }),
                label(move || msg.get())
                    .style(move |s| s.width_pct(100.0).margin_top(10.0)),
                dyn_stack(
                    move || buttons.get(),
                    move |_button| {
                        button_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    },
                    move |button| {
                        label(move || button.text.clone())
                            .on_click_stop(move |_| {
                                (button.action)();
                            })
                            .style(move |s| {
                                let (font_size, border_color, br_color, abr_color) =
                                    config.with(|config| {
                                        (config.ui.font_size()
                                    , config.color(LapceColor::LAPCE_BORDER)
                                    ,config.color(
                                        LapceColor::PANEL_HOVERED_BACKGROUND
                                    ),config.color(
                                        LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND,
                                    ))
                                    });
                                s.margin_top(10.0)
                                    .width_pct(100.0)
                                    .justify_center()
                                    .font_size((font_size + 1) as f32)
                                    .line_height(1.6)
                                    .border(1.0)
                                    .border_radius(6.0)
                                    .border_color(border_color)
                                    .hover(|s| {
                                        s.cursor(CursorStyle::Pointer)
                                            .background(br_color)
                                    })
                                    .active(|s| s.background(abr_color))
                            })
                    }
                )
                .style(|s| s.flex_col().width_pct(100.0).margin_top(10.0)),
                label(|| "Cancel".to_string())
                    .on_click_stop(move |_| {
                        active.set(false);
                    })
                    .style(move |s| {
                        let (font_size, border_color, br_color, abr_color) = config
                            .with(|config| {
                                (
                                    config.ui.font_size(),
                                    config.color(LapceColor::LAPCE_BORDER),
                                    config
                                        .color(LapceColor::PANEL_HOVERED_BACKGROUND),
                                    config.color(
                                        LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND
                                    )
                                )
                            });
                        s.margin_top(20.0)
                            .width_pct(100.0)
                            .justify_center()
                            .font_size((font_size + 1) as f32)
                            .line_height(1.5)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(border_color)
                            .hover(|s| {
                                s.cursor(CursorStyle::Pointer).background(br_color)
                            })
                            .active(|s| s.background(abr_color))
                    })
            ))
            .style(|s| s.flex_col().items_center().width_pct(100.0))
        })
        .on_event_stop(EventListener::PointerDown, |_| {})
        .style(move |s| {
            let (border_color, fr, br) = config.with(|config| {
                (
                    config.color(LapceColor::LAPCE_BORDER),
                    config.color(LapceColor::EDITOR_FOREGROUND),
                    config.color(LapceColor::PANEL_BACKGROUND)
                )
            });
            s.padding(20.0)
                .width(250.0)
                .border(1.0)
                .border_radius(6.0)
                .border_color(border_color)
                .color(fr)
                .background(br)
        })
    })
    .on_event_stop(EventListener::PointerDown, move |_| {})
    .style(move |s| {
        s.absolute()
            .size_pct(100.0, 100.0)
            .items_center()
            .justify_center()
            .apply_if(!active.get(), |s| s.hide())
            .background(
                config
                    .with_color(LapceColor::LAPCE_DROPDOWN_SHADOW)
                    .multiply_alpha(0.5)
            )
    })
    .debug_name("Alert Box")
}

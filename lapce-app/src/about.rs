use doc::lines::{
    command::FocusCommand, editor_command::CommandExecuted, mode::Mode,
};
use floem::{
    View,
    event::EventListener,
    keyboard::Modifiers,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate},
    style::{CursorStyle, Display, Position},
    views::{Decorators, container, label, stack, svg},
};
use lapce_core::meta::*;

use crate::{
    command::CommandKind,
    config::color::LapceColor,
    keypress::KeyPressFocus,
    web_link::web_link,
    window_workspace::{Focus, SignalManager, WindowWorkspaceData},
};

struct AboutUri {}

impl AboutUri {
    const CODICONS: &'static str = "https://github.com/microsoft/vscode-codicons";
    // const DISCORD: &'static str = "https://discord.gg/n8tGJ6Rn6D";
    const GITHUB: &'static str = "https://github.com/jm-observer/cozy-lapce";
    // const LAPCE: &'static str = "https://lapce.dev";
    // const MATRIX: &'static str = "https://matrix.to/#/#lapce-editor:matrix.org";
}

#[derive(Clone, Debug)]
pub struct AboutData {
    pub visible: RwSignal<bool>,
    pub focus:   SignalManager<Focus>,
}

impl AboutData {
    pub fn new(cx: Scope, focus: SignalManager<Focus>) -> Self {
        let visible = cx.create_rw_signal(false);

        Self { visible, focus }
    }

    pub fn open(&self) {
        self.visible.set(true);
        self.focus.set(Focus::AboutPopup);
    }

    pub fn close(&self) {
        self.visible.set(false);
        self.focus.set(Focus::Workbench);
    }
}

impl KeyPressFocus for AboutData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(
        &self,
        _condition: crate::keypress::condition::Condition,
    ) -> bool {
        self.visible.get_untracked()
    }

    fn run_command(
        &self,
        command: &crate::command::LapceCommand,
        _count: Option<usize>,
        _mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(_) => {},
            CommandKind::Edit(_) => {},
            CommandKind::Move(_) => {},
            CommandKind::Scroll(_) => {},
            CommandKind::Focus(cmd) => {
                if cmd == &FocusCommand::ModalClose {
                    self.close();
                }
            },
            CommandKind::MotionMode(_) => {},
            CommandKind::MultiSelection(_) => {},
        }
        CommandExecuted::Yes
    }

    fn receive_char(&self, _c: &str) {}

    fn focus_only(&self) -> bool {
        true
    }
}

pub fn about_popup(window_tab_data: WindowWorkspaceData) -> impl View {
    let about_data = window_tab_data.about_data.clone();
    let config = window_tab_data.common.config;
    let internal_command = window_tab_data.common.internal_command;
    let logo_size = 100.0;

    exclusive_popup(window_tab_data, about_data.visible, move || {
        stack((
            svg(move || config.signal(|config| config.logo_svg())).style(move |s| {
                s.size(logo_size, logo_size)
                    .color(config.with_color(LapceColor::EDITOR_FOREGROUND))
            }),
            label(|| "CozyLapce".to_string()).style(move |s| {
                s.font_bold()
                    .margin_top(10.0)
                    .color(config.with_color(LapceColor::EDITOR_FOREGROUND))
            }),
            label(|| format!("Version: {}", VERSION)).style(move |s| {
                s.margin_top(10.0)
                    .color(config.with_color(LapceColor::EDITOR_DIM))
            }),
            label(|| format!("Build Date: {}", BUILD_DATE)).style(move |s| {
                s.margin_top(10.0)
                    .color(config.with_color(LapceColor::EDITOR_DIM))
            }),
            // web_link(
            //     || "Website".to_string(),
            //     || AboutUri::LAPCE.to_string(),
            //     move || config.with_color(LapceColor::EDITOR_LINK),
            //     internal_command,
            // )
            // .style(|s| s.margin_top(20.0)),
            web_link(
                || "GitHub".to_string(),
                || AboutUri::GITHUB.to_string(),
                move || config.with_color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(10.0)),
            // web_link(
            //     || "Discord".to_string(),
            //     || AboutUri::DISCORD.to_string(),
            //     move || config.with_color(LapceColor::EDITOR_LINK),
            //     internal_command,
            // )
            // .style(|s| s.margin_top(10.0)),
            // web_link(
            //     || "Matrix".to_string(),
            //     || AboutUri::MATRIX.to_string(),
            //     move || config.with_color(LapceColor::EDITOR_LINK),
            //     internal_command,
            // )
            // .style(|s| s.margin_top(10.0)),
            label(|| "Attributions".to_string()).style(move |s| {
                s.font_bold()
                    .color(config.with_color(LapceColor::EDITOR_DIM))
                    .margin_top(40.0)
            }),
            web_link(
                || "Codicons (CC-BY-4.0)".to_string(),
                || AboutUri::CODICONS.to_string(),
                move || config.with_color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(10.0)),
        ))
        .style(|s| s.flex_col().items_center())
    })
    .debug_name("About Popup")
}

fn exclusive_popup<V: View + 'static>(
    window_tab_data: WindowWorkspaceData,
    visibility: RwSignal<bool>,
    content: impl FnOnce() -> V,
) -> impl View {
    let config = window_tab_data.common.config;

    container(
        container(
            container(content())
                .style(move |s| {
                    let (border_color, bg) = config.signal(|config| {
                        (
                            config.color(LapceColor::LAPCE_BORDER)
                            , config.color(LapceColor::PANEL_BACKGROUND)
                        )
                    });

                    s.padding_vert(25.0)
                        .padding_horiz(100.0)
                        .border(1.0)
                        .border_radius(6.0)
                        .border_color(border_color.get())
                        .background(bg.get())
                })
                .on_event_stop(EventListener::PointerDown, move |_| {}),
        )
        .style(move |s| {
            s.flex_grow(1.0)
                .flex_row()
                .items_center()
                .hover(move |s| s.cursor(CursorStyle::Default))
        }),
    )
    .on_event_stop(EventListener::PointerDown, move |_| {
        window_tab_data.about_data.close();
    })
    // Prevent things behind the grayed out area from being hovered.
    .on_event_stop(EventListener::PointerMove, move |_| {})
    .style(move |s| {
        s.display(if visibility.get() {
            Display::Flex
        } else {
            Display::None
        })
        .position(Position::Absolute)
        .size_pct(100.0, 100.0)
        .flex_col()
        .items_center()
        .background(
            config.with_color(LapceColor::LAPCE_DROPDOWN_SHADOW)
                .multiply_alpha(0.5),
        )
    })
}

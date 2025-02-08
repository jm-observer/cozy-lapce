use std::fs;

use floem::{
    keyboard::{Key, NamedKey},
    prelude::*,
    style::CursorStyle,
    views::container
};

fn app_view() -> impl IntoView {
    let view = v_stack((
        container(svg(svg_str("other")).style(move |s| {
            // Color::from_rgba8(0, 0, 0, 100)
            let size = 13.0;
            s.size(size, size)
                .color(Color::from_rgba8(0, 0, 0, 120))
                .hover(|s| s.cursor(CursorStyle::Pointer).color(palette::css::BLACK))
        })),
        container(svg(svg_str("folded")).style(move |s| {
            let size = 13.0;
            s.size(size, size).color(palette::css::RED).padding(2.0) // 无效
        })),
        container(svg(svg_str("folded-end")).style(move |s| {
            let size = 13.0;
            s.size(size, size)
        })),
        container(svg(svg_str("start")).style(move |s| {
            let size = 13.0;
            s.size(size, size).color(palette::css::GREEN)
        })),
        container(svg(svg_str("start-big")).style(move |s| {
            let size = 13.0;
            s.size(size, size).color(palette::css::GREEN)
        })),
        container(svg(svg_str("other")).style(move |s| {
            let size = 13.0;
            s.size(size, size)
        }))
    ))
    .style(|x| x.margin(100.0));

    let id = view.id();
    view.on_key_up(
        Key::Named(NamedKey::F11),
        |m| m.is_empty(),
        move |_| id.inspect()
    )
}

fn main() {
    floem::launch(app_view);
}

fn svg_str(svg_name: &str) -> String {
    match svg_name {
        "folded-start" => {
            fs::read_to_string("resources/svg/folding-start.svg").unwrap()
        },
        "folded" => fs::read_to_string("resources/svg/folding-folded.svg").unwrap(),
        "folded-end" => fs::read_to_string("resources/svg/folding-end.svg").unwrap(),
        "other" => fs::read_to_string("resources/svg/warning.svg").unwrap(),
        "start" => fs::read_to_string("resources/svg/start.svg").unwrap(),
        "start-big" => fs::read_to_string("resources/svg/start-big.svg").unwrap(),
        "debug" => fs::read_to_string("resources/svg/debug.svg").unwrap(),
        _ => {
            panic!()
        }
    }
}

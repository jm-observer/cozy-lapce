use std::str::FromStr;

use floem::{
    keyboard::{Key, NamedKey},
    prelude::*,
    style::{AlignItems, JustifyContent, StyleValue},
    views::container,
};

fn app_view() -> impl IntoView {
    // Color::rgba8(0, 0, 0, 120)
    // (0, 255, 0),
    // (0, 0, 255)
    let view = v_stack((
        container(label("warning", Some(Color::from_rgb8(255, 255, 85)), None))
            .style(|x| x.width(10.0)),
        container(label("{", Some(Color::from_rgb8(0, 0, 255)), None))
            .style(|x| x.width(10.0)),
        container(label("{", Some(Color::from_str("#98FB98").unwrap()), None))
            .style(|x| x.width(10.0)),
        container(
            label("{", None, Some(Color::from_str("#B7E1CD").unwrap()))
                .style(|x| x.width(10.0)),
        )
        .style(|x| x.width(10.0)),
        container(label("    println!(\"abc\")", None, None))
            .style(|x| x.width(100.0)),
    ));

    let id = view.id();
    view.on_key_up(
        Key::Named(NamedKey::F11),
        |m| m.is_empty(),
        move |_| id.inspect(),
    )
}

fn main() {
    floem::launch(app_view);
}

fn label(text: &str, color: Option<Color>, bg_color: Option<Color>) -> impl View {
    static_label(text).style(move |style| {
        let style = style
            .height(23.0)
            .font_size(13.0)
            .padding_horiz(4.0)
            .font_family(StyleValue::Val("JetBrains Mono".to_string()))
            .align_items(AlignItems::Center)
            .justify_content(JustifyContent::FlexEnd);
        let style = if let Some(color) = color {
            style.color(color)
        } else {
            style
        };
        if let Some(bg) = bg_color {
            style.background(bg)
        } else {
            style
        }
    })
}

use std::{borrow::Cow, time::Duration};

use ansi_to_style::TextStyle;
use cozy_floem::{
    channel::ExtChannel,
    views::{
        panel::{DocManager, DocStyle, ErrLevel, TextSrc, panel},
        tree_with_panel::data::{Level, StyledText, TreePanelData},
    },
};
use floem::{
    View,
    keyboard::{Key, NamedKey},
    peniko::Color,
    prelude::Decorators,
    reactive::Scope,
    text::{Attrs, AttrsList, FamilyOwned, LineHeightValue, Weight},
};
use log::LevelFilter::Info;

fn main() -> anyhow::Result<()> {
    let _ = custom_utils::logger::logger_feature(
        "panel",
        "error,cozy_simple_panel=debug,cozy_floem=debug",
        Info,
        false,
    )
    .build();

    let cx = Scope::new();
    let data = TreePanelData::new(cx, DocStyle::default());
    data.run_with_async_task(init_content);
    floem::launch(move || app_view(data.doc));
    Ok(())
}

fn app_view(simple_doc: DocManager) -> impl View {
    let view = panel(simple_doc);
    let id = view.id();
    view.on_key_up(
        Key::Named(NamedKey::F11),
        |m| m.is_empty(),
        move |_| id.inspect(),
    )
}

async fn init_content(mut channel: ExtChannel<StyledText>) -> anyhow::Result<()> {
    let family = Cow::Owned(FamilyOwned::parse_list("JetBrains Mono").collect());
    let font_size = 13.0;
    let attrs = Attrs::new()
        // .color(self.editor_style.ed_text_color())
        .family(&family)
        .font_size(font_size as f32)
        .line_height(LineHeightValue::Px(23.0));
    let mut attr_list = AttrsList::new(attrs);
    let attrs = Attrs::new()
        .color(Color::from_rgba8(214, 214, 51, 255))
        .family(&family)
        .font_size(font_size as f32)
        .weight(Weight::BOLD)
        .line_height(LineHeightValue::Px(23.0));
    attr_list.add_span(3..12, attrs);

    for i in 0..20 {
        let content = format!(
            "{}-{}",
            "   Compiling icu_collections v1.5.0         1234567890", i
        );
        let line = StyledText {
            id:          TextSrc::StdErr {
                level: ErrLevel::Error,
            },
            level:       Level::Error,
            styled_text: ansi_to_style::TextWithStyle {
                text:   content,
                styles: vec![TextStyle {
                    range:     3..12,
                    bold:      true,
                    italic:    false,
                    underline: false,
                    bg_color:  None,
                    fg_color:  Some(Color::from_rgba8(214, 214, 51, 255)),
                }],
            },
            hyperlink:   vec![],
        };
        channel.send(line);
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    Ok(())
}

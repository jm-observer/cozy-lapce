use cozy_floem::{
    channel::ExtChannel,
    views::{
        panel::DocStyle,
        tree_with_panel::{
            data::{StyledText, TreePanelData},
            tree_with_panel
        }
    }
};
use floem::{
    Application, View,
    keyboard::{Key, NamedKey},
    kurbo::Point,
    prelude::Decorators,
    reactive::Scope,
    window::WindowConfig
};
use log::LevelFilter::Info;
use rust_resolve::async_command::run_command;
use tokio::process::Command;

fn main() -> anyhow::Result<()> {
    let _ = custom_utils::logger::logger_feature(
        "panel",
        "warn,rust_resolve=debug,cozy_rust_panel=debug,cozy_floem=debug,\
         cozy_rust_tree_panel=debug",
        Info,
        false
    )
    .build();

    let cx = Scope::new();
    let data = TreePanelData::new(cx, DocStyle::default());
    data.run_with_async_task(_run);
    let config = WindowConfig::default().position(Point::new(300.0, 300.));
    Application::new()
        .window(move |_| app_view(data), Some(config))
        .run();
    Ok(())
}

fn app_view(data: TreePanelData) -> impl View {
    let view = tree_with_panel(data).style(|x| x.height(300.0).width(800.0));
    let id = view.id();

    view.on_key_up(
        Key::Named(NamedKey::F11),
        |m| m.is_empty(),
        move |_| id.inspect()
    )
}

async fn _run(channel: ExtChannel<StyledText>) -> anyhow::Result<()> {
    let mut command = Command::new("cargo");
    command.args(["clean", "--manifest-path", "D:\\git\\check_2\\Cargo.toml"]);
    command.output().await?;

    let mut command = Command::new("cargo");
    command.args([
        "build",
        "--message-format=json-diagnostic-rendered-ansi",
        "--color=always",
        "--manifest-path",
        "D:\\git\\check_2\\Cargo.toml",
        "--package",
        "check",
        "--bin",
        "check"
    ]);
    run_command(command, channel).await?;
    Ok(())
}

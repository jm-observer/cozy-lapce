use cozy_floem::{
    channel::ExtChannel,
    views::{
        panel::{DocManager, DocStyle, panel},
        tree_with_panel::data::{StyledText, TreePanelData},
    },
};
use floem::{
    Application, View,
    keyboard::{Key, NamedKey},
    kurbo::Point,
    prelude::Decorators,
    reactive::Scope,
    views::stack,
    window::WindowConfig,
};
use log::{LevelFilter::Info, error};
use rust_resolve::async_command::run_command;
use tokio::process::Command;

fn main() -> anyhow::Result<()> {
    let _ = custom_utils::logger::logger_feature(
        "panel",
        "warn,rust_resolve=debug,cozy_rust_panel=debug,cozy_floem=debug",
        Info,
        false,
    )
    .build();

    let cx = Scope::new();
    let data = TreePanelData::new(cx, DocStyle::default());
    data.run_with_async_task(_run);
    let config = WindowConfig::default().position(Point::new(300.0, 300.));
    Application::new()
        .window(move |_| app_view(data.doc), Some(config))
        .run();
    Ok(())
}

fn app_view(simple_doc: DocManager) -> impl View {
    let view = stack((panel(simple_doc).style(|x| x.width(600.).height(300.)),));
    let id = view.id();

    view.on_key_up(
        Key::Named(NamedKey::F11),
        |m| m.is_empty(),
        move |_| id.inspect(),
    )
}

#[tokio::main(flavor = "current_thread")]
pub async fn run(channel: ExtChannel<StyledText>) {
    if let Err(err) = _run(channel).await {
        error!("{:?}", err);
    }
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
        "check",
    ]);
    run_command(command, channel).await?;
    Ok(())
}

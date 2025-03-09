use log::info;

pub fn main() {
    let _ = custom_utils::logger::logger_feature(
        "test",
        "warn,wgpu_core=error,lapce_app::keypress::loader=info",
        log::LevelFilter::Info,
        true,
    )
        .build();
    // let _ = custom_utils::logger::logger_stdout(log::LevelFilter::Info).log_to_stdout();
    log::debug!("startss");
    log::info!("startss");
    log::warn!("startss");
    let a = 'a';
    let chars = ['\"', '(', ')', '[', ']', '{', '}', '!', ' ', '_', '\''];
    for c in chars {
        info!("{} {}", c, c.is_ascii_punctuation());
    }
}

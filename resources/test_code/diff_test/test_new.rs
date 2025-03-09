use log::info;

pub fn main() {
    custom_utils::logger::logger_stdout_debug();
    // let _ = custom_utils::logger::logger_stdout(log::LevelFilter::Info).log_to_stdout();
    log::debug!("startss");
    log::info!("startss");
    log::warn!("startss");
    let a = 'a';
    let chars = ['\"', '(', ')', '[', ']', '{', '}', '!', ' ', '_', '\''];
    for c in chars {
        info!("{}-{}", c, c.is_ascii_punctuation());
        println!("startss");
        println!("startss");
    }
}

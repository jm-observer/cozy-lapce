#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use lapce_app::app;

#[tokio::main]
pub async fn main() {
    if let Err(err) = app::launch().await {
        eprintln!("launch fail: {}", err);
    }
}

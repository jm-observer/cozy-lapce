use lapce_proxy::mainloop;

#[tokio::main]
async fn main() {
    if let Err(err) = mainloop().await {
        eprintln!("{}", err);
    }
}

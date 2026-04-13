mod adb;
mod cli;
mod config;
mod logs;
mod mock;
mod traffic;

#[tokio::main]
async fn main() {
    cli::tui::run().await;
}

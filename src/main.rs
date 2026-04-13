mod adb;
mod cli;
mod config;
mod logs;
mod mock;
mod traffic;

#[tokio::main]
async fn main() {
    cli::shell::run().await;
}

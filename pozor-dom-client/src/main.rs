mod tui;
mod websocket;

use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <server_url>", args[0]);
        eprintln!("Example: {} ws://localhost:8081", args[0]);
        eprintln!("Example: {} ws://localhost:8082", args[0]);
        std::process::exit(1);
    }

    let server_url = args[1].clone();
    let app_state = Arc::new(Mutex::new(tui::AppState::new(server_url.clone())));

    websocket::connect_and_run(server_url, app_state).await
}

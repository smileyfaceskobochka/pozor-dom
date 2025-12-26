use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::sync::Arc;
use tokio::sync::Mutex;
use pozor_dom_shared::messages;
use crate::tui::AppState;

pub async fn connect_and_run(
    server_url: String,
    app_state: Arc<Mutex<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    match connect_async(&server_url).await {
        Ok((ws_stream, _)) => {
            {
                let mut state = app_state.lock().await;
                state.connected = true;
                state.add_message(format!("Connected to server: {}", server_url));
            }

            let (mut write, mut read) = ws_stream.split();

            // Send hello message to server
            let hello_msg = messages::create_welcome_message("Client");
            if let Err(e) = write.send(hello_msg).await {
                eprintln!("Failed to send hello message: {}", e);
                return Ok(());
            }

            // Clone app_state for the WebSocket reading task
            let app_state_ws = Arc::clone(&app_state);

            // Spawn WebSocket reading task
            tokio::spawn(async move {
                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            let mut state = app_state_ws.lock().await;
                            state.add_message(format!("Received: {}", text));
                        }
                        Ok(Message::Close(_)) => {
                            let mut state = app_state_ws.lock().await;
                            state.add_message("Connection closed by server".to_string());
                            state.connected = false;
                            break;
                        }
                        Ok(other) => {
                            let mut state = app_state_ws.lock().await;
                            state.add_message(format!("Received non-text message: {:?}", other));
                        }
                        Err(e) => {
                            let mut state = app_state_ws.lock().await;
                            state.add_message(format!("WebSocket error: {}", e));
                            state.connected = false;
                            break;
                        }
                    }
                }
            });

            // Run TUI with the write half
            if let Err(e) = crate::tui::run_tui(app_state, write).await {
                eprintln!("TUI error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            eprintln!("Make sure the server is running on {}", server_url);
        }
    }

    Ok(())
}

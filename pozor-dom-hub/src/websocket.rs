use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use std::sync::Arc;
use tokio::sync::broadcast;
use rumqttc::AsyncClient;
use serde_json::Value;


pub async fn start_websocket_server(
    broadcast_tx: Arc<broadcast::Sender<String>>,
    mqtt_client: Arc<AsyncClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:8082").await?;
    println!("✅ Hub WebSocket server listening on: 127.0.0.1:8082");

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("📱 New WebSocket connection from: {}", addr);
                let tx = Arc::clone(&broadcast_tx);
                let mqtt = Arc::clone(&mqtt_client);

                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, tx, mqtt).await {
                        eprintln!("Client handler error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("❌ Connection error: {}", e),
        }
    }
}

async fn handle_client(
    stream: TcpStream,
    broadcast_tx: Arc<broadcast::Sender<String>>,
    mqtt_client: Arc<AsyncClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match accept_async(stream).await {
        Ok(ws_stream) => {
            let (mut write, mut read) = ws_stream.split();
            let mut rx = broadcast_tx.subscribe();
            let client_id = format!("client-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis());

            println!("👤 Client connected: {}", client_id);

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                println!("📨 Received from {}: {}", client_id, text);

                                // Parse and handle commands
                                if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                    if json["type"].as_str() == Some("command") {
                                        if let Err(e) = crate::mqtt::send_device_command(&json, &mqtt_client).await {
                                            eprintln!("Failed to send device command: {}", e);
                                        }
                                    }
                                }

                                // Broadcast to all clients
                                let _ = broadcast_tx.send(text.to_string());
                            }
                            Some(Ok(Message::Close(_))) => {
                                println!("👤 Client closed: {}", client_id);
                                break;
                            }
                            Some(Err(e)) => {
                                eprintln!("❌ WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    msg = rx.recv() => {
                        if let Ok(text) = msg {
                            let ws_msg = Message::Text(text.into());
                            if write.send(ws_msg).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }

            println!("👤 Client disconnected: {}", client_id);
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ WebSocket error: {}", e);
            Err(e.into())
        }
    }
}

pub async fn connect_to_cloud(
    url: &str,
    broadcast_tx: Arc<broadcast::Sender<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        match tokio_tungstenite::connect_async(url).await {
            Ok((ws_stream, _)) => {
                println!("✅ Connected to Cloud: {}", url);

                let (mut write, mut read) = ws_stream.split();
                let mut rx = broadcast_tx.subscribe();

                loop {
                    tokio::select! {
                        msg = rx.recv() => {
                            if let Ok(text) = msg {
                                let ws_msg = tokio_tungstenite::tungstenite::Message::Text(text.into());
                                if write.send(ws_msg).await.is_err() {
                                    break;
                                }
                            }
                        }
                        msg = read.next() => {
                            match msg {
                                Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                    let _ = broadcast_tx.send(text.to_string());
                                }
                                Some(Err(_)) | None => break,
                                _ => {}
                            }
                        }
                    }
                }

                eprintln!("⚠️  Cloud connection closed");
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to Cloud: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

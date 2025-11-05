// Shared constants and utilities for Pozor-dom

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

// Ports
pub const CLOUD_PORT: u16 = 8081;
pub const HUB_PORT: u16 = 8082;

// Default hosts
pub const DEFAULT_CLOUD_HOST: &str = "127.0.0.1"; // Change to public IP for production
pub const DEFAULT_HUB_HOST: &str = "127.0.0.1";

// URL builders
pub fn cloud_url() -> String {
    format!("ws://{}:{}", DEFAULT_CLOUD_HOST, CLOUD_PORT)
}

pub fn cloud_url_with_host(host: &str) -> String {
    format!("ws://{}:{}", host, CLOUD_PORT)
}

pub fn hub_url() -> String {
    format!("ws://{}:{}", DEFAULT_HUB_HOST, HUB_PORT)
}

// Common types
pub type Tx = mpsc::UnboundedSender<tokio_tungstenite::tungstenite::protocol::Message>;
pub type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

// Message handling utilities
pub mod messages {
    use tokio_tungstenite::tungstenite::protocol::Message;

    pub const HUB_BROADCAST_PREFIX: &str = "HUB_BROADCAST:";
    pub const CLOUD_PREFIX: &str = "[CLOUD]";
    pub const HUB_PREFIX: &str = "[HUB]";
    pub const RECEIVED_SUFFIX: &str = "received:";

    pub fn is_hub_broadcast(msg: &str) -> bool {
        msg.starts_with(HUB_BROADCAST_PREFIX)
    }

    pub fn extract_hub_broadcast(msg: &str) -> &str {
        msg.strip_prefix(HUB_BROADCAST_PREFIX).unwrap_or(msg).trim()
    }

    pub fn create_hub_broadcast(content: &str) -> String {
        format!("{} {}", HUB_BROADCAST_PREFIX, content)
    }

    pub fn is_response_message(msg: &str) -> bool {
        msg.contains(RECEIVED_SUFFIX)
    }

    pub fn create_cloud_message(content: &str) -> Message {
        Message::Text(format!("{} {}", CLOUD_PREFIX, content).into())
    }

    pub fn create_hub_message(content: &str) -> Message {
        Message::Text(format!("{} {}", HUB_PREFIX, content).into())
    }

    pub fn create_welcome_message(component: &str) -> Message {
        Message::Text(format!("Welcome to Pozor-dom {}!", component).into())
    }

    pub fn create_echo_message(component: &str, content: &str) -> Message {
        Message::Text(format!("{} received: {}", component, content).into())
    }
}

// Connection management utilities
pub mod connection {
    use super::*;
    use std::io;

    pub fn create_peer_map() -> PeerMap {
        Arc::new(Mutex::new(HashMap::new()))
    }

    pub async fn setup_stdin_channel() -> Result<mpsc::UnboundedReceiver<String>, io::Error> {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let reader = tokio::io::BufReader::new(stdin);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    if tx.send(trimmed.to_string()).is_err() {
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}

// Configuration
pub mod config {
    use std::env;

    pub fn get_cloud_host() -> String {
        env::var("POZOR_DOM_CLOUD_HOST").unwrap_or(super::DEFAULT_CLOUD_HOST.to_string())
    }

    pub fn get_hub_host() -> String {
        env::var("POZOR_DOM_HUB_HOST").unwrap_or(super::DEFAULT_HUB_HOST.to_string())
    }

    pub fn get_cloud_port() -> u16 {
        env::var("POZOR_DOM_CLOUD_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(super::CLOUD_PORT)
    }

    pub fn get_hub_port() -> u16 {
        env::var("POZOR_DOM_HUB_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(super::HUB_PORT)
    }
}

// Logging utilities
pub mod logging {
    pub fn log_connection(addr: &std::net::SocketAddr, component: &str) {
        println!("New {} connection from: {}", component, addr);
    }

    pub fn log_disconnection(addr: &std::net::SocketAddr, component: &str) {
        println!("{} connection closed: {}", component, addr);
    }

    pub fn log_message_received(addr: &std::net::SocketAddr, msg: &str) {
        println!("Received from {}: {}", addr, msg);
    }

    pub fn log_broadcast(content: &str, source: &str) {
        println!("Broadcasting from {}: {}", source, content);
    }
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_cloud_url() {
        assert_eq!(cloud_url(), "ws://127.0.0.1:8081");
    }

    #[test]
    fn test_hub_url() {
        assert_eq!(hub_url(), "ws://127.0.0.1:8082");
    }

    #[test]
    fn test_hub_broadcast() {
        let msg = messages::create_hub_broadcast("test message");
        assert!(messages::is_hub_broadcast(&msg));
        assert_eq!(messages::extract_hub_broadcast(&msg), "test message");
    }
}

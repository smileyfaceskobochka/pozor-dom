use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use pozor_dom_shared::{config, connection, logging, messages, PeerMap, Tx};

#[tokio::main]
async fn main() {
    println!("Pozor-dom Cloud starting...");
    println!("Type messages to broadcast to all connected clients. Press Ctrl+C to exit.");

    let host = config::get_cloud_host();
    let port = config::get_cloud_port();
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Cloud WebSocket server listening on: {}", addr);
    println!("Note: This is using plain WebSocket (ws://), not WSS yet.");
    println!("For production WSS, you'll need to add TLS certificates.");

    let peer_map = connection::create_peer_map();
    let mut rx = connection::setup_stdin_channel().await.expect("Failed to setup stdin channel");

    // Main loop to handle both new connections and stdin input
    loop {
        tokio::select! {
            // Handle new connections
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let peer_map = peer_map.clone();
                        tokio::spawn(async move {
                            println!("New connection from: {}", addr);
                            handle_connection(peer_map, stream, addr).await;
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }

            // Handle stdin input
            input = rx.recv() => {
                match input {
                    Some(text) => {
                        logging::log_broadcast(&text, "Cloud");
                        // Broadcast the message to all connected peers
                        let peers = peer_map.lock().unwrap().clone();
                        for (addr, tx) in peers {
                            if let Err(e) = tx.send(messages::create_cloud_message(&text)) {
                                eprintln!("Failed to send to {}: {}", addr, e);
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

async fn handle_connection(peer_map: PeerMap, raw_stream: tokio::net::TcpStream, addr: SocketAddr) {
    match accept_async(raw_stream).await {
        Ok(ws_stream) => {
            logging::log_connection(&addr, "Cloud");

            // Create channels for this peer
            let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
            peer_map.lock().unwrap().insert(addr, tx);

            let (mut write, mut read) = ws_stream.split();

            // Send welcome message
            let component = if addr.ip().is_loopback() { "Hub" } else { "external client" };
            let welcome_msg = messages::create_welcome_message(component);

            if let Err(e) = write.send(welcome_msg).await {
                eprintln!("Failed to send welcome message: {}", e);
                peer_map.lock().unwrap().remove(&addr);
                return;
            }

            // Handle incoming messages
            loop {
                tokio::select! {
                    // Handle incoming WebSocket messages
                    message = read.next() => {
                        match message {
                            Some(Ok(Message::Text(text))) => {
                                logging::log_message_received(&addr, &text);

                                // Check if this is a hub broadcast message
                                if messages::is_hub_broadcast(&text) {
                                    // Extract the actual message
                                    let broadcast_content = messages::extract_hub_broadcast(&text);

                                    // Broadcast to ALL clients (including the hub)
                                    let all_peers = peer_map.lock().unwrap().clone();
                                    let broadcast_msg = messages::create_hub_message(broadcast_content);
                                    for (peer_addr, tx) in all_peers {
                                        if let Err(e) = tx.send(broadcast_msg.clone()) {
                                            eprintln!("Failed to send hub broadcast to {}: {}", peer_addr, e);
                                        }
                                    }
                                } else {
                                    // Normal message handling
                                    // Collect broadcast recipients (release lock immediately)
                                    let broadcast_recipients: Vec<Tx> = {
                                        let peers = peer_map.lock().unwrap();
                                        peers.iter()
                                            .filter(|(peer_addr, _)| *peer_addr != &addr)
                                            .map(|(_, tx)| tx.clone())
                                            .collect()
                                    };

                                    // Broadcast to all other peers
                                    let broadcast_msg = format!("[{}] {}", addr, text);
                                    for recipient in broadcast_recipients {
                                        if let Err(e) = recipient.send(Message::Text(broadcast_msg.clone().into())) {
                                            eprintln!("Failed to send to peer: {}", e);
                                        }
                                    }

                                    // Only echo back if this is not a response message (to prevent infinite loop)
                                    if !messages::is_response_message(&text) {
                                        // Echo back to sender
                                        let echo_msg = messages::create_echo_message("Cloud", &text);
                                        if let Err(e) = write.send(echo_msg).await {
                                            eprintln!("Failed to send response: {}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                println!("Connection closed by: {}", addr);
                                break;
                            }
                            Some(Ok(other)) => {
                                println!("Received non-text message from {}: {:?}", addr, other);
                            }
                            Some(Err(e)) => {
                                eprintln!("Error from {}: {}", addr, e);
                                break;
                            }
                            None => break,
                        }
                    }

                    // Handle messages to send to this peer
                    message = rx.recv() => {
                        match message {
                            Some(msg) => {
                                if let Err(e) = write.send(msg).await {
                                    eprintln!("Failed to send message to {}: {}", addr, e);
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }

            // Clean up
            peer_map.lock().unwrap().remove(&addr);
            println!("Connection with {} closed", addr);
        }
        Err(e) => {
            eprintln!("Failed to accept WebSocket connection from {}: {}", addr, e);
        }
    }
}

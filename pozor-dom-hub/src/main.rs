use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::protocol::Message};
use std::net::SocketAddr;
use tokio::sync::mpsc;
use pozor_dom_shared::{cloud_url_with_host, config, connection, logging, messages, PeerMap, Tx};
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let cloud_host = if args.len() > 1 {
        args[1].clone()
    } else {
        config::get_cloud_host()
    };

    println!("Pozor-dom Hub starting...");
    println!("Local network server + Cloud relay. Type messages to broadcast to all clients (local + cloud). Press Ctrl+C to exit.");
    println!("Usage: {} [cloud_host] (default: {})", args[0], config::get_cloud_host());

    let addr = format!("127.0.0.1:{}", pozor_dom_shared::HUB_PORT);
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Hub WebSocket server listening on: {}", addr);
    println!("Note: This is using plain WebSocket (ws://), not WSS yet.");
    println!("For production WSS, you'll need to add TLS certificates.");

    let peer_map = connection::create_peer_map();

    // Connect to Cloud server for relaying messages
    let cloud_url = cloud_url_with_host(&cloud_host);
    println!("Connecting to Cloud at: {} for message relaying...", cloud_url);

    let mut rx = connection::setup_stdin_channel().await.expect("Failed to setup stdin channel");

    // Channel for forwarding cloud messages to local clients
    let (cloud_msg_tx, mut cloud_msg_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    // Channel for sending messages to cloud
    let (cloud_send_tx, mut cloud_send_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Connect to Cloud server for relaying messages
    let cloud_connection_result = connect_async(cloud_url.clone()).await;

    // Handle cloud connection messages
    match cloud_connection_result {
        Ok((mut cloud_stream, _)) => {
            println!("Connected to Cloud successfully for relaying!");
            let cloud_msg_tx_clone = cloud_msg_tx.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        // Receive messages from cloud
                        message = cloud_stream.next() => {
                            match message {
                                Some(Ok(Message::Text(text))) => {
                                    // Log messages received from cloud
                                    println!("Received from Cloud: {}", text);

                                    // Forward hub broadcasts to local clients
                                    if messages::is_hub_broadcast(&text) {
                                        let broadcast_content = messages::extract_hub_broadcast(&text);
                                        let hub_msg = messages::create_hub_message(broadcast_content);
                                        if let Err(e) = cloud_msg_tx_clone.send(hub_msg) {
                                            eprintln!("Failed to forward hub broadcast to local clients: {}", e);
                                        }
                                    }
                                }
                                Some(Ok(Message::Close(_))) => {
                                    println!("Cloud connection closed");
                                    break;
                                }
                                Some(Ok(other)) => {
                                    println!("Received non-text message from Cloud: {:?}", other);
                                }
                                Some(Err(e)) => {
                                    eprintln!("Error receiving from cloud: {}", e);
                                    break;
                                }
                                None => break,
                            }
                        }

                        // Send messages to cloud
                        msg_to_send = cloud_send_rx.recv() => {
                            if let Some(msg) = msg_to_send {
                                if let Err(e) = cloud_stream.send(Message::Text(msg.into())).await {
                                    eprintln!("Failed to send to cloud: {}", e);
                                }
                            } else {
                                break;
                            }
                        }
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("Failed to connect to Cloud: {}. Hub will work in local-only mode.", e);
        }
    }

    // Main loop to handle new connections, cloud messages, and stdin input
    loop {
        tokio::select! {
            // Handle new local connections
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let peer_map = peer_map.clone();
                        tokio::spawn(async move {
                            println!("New local connection from: {}", addr);
                            handle_local_connection(peer_map, stream, addr).await;
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept local connection: {}", e);
                    }
                }
            }

            // Handle cloud messages
            cloud_msg = cloud_msg_rx.recv() => {
                if let Some(msg) = cloud_msg {
                    // Broadcast hub messages from cloud to local clients
                    let peers = peer_map.lock().unwrap().clone();
                    for (addr, tx) in peers {
                        if let Err(e) = tx.send(msg.clone()) {
                            eprintln!("Failed to send cloud message to local client {}: {}", addr, e);
                        }
                    }
                }
            }

            // Handle stdin input
            input = rx.recv() => {
                match input {
                    Some(text) => {
                        logging::log_broadcast(&text, "Hub");

                        // Broadcast to local clients
                        let peers = peer_map.lock().unwrap().clone();
                        for (addr, tx) in peers {
                            if let Err(e) = tx.send(messages::create_hub_message(&text)) {
                                eprintln!("Failed to send to local client {}: {}", addr, e);
                            }
                        }

                        // Send to cloud for relaying to cloud clients
                        let relay_msg = messages::create_hub_broadcast(&text);
                        if let Err(e) = cloud_send_tx.send(relay_msg) {
                            eprintln!("Failed to send to cloud: {}", e);
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

async fn handle_local_connection(peer_map: PeerMap, raw_stream: tokio::net::TcpStream, addr: SocketAddr) {
    match accept_async(raw_stream).await {
        Ok(ws_stream) => {
            logging::log_connection(&addr, "Hub");

            // Create channels for this peer
            let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
            peer_map.lock().unwrap().insert(addr, tx);

            let (mut write, mut read) = ws_stream.split();

            // Send welcome message
            let welcome_msg = messages::create_welcome_message("Hub");
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
                                    let echo_msg = messages::create_echo_message("Hub", &text);
                                    if let Err(e) = write.send(echo_msg).await {
                                        eprintln!("Failed to send response: {}", e);
                                        break;
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

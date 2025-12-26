use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use warp::Filter;
use pozor_dom_shared::{config, connection, logging, messages, dashboard, PeerMap, Tx};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Pozор-дом Cloud starting...");
    println!("WebSocket relay + Web Dashboard");
    println!("Press Ctrl+C to exit.\n");

    let host = config::get_cloud_host();
    let port = config::get_cloud_port();
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;
    println!("✅ Cloud WebSocket server listening on: {}", addr);
    println!("🌐 Cloud dashboard: http://localhost:8080\n");
    println!("Note: This is using plain WebSocket (ws://), not WSS yet.");
    println!("For production WSS, you'll need to add TLS certificates.");

    let peer_map = connection::create_peer_map();
    let cloud_state = Arc::new(Mutex::new(dashboard::HubState::new("Cloud")));
    let mut rx = connection::setup_stdin_channel().await?;

    // Start web dashboard server (proxying API calls to hub)
    let cloud_state_web = Arc::clone(&cloud_state);
    tokio::spawn(async move {
        if let Err(e) = start_cloud_web_server(cloud_state_web).await {
            eprintln!("Cloud web server error: {}", e);
        }
    });

    // Main loop to handle both new connections and stdin input
    loop {
        tokio::select! {
            // Handle new connections
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let peer_map = peer_map.clone();
                        let cloud_state = Arc::clone(&cloud_state);
                        tokio::spawn(async move {
                            println!("New connection from: {}", addr);
                            handle_connection(peer_map, stream, addr, cloud_state).await;
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

    Ok(())
}

async fn handle_connection(peer_map: PeerMap, raw_stream: tokio::net::TcpStream, addr: SocketAddr, cloud_state: Arc<Mutex<dashboard::HubState>>) {
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

                                    // Try to parse as device telemetry and update cloud state
                                    match serde_json::from_str::<pozor_dom_shared::dashboard::lib::DeviceTelemetry>(broadcast_content) {
                                        Ok(telemetry) => {
                                            println!("✅ Cloud parsed telemetry for device: {}", telemetry.device_id);
                                            let mut state = cloud_state.lock().await;
                                            state.update_device(telemetry);
                                        }
                                        Err(_) => {
                                            // Not device telemetry, broadcast to all clients
                                            let all_peers = peer_map.lock().unwrap().clone();
                                            let broadcast_msg = messages::create_hub_message(broadcast_content);
                                            for (peer_addr, tx) in all_peers {
                                                if let Err(e) = tx.send(broadcast_msg.clone()) {
                                                    eprintln!("Failed to send hub broadcast to {}: {}", peer_addr, e);
                                                }
                                            }
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

async fn start_cloud_web_server(cloud_state: Arc<Mutex<dashboard::HubState>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Serve Yew-based dashboard
    let dashboard = warp::path::end()
        .map(move || {
            let html_content = include_str!("../../pozor-dom-shared/src/dashboard/yew_index.html");
            warp::reply::html(html_content)
        });

    // Cloud state filter for API endpoints
    let cloud_state_filter = warp::any().map(move || Arc::clone(&cloud_state));

    // Cloud-specific API endpoints
    let api_toggle_cloud = warp::path!("api" / "toggle-cloud")
        .and(warp::post())
        .and(cloud_state_filter.clone())
        .and_then(toggle_cloud);

    // Proxy other API requests to hub
    let api_proxy = warp::path("api")
        .and(warp::path::full())
        .and(warp::method())
        .and(warp::body::bytes())
        .and_then(proxy_to_hub);

    // Serve WebAssembly files (embedded)
    let wasm_js_content = include_bytes!("../../pozor-dom-shared/pkg/pozor_dom_shared.js");
    let wasm_js = warp::path("pozor_dom_shared.js")
        .map(move || {
            warp::http::Response::builder()
                .header("content-type", "application/javascript")
                .body(wasm_js_content.to_vec())
                .unwrap()
        });

    let wasm_binary_content = include_bytes!("../../pozor-dom-shared/pkg/pozor_dom_shared_bg.wasm");
    let wasm_binary = warp::path("pozor_dom_shared_bg.wasm")
        .map(move || {
            warp::http::Response::builder()
                .header("content-type", "application/wasm")
                .body(wasm_binary_content.to_vec())
                .unwrap()
        });

    let routes = dashboard
        .or(api_toggle_cloud)
        .or(api_proxy)
        .or(wasm_js)
        .or(wasm_binary)
        .with(warp::cors().allow_any_origin());

    println!("🌐 Cloud web dashboard available at: http://localhost:8080");
    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;

    Ok(())
}

async fn proxy_to_hub(
    path: warp::path::FullPath,
    method: warp::http::Method,
    body: bytes::Bytes,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    // Proxy to hub at localhost:3000
    let hub_url = format!("http://localhost:3000{}", path.as_str());

    let client = reqwest::Client::new();
    let mut request = match method {
        warp::http::Method::GET => client.get(&hub_url),
        warp::http::Method::POST => client.post(&hub_url),
        warp::http::Method::PUT => client.put(&hub_url),
        warp::http::Method::DELETE => client.delete(&hub_url),
        _ => return Err(warp::reject::not_found()),
    };

    if !body.is_empty() {
        request = request.body(body);
    }

    match request.send().await {
        Ok(response) => {
            let status_code = response.status().as_u16();
            let is_json = response.headers().get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.contains("application/json"))
                .unwrap_or(false);
            let body_bytes = response.bytes().await.unwrap_or_default();

            // Return the response body with appropriate status
            let reply = warp::reply::with_status(body_bytes.to_vec(), warp::http::StatusCode::from_u16(status_code).unwrap_or(warp::http::StatusCode::OK));

            // Add content-type header if it's JSON
            if is_json {
                Ok(Box::new(warp::reply::with_header(reply, "content-type", "application/json")))
            } else {
                Ok(Box::new(reply))
            }
        }
        Err(e) => {
            eprintln!("Failed to proxy request to hub: {}", e);
            Err(warp::reject::not_found())
        }
    }
}

async fn toggle_cloud(
    cloud_state: Arc<Mutex<dashboard::HubState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut state = cloud_state.lock().await;
    state.toggle_cloud();
    Ok(warp::reply::json(&serde_json::json!({
        "cloud_enabled": state.cloud_enabled,
        "message": format!("Cloud {} for {}", if state.cloud_enabled { "enabled" } else { "disabled" }, state.service_name)
    })))
}

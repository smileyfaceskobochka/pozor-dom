mod mqtt;
mod websocket;
mod database;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, mpsc};
use tokio::task::JoinHandle;
use pozor_dom_shared::dashboard;
use warp::Filter;

#[derive(Debug)]
enum CloudCommand {
    Connect,
    Disconnect,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Позор-дом Hub starting...");
    println!("Local network server + Cloud relay + MQTT bridge + Web Dashboard");
    println!("Press Ctrl+C to exit.\n");

    let args: Vec<String> = std::env::args().collect();
    let cloud_host = if args.len() > 1 {
        args[1].as_str()
    } else {
        "127.0.0.1"
    };

    // Initialize SQLite database
    let db = Arc::new(database::Database::new("pozor_dom_hub.db")?);
    println!("💾 Database initialized: pozor_dom_hub.db");

    // Load cloud enabled state from database
    let cloud_enabled = db.get_cloud_enabled().unwrap_or(true);
    println!("🌐 Cloud connectivity: {}", if cloud_enabled { "enabled" } else { "disabled" });

    // Broadcast channel for messages
    let (tx, _rx) = tokio::sync::broadcast::channel::<String>(100);
    let tx = Arc::new(tx);

    // Hub state for web dashboard (load initial devices from database)
    let initial_devices = db.load_devices().unwrap_or_default();
    let mut hub_state = dashboard::HubState::new("Hub");
    hub_state.cloud_enabled = cloud_enabled; // Set initial cloud state
    for (_device_id, telemetry) in initial_devices {
        hub_state.update_device(telemetry);
    }
    let hub_state = Arc::new(Mutex::new(hub_state));

    // Channel for controlling cloud connection
    let (cloud_tx, cloud_rx) = mpsc::unbounded_channel::<CloudCommand>();

    // Setup MQTT client
    let (mqtt_client, eventloop) = mqtt::setup_mqtt_client().await;
    let mqtt_client = Arc::new(mqtt_client);

    // Clone for telemetry processing
    let hub_state_mqtt = Arc::clone(&hub_state);
    let tx_mqtt = Arc::clone(&tx);
    let db_mqtt = Arc::clone(&db);

    // Spawn MQTT listener for telemetry
    tokio::spawn(async move {
        listen_and_process_telemetry(eventloop, tx_mqtt, hub_state_mqtt, db_mqtt).await;
    });

    // Spawn cloud connection manager
    let tx_cloud_manager = Arc::clone(&tx);
    let cloud_url = format!("ws://{}:{}", cloud_host, 8081);
    let cloud_url_clone = cloud_url.clone();
    tokio::spawn(async move {
        manage_cloud_connection(cloud_rx, cloud_url, tx_cloud_manager).await;
    });

    // If cloud should be enabled initially, send connect command
    if cloud_enabled {
        let _ = cloud_tx.send(CloudCommand::Connect);
        println!("🌐 Cloud relay enabled: {}", cloud_url_clone);
    } else {
        println!("🌐 Cloud relay disabled - local network only");
    }

    // Start WebSocket server
    let tx_ws = Arc::clone(&tx);
    let mqtt_ws = Arc::clone(&mqtt_client);
    tokio::spawn(async move {
        if let Err(e) = websocket::start_websocket_server(tx_ws, mqtt_ws).await {
            eprintln!("WebSocket server error: {}", e);
        }
    });

    // Start web dashboard server with database support
    let hub_state_web = Arc::clone(&hub_state);
    let db_web = Arc::clone(&db);
    let cloud_tx_web = cloud_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = start_web_server_with_db(hub_state_web, db_web, cloud_tx_web, 3000).await {
            eprintln!("Web server error: {}", e);
        }
    });

    println!("🔌 MQTT broker: 127.0.0.1:1883");
    println!("🌐 Web dashboard: http://localhost:3000\n");

    // Keep main thread alive
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Hub shutting down...");

    Ok(())
}

async fn listen_and_process_telemetry(
    eventloop: rumqttc::EventLoop,
    broadcast_tx: Arc<broadcast::Sender<String>>,
    hub_state: Arc<Mutex<dashboard::HubState>>,
    db: Arc<database::Database>,
) {
    use rumqttc::{Event, Incoming};

    let mut eventloop = eventloop;

    loop {
        match eventloop.poll().await {
            Ok(notification) => {
                match notification {
                    Event::Incoming(Incoming::Publish(publish)) => {
                        if let Ok(payload) = std::str::from_utf8(&publish.payload) {
                            println!("📡 Received MQTT telemetry on {}: {}", publish.topic, payload);

                            // Try to parse as device telemetry and update hub state
                            match serde_json::from_str::<pozor_dom_shared::dashboard::lib::DeviceTelemetry>(payload) {
                                Ok(telemetry) => {
                                    println!("✅ Successfully parsed telemetry for device: {}", telemetry.device_id);

                                    // Save to database (blocking operation within async context)
                                    let db_clone = Arc::clone(&db);
                                    let telemetry_clone = telemetry.clone();
                                    tokio::task::block_in_place(|| {
                                        if let Err(e) = db_clone.save_device(&telemetry_clone) {
                                            eprintln!("❌ Failed to save device to database: {}", e);
                                        }
                                    });

                                    // Update in-memory state
                                    let mut state = hub_state.lock().await;
                                    state.update_device(telemetry);
                                }
                                Err(e) => {
                                    println!("❌ Failed to parse telemetry payload: {} (error: {})", payload, e);
                                }
                            }

                            // Broadcast telemetry to all WebSocket clients
                            let _ = broadcast_tx.send(payload.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("❌ MQTT telemetry listener error: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn manage_cloud_connection(
    mut rx: mpsc::UnboundedReceiver<CloudCommand>,
    cloud_url: String,
    broadcast_tx: Arc<broadcast::Sender<String>>,
) {
    let mut current_task: Option<JoinHandle<()>> = None;

    while let Some(command) = rx.recv().await {
        match command {
            CloudCommand::Connect => {
                // If already connected, do nothing
                if current_task.is_some() {
                    continue;
                }

                println!("🌐 Connecting to cloud: {}", cloud_url);
                let tx_clone = Arc::clone(&broadcast_tx);
                let url_clone = cloud_url.clone();

                let task = tokio::spawn(async move {
                    if let Err(e) = websocket::connect_to_cloud(&url_clone, tx_clone).await {
                        eprintln!("Cloud connection error: {}", e);
                    }
                });

                current_task = Some(task);
            }
            CloudCommand::Disconnect => {
                // If connected, abort the task
                if let Some(task) = current_task.take() {
                    task.abort();
                    println!("🌐 Disconnected from cloud");
                }
            }
        }
    }
}

async fn start_web_server_with_db(
    hub_state: Arc<Mutex<dashboard::HubState>>,
    db: Arc<database::Database>,
    cloud_tx: mpsc::UnboundedSender<CloudCommand>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let hub_state_filter = warp::any().map(move || Arc::clone(&hub_state));
    let db_for_toggle = Arc::clone(&db);
    let db_filter = warp::any().map(move || Arc::clone(&db));

    // Serve Yew-based dashboard instead of static HTML
    let dashboard = warp::path::end()
        .map(move || {
            let html_content = include_str!("../../pozor-dom-shared/src/dashboard/yew_index.html");
            warp::reply::html(html_content)
        });

    // API endpoints that use database (synchronous)
    let api_devices = warp::path!("api" / "devices")
        .and(db_filter.clone())
        .map(|db: Arc<database::Database>| {
            // Hub always has access to its database
            match db.load_devices() {
                Ok(devices) => {
                    let device_list: Vec<&pozor_dom_shared::dashboard::lib::DeviceTelemetry> = devices.values().collect();
                    warp::reply::with_status(warp::reply::json(&device_list), warp::http::StatusCode::OK)
                }
                Err(e) => {
                    eprintln!("Database error loading devices: {}", e);
                    // Return empty array on error
                    warp::reply::with_status(warp::reply::json(&Vec::<&pozor_dom_shared::dashboard::lib::DeviceTelemetry>::new()), warp::http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        });

    let api_messages = warp::path!("api" / "messages")
        .and(hub_state_filter.clone())
        .and_then(get_messages);

    let cloud_tx_filter = warp::any().map(move || cloud_tx.clone());
    let db_filter_for_toggle = warp::any().map(move || Arc::clone(&db_for_toggle));

    let api_toggle_cloud = warp::path!("api" / "toggle-cloud")
        .and(warp::post())
        .and(hub_state_filter.clone())
        .and(cloud_tx_filter)
        .and(db_filter_for_toggle)
        .and_then(toggle_cloud);



    let routes = dashboard
        .or(api_devices)
        .or(api_messages)
        .or(api_toggle_cloud)
        .with(warp::cors().allow_any_origin());

    println!("🌐 Web dashboard available at: http://localhost:{}", port);
    warp::serve(routes).run(([127, 0, 0, 1], port)).await;

    Ok(())
}



async fn get_messages_protected(
    hub_state: Arc<Mutex<dashboard::HubState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = hub_state.lock().await;

    // Check if cloud access is allowed
    if !state.cloud_enabled {
        // Cloud is disabled, return forbidden
        return Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({
                "error": "Cloud access disabled",
                "message": "Cloud connectivity is currently disabled on this hub"
            })),
            warp::http::StatusCode::FORBIDDEN
        ));
    }

    Ok(warp::reply::with_status(warp::reply::json(&state.messages), warp::http::StatusCode::OK))
}

async fn get_messages(
    hub_state: Arc<Mutex<dashboard::HubState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = hub_state.lock().await;
    Ok(warp::reply::json(&state.messages))
}

async fn toggle_cloud(
    hub_state: Arc<Mutex<dashboard::HubState>>,
    cloud_tx: mpsc::UnboundedSender<CloudCommand>,
    db: Arc<database::Database>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut state = hub_state.lock().await;
    let was_enabled = state.cloud_enabled;
    state.toggle_cloud();
    let is_now_enabled = state.cloud_enabled;

    // Persist the new state to database
    let db_clone = Arc::clone(&db);
    let enabled_value = is_now_enabled;
    tokio::task::block_in_place(|| {
        if let Err(e) = db_clone.set_cloud_enabled(enabled_value) {
            eprintln!("Failed to persist cloud enabled state: {}", e);
        }
    });

    // Control the cloud connection
    if !was_enabled && is_now_enabled {
        // Was disabled, now enabled - connect
        let _ = cloud_tx.send(CloudCommand::Connect);
    } else if was_enabled && !is_now_enabled {
        // Was enabled, now disabled - disconnect
        let _ = cloud_tx.send(CloudCommand::Disconnect);
    }

    Ok(warp::reply::json(&serde_json::json!({
        "cloud_enabled": is_now_enabled,
        "message": format!("Cloud {} for {}", if is_now_enabled { "enabled" } else { "disabled" }, state.service_name)
    })))
}

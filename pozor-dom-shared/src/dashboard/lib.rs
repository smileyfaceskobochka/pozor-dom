#[cfg(feature = "server")]
use std::collections::HashMap;
#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use tokio::sync::Mutex;
#[cfg(feature = "server")]
use warp::Filter;
#[cfg(feature = "server")]
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceTelemetry {
    pub device_id: String,
    pub channel: String,
    pub temperature: String,
    pub humidity: String,
    pub signal_strength: i32,
    pub timestamp: String,
}

#[cfg(feature = "server")]
#[derive(Clone)]
pub struct HubState {
    pub devices: HashMap<String, DeviceTelemetry>,
    pub messages: Vec<String>,
    pub cloud_enabled: bool,
    pub service_name: String,
}

#[cfg(feature = "server")]
impl HubState {
    pub fn new(service_name: &str) -> Self {
        Self {
            devices: HashMap::new(),
            messages: Vec::new(),
            cloud_enabled: false,
            service_name: service_name.to_string(),
        }
    }

    pub fn update_device(&mut self, telemetry: DeviceTelemetry) {
        self.devices.insert(telemetry.device_id.clone(), telemetry);
    }

    pub fn add_message(&mut self, message: String) {
        self.messages.push(message);
        // Keep only last 100 messages
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    pub fn toggle_cloud(&mut self) {
        self.cloud_enabled = !self.cloud_enabled;
    }
}

#[cfg(feature = "server")]
pub async fn start_web_server(
    hub_state: Arc<Mutex<HubState>>,
    port: u16,
    static_dir: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let hub_state_filter = warp::any().map(move || Arc::clone(&hub_state));

    // Serve Yew-based dashboard instead of static HTML
    let dashboard = warp::path::end()
        .map(move || {
            let html_content = include_str!("yew_index.html");
            warp::reply::html(html_content)
        });

    let api_devices = warp::path!("api" / "devices")
        .and(hub_state_filter.clone())
        .and_then(get_devices);

    let api_messages = warp::path!("api" / "messages")
        .and(hub_state_filter.clone())
        .and_then(get_messages);

    let api_toggle_cloud = warp::path!("api" / "toggle-cloud")
        .and(warp::post())
        .and(hub_state_filter.clone())
        .and_then(toggle_cloud);

    let static_files = warp::path("static")
        .and(warp::fs::dir(static_dir));

    // Serve WebAssembly files
    let wasm_js = warp::path("pozor_dom_shared.js")
        .and(warp::fs::file("./pozor-dom-shared/pkg/pozor_dom_shared.js"))
        .with(warp::reply::with::header("content-type", "application/javascript"));

    let wasm_binary = warp::path("pozor_dom_shared_bg.wasm")
        .and(warp::fs::file("./pozor-dom-shared/pkg/pozor_dom_shared_bg.wasm"))
        .with(warp::reply::with::header("content-type", "application/wasm"));

    let routes = dashboard
        .or(api_devices)
        .or(api_messages)
        .or(api_toggle_cloud)
        .or(wasm_js)
        .or(wasm_binary)
        .or(static_files)
        .with(warp::cors().allow_any_origin());

    println!("🌐 Web dashboard available at: http://localhost:{}", port);
    warp::serve(routes).run(([127, 0, 0, 1], port)).await;

    Ok(())
}

#[cfg(feature = "server")]
async fn get_devices(
    hub_state: Arc<Mutex<HubState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = hub_state.lock().await;
    let devices: Vec<&DeviceTelemetry> = state.devices.values().collect();
    Ok(warp::reply::json(&devices))
}

#[cfg(feature = "server")]
async fn get_messages(
    hub_state: Arc<Mutex<HubState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let state = hub_state.lock().await;
    Ok(warp::reply::json(&state.messages))
}

#[cfg(feature = "server")]
async fn toggle_cloud(
    hub_state: Arc<Mutex<HubState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let mut state = hub_state.lock().await;
    state.toggle_cloud();
    Ok(warp::reply::json(&serde_json::json!({
        "cloud_enabled": state.cloud_enabled,
        "message": format!("Cloud {} for {}", if state.cloud_enabled { "enabled" } else { "disabled" }, state.service_name)
    })))
}

#[cfg(any(feature = "server", feature = "wasm"))]
use yew::prelude::*;
#[cfg(any(feature = "server", feature = "wasm"))]
use web_sys::WebSocket;
#[cfg(any(feature = "server", feature = "wasm"))]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(any(feature = "server", feature = "wasm"))]
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "server", feature = "wasm"))]
use std::collections::HashMap;
#[cfg(any(feature = "server", feature = "wasm"))]
use crate::dashboard::yew_components::{Dashboard, DashboardContext, ServiceType, DashboardProps};
#[cfg(any(feature = "server", feature = "wasm"))]
use crate::dashboard::DeviceTelemetry;

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub devices: Vec<DeviceTelemetry>,
    pub messages: Vec<String>,
    pub cloud_enabled: bool,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(YewDashboardApp)]
pub fn yew_dashboard_app() -> Html {
    let devices = use_state(|| HashMap::<String, DeviceTelemetry>::new());
    let messages = use_state(|| Vec::<String>::new());
    let cloud_enabled = use_state(|| false);
    let service_type = use_state(|| ServiceType::Hub);
    let version = use_state(|| 0);
    let ws = use_state(|| None::<WebSocket>);
    let ws_clone = ws.clone();

    // Determine service type from URL or default to Hub
    {
        let service_type = service_type.clone();
        use_effect_with((), move |_| {
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let pathname = location.pathname().expect("should have pathname");

            if pathname.contains("cloud") {
                service_type.set(ServiceType::Cloud);
            } else {
                service_type.set(ServiceType::Hub);
            }
        });
    }

    // Load initial data
    {
        let devices = devices.clone();
        let messages = messages.clone();
        let cloud_enabled = cloud_enabled.clone();
        let service_type = service_type.clone();
        let version = version.clone();

        use_effect_with((), move |_| {
            let devices = devices.clone();
            let messages = messages.clone();
            let cloud_enabled = cloud_enabled.clone();
            let service_type = service_type.clone();
            let version = version.clone();

            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = fetch_data().await {
                    devices.set(response.devices.into_iter().map(|d| (d.device_id.clone(), d)).collect());
                    messages.set(response.messages);
                    // Set cloud_enabled based on service type for initial state
                    let initial_cloud_enabled = matches!(*service_type, ServiceType::Cloud);
                    cloud_enabled.set(initial_cloud_enabled);
                    version.set(*version + 1);
                }
            });
        });
    }

    // Set up WebSocket connection
    {
        let devices = devices.clone();
        let messages = messages.clone();
        let version = version.clone();
        let service_type_clone = (*service_type).clone();

        use_effect_with(service_type_clone.clone(), move |_| {
            let devices = devices.clone();
            let messages = messages.clone();
            let version = version.clone();
            let ws_state = ws.clone();

            // Determine WebSocket URL based on service type
            let ws_url = match service_type_clone {
                ServiceType::Hub => "ws://localhost:8082",
                ServiceType::Cloud => "ws://localhost:8081",
            };

            if let Ok(websocket) = WebSocket::new(ws_url) {
                websocket.set_binary_type(web_sys::BinaryType::Arraybuffer);

                // Set up message handler
                let onmessage = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                    if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                        let message = text.as_string().unwrap_or_default();

                        // Skip echo messages that start with component names
                        if message.starts_with("Cloud received:") ||
                           message.starts_with("Hub received:") ||
                           message.starts_with("[CLOUD]") ||
                           message.starts_with("[HUB]") ||
                           message == "Welcome to Pozor-dom Hub!" ||
                           message == "Welcome to Pozor-dom Cloud!" {
                            // Still add to messages list for display
                            messages.set({
                                let mut msgs = (*messages).clone();
                                msgs.push(message.clone());
                                if msgs.len() > 100 {
                                    msgs.remove(0);
                                }
                                msgs
                            });
                            return;
                        }

                        // Try to parse as device telemetry
                        if let Ok(_telemetry) = serde_json::from_str::<DeviceTelemetry>(&message) {
                            // Refetch all devices from API to ensure we have the latest state
                            let devices_clone = devices.clone();
                            let version_clone = version.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Ok(response) = fetch_devices().await {
                                    devices_clone.set(response.into_iter().map(|d| (d.device_id.clone(), d)).collect());
                                    version_clone.set(*version_clone + 1);
                                }
                            });
                        }

                        // Add to messages list
                        messages.set({
                            let mut msgs = (*messages).clone();
                            msgs.push(message.clone());
                            if msgs.len() > 100 {
                                msgs.remove(0);
                            }
                            msgs
                        });
                    }
                });

                websocket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                onmessage.forget();

                // Set up close handler for reconnection
                let onclose = Closure::<dyn FnMut()>::new(move || {
                    gloo::timers::callback::Timeout::new(5000, move || {
                        // This will trigger a reconnection when the effect runs again
                    }).forget();
                });

                websocket.set_onclose(Some(onclose.as_ref().unchecked_ref()));

                // Set up error handler
                let onerror = Closure::<dyn FnMut()>::new(move || {
                    // Handle WebSocket errors
                });

                websocket.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                onclose.forget();
                onerror.forget();

                // Store WebSocket in state
                ws_state.set(Some(websocket));
            }
        });
    }

    // Set up periodic data refresh (only for messages, don't override WebSocket device updates)
    {
        let messages = messages.clone();

        use_effect_with((), move |_| {
            let messages = messages.clone();

            let _interval = gloo::timers::callback::Interval::new(5000, move || {
                let messages = messages.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    // Only update messages, don't touch devices (they're updated via WebSocket)
                    if let Ok(response) = fetch_messages().await {
                        if messages.is_empty() {
                            messages.set(response);
                        }
                    }
                });
            });
        });
    }

    let on_toggle_cloud = {
        let cloud_enabled = cloud_enabled.clone();
        let version = version.clone();
        Callback::from(move |_| {
            let cloud_enabled = cloud_enabled.clone();
            let version = version.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = toggle_cloud().await {
                    cloud_enabled.set(response.cloud_enabled);
                    version.set(*version + 1); // Trigger re-render
                }
            });
        })
    };

    let context = DashboardContext {
        devices: (*devices).clone(),
        messages: (*messages).clone(),
        cloud_enabled: *cloud_enabled,
        service_name: match *service_type {
            ServiceType::Hub => "Hub Dashboard".to_string(),
            ServiceType::Cloud => "Cloud Dashboard".to_string(),
        },
        service_type: (*service_type).clone(),
        version: *version,
    };

    let dashboard_props = DashboardProps {
        context,
        on_toggle_cloud: Some(on_toggle_cloud),
    };

    html! {
        <>
            <Dashboard ..dashboard_props />
            // Hidden WebSocket element for device controls to access
            {if let Some(_ws) = &*ws_clone {
                html! { <div id="websocket" style="display: none;"></div> }
            } else {
                html! {}
            }}
        </>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
async fn fetch_data() -> Result<ApiResponse, Box<dyn std::error::Error>> {
    // Use relative URLs since API is served from the same server
    let devices_response = gloo_net::http::Request::get("/api/devices")
        .send()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let messages_response = gloo_net::http::Request::get("/api/messages")
        .send()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let devices: Vec<DeviceTelemetry> = devices_response.json().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let messages: Vec<String> = messages_response.json().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // For cloud service, cloud_enabled is always true
    let cloud_enabled = false; // Will be set based on service type

    Ok(ApiResponse {
        devices,
        messages,
        cloud_enabled,
    })
}

#[cfg(any(feature = "server", feature = "wasm"))]
async fn fetch_devices() -> Result<Vec<DeviceTelemetry>, Box<dyn std::error::Error>> {
    // Use relative URLs since API is served from the same server
    let devices_response = gloo_net::http::Request::get("/api/devices")
        .send()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let devices: Vec<DeviceTelemetry> = devices_response.json().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    Ok(devices)
}

#[cfg(any(feature = "server", feature = "wasm"))]
async fn fetch_messages() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Use relative URLs since API is served from the same server
    let messages_response = gloo_net::http::Request::get("/api/messages")
        .send()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let messages: Vec<String> = messages_response.json().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    Ok(messages)
}

#[cfg(any(feature = "server", feature = "wasm"))]
async fn toggle_cloud() -> Result<ApiResponse, Box<dyn std::error::Error>> {
    // Use relative URLs since API is served from the same server
    let response = gloo_net::http::Request::post("/api/toggle-cloud")
        .send()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let result: serde_json::Value = response.json().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let cloud_enabled = result["cloud_enabled"].as_bool().unwrap_or(false);

    Ok(ApiResponse {
        devices: vec![],
        messages: vec![],
        cloud_enabled,
    })
}

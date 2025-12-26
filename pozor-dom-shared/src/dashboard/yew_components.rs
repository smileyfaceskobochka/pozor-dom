#[cfg(any(feature = "server", feature = "wasm"))]
use yew::prelude::*;
#[cfg(any(feature = "server", feature = "wasm"))]
use web_sys::WebSocket;
#[cfg(any(feature = "server", feature = "wasm"))]
use wasm_bindgen::JsCast;
#[cfg(any(feature = "server", feature = "wasm"))]
use std::collections::HashMap;
#[cfg(any(feature = "server", feature = "wasm"))]
use crate::dashboard::DeviceTelemetry;

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq)]
pub struct DashboardContext {
    pub devices: HashMap<String, DeviceTelemetry>,
    pub messages: Vec<String>,
    pub cloud_enabled: bool,
    pub service_name: String,
    pub service_type: ServiceType,
    pub version: u32,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq)]
pub enum ServiceType {
    Hub,
    Cloud,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
pub struct DashboardProps {
    pub context: DashboardContext,
    pub on_toggle_cloud: Option<Callback<()>>,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(Dashboard)]
pub fn dashboard(props: &DashboardProps) -> Html {
    let devices = props.context.devices.clone();
    let messages = props.context.messages.clone();
    let cloud_enabled = props.context.cloud_enabled;
    let service_name = props.context.service_name.clone();
    let service_type = props.context.service_type.clone();

    html! {
        <div class="container">
            <Header service_name={service_name} service_type={service_type.clone()} />

            <div class="content">
                <CloudToggle
                    enabled={cloud_enabled}
                    on_toggle={props.on_toggle_cloud.clone()}
                />

                <section class="section">
                    <h2>{"📊 Connected Devices"}</h2>
                    <DevicesGrid devices={devices.clone()} />
                </section>

                <section class="section">
                    <h2>{"💬 Recent Messages"}</h2>
                    <MessagesList messages={messages} />
                </section>

                <section class="section">
                    <h2>{"🎛️ Device Controls"}</h2>
                    <DeviceControls devices={devices} />
                </section>
            </div>
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(Header)]
fn header(props: &HeaderProps) -> Html {
    let title = match props.service_type {
        ServiceType::Hub => "🏠 Pozor-dom Hub Dashboard",
        ServiceType::Cloud => "☁️ Pozor-dom Cloud Dashboard",
    };

    let subtitle = match props.service_type {
        ServiceType::Hub => "Smart Home Device Monitoring & Control - Hub",
        ServiceType::Cloud => "Smart Home Device Monitoring & Control - Cloud",
    };

    html! {
        <div class="header">
            <h1>{title}</h1>
            <p>{subtitle}</p>
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
struct HeaderProps {
    pub service_name: String,
    pub service_type: ServiceType,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(CloudToggle)]
fn cloud_toggle(props: &CloudToggleProps) -> Html {
    let enabled = props.enabled;
    let on_toggle = props.on_toggle.clone();

    let status_class = if enabled { "enabled" } else { "disabled" };
    let status_text = if enabled { "Cloud: Enabled" } else { "Cloud: Disabled" };
    let button_text = if enabled { "Disable Cloud" } else { "Enable Cloud" };
    let button_class = if enabled { "btn-cloud enabled" } else { "btn-cloud" };

    let onclick = {
        let on_toggle = on_toggle.clone();
        Callback::from(move |_| {
            if let Some(toggle) = on_toggle.clone() {
                toggle.emit(());
            }
        })
    };

    html! {
        <div class="cloud-toggle">
            <span class={classes!("cloud-status", status_class)}>{status_text}</span>
            <button class={classes!("btn", button_class)} {onclick}>{button_text}</button>
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
struct CloudToggleProps {
    pub enabled: bool,
    pub on_toggle: Option<Callback<()>>,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(DevicesGrid)]
fn devices_grid(props: &DevicesGridProps) -> Html {
    let devices: Vec<DeviceTelemetry> = props.devices.values().cloned().collect();

    if devices.is_empty() {
        return html! { <p>{"No devices connected"}</p> };
    }

    html! {
        <div class="devices-grid">
            {for devices.iter().map(|device| html! {
                <DeviceCard device={device.clone()} />
            })}
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
struct DevicesGridProps {
    pub devices: HashMap<String, DeviceTelemetry>,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(DeviceCard)]
fn device_card(props: &DeviceCardProps) -> Html {
    let channel_class = format!("channel-{}", props.device.channel.to_lowercase());

    html! {
        <div class="device-card">
            <div class="device-id">{&props.device.device_id}</div>
            <span class={classes!("device-channel", channel_class)}>{&props.device.channel}</span>

            <div class="device-metrics">
                <div class="metric">
                    <span class="metric-label">{"Temperature:"}</span>
                    <span class={classes!("metric-value", "temperature")}>{props.device.temperature.clone() + "°C"}</span>
                </div>
                <div class="metric">
                    <span class="metric-label">{"Humidity:"}</span>
                    <span class={classes!("metric-value", "humidity")}>{props.device.humidity.clone() + "%"}</span>
                </div>
                <div class="metric">
                    <span class="metric-label">{"Signal Strength:"}</span>
                    <span class={classes!("metric-value", "signal")}>{props.device.signal_strength.to_string() + " dBm"}</span>
                </div>
            </div>

            <div class="timestamp">
                {"Last updated: "}{&props.device.timestamp}
            </div>
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
struct DeviceCardProps {
    pub device: DeviceTelemetry,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(MessagesList)]
fn messages_list(props: &MessagesListProps) -> Html {
    let messages = props.messages.clone();

    if messages.is_empty() {
        return html! { <p>{"No messages yet"}</p> };
    }

    // Show only last 20 messages
    let recent_messages = messages.iter().rev().take(20).rev().cloned().collect::<Vec<_>>();

    html! {
        <div class="messages">
            {for recent_messages.iter().map(|message| html! {
                <div class="message">{message}</div>
            })}
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
struct MessagesListProps {
    pub messages: Vec<String>,
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[function_component(DeviceControls)]
fn device_controls(props: &DeviceControlsProps) -> Html {
    let devices: Vec<DeviceTelemetry> = props.devices.values().cloned().collect();
    let selected_device = use_state(|| String::new());
    let action_input = use_state(|| String::new());

    let on_device_change = {
        let selected_device = selected_device.clone();
        Callback::from(move |e: Event| {
            let value = e.target_unchecked_into::<web_sys::HtmlElement>()
                .dyn_into::<web_sys::HtmlSelectElement>()
                .ok()
                .map(|el| el.value())
                .unwrap_or_default();
            selected_device.set(value);
        })
    };

    let on_action_change = {
        let action_input = action_input.clone();
        Callback::from(move |e: Event| {
            let value = e.target_unchecked_into::<web_sys::HtmlInputElement>().value();
            action_input.set(value);
        })
    };

    let on_submit = {
        let selected_device = selected_device.clone();
        let action_input = action_input.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let device_id = (*selected_device).clone();
            let action = (*action_input).clone();

            if !device_id.is_empty() && !action.is_empty() {
                // Send command via WebSocket
                if let Some(ws) = web_sys::window().and_then(|w| w.document()).and_then(|d| {
                    d.get_element_by_id("websocket").and_then(|e| e.dyn_into::<WebSocket>().ok())
                }) {
                    let command = serde_json::json!({
                        "type": "command",
                        "device_id": device_id,
                        "action": action,
                        "timestamp": js_sys::Date::now().to_string()
                    });

                    if let Ok(msg) = serde_json::to_string(&command) {
                        if let Err(_err) = ws.send_with_str(&msg) {
                            // Log error if needed
                        }
                    }
                }

                action_input.set(String::new());
            }
        })
    };

    let on_key_press = {
        let selected_device = selected_device.clone();
        let action_input = action_input.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                let device_id = (*selected_device).clone();
                let action = (*action_input).clone();

                if !device_id.is_empty() && !action.is_empty() {
                    // Send command via WebSocket
                    if let Some(ws) = web_sys::window().and_then(|w| w.document()).and_then(|d| {
                        d.get_element_by_id("websocket").and_then(|e| e.dyn_into::<WebSocket>().ok())
                    }) {
                        let command = serde_json::json!({
                            "type": "command",
                            "device_id": device_id,
                            "action": action,
                            "timestamp": js_sys::Date::now().to_string()
                        });

                        if let Ok(msg) = serde_json::to_string(&command) {
                        if let Err(_err) = ws.send_with_str(&msg) {
                            // Log error if needed
                        }
                        }
                    }

                    action_input.set(String::new());
                }
            }
        })
    };

    html! {
        <div class="controls">
            <div class="control-group">
                <label for="device-select">{"Select Device:"}</label>
                <select id="device-select" onchange={on_device_change}>
                    <option value="">{"Choose a device..."}</option>
                    {for devices.iter().map(|device| html! {
                        <option value={device.device_id.clone()}>
                            {format!("{} ({})", device.device_id, device.channel)}
                        </option>
                    })}
                </select>
            </div>
            <div class="control-group">
                <label for="action-input">{"Action:"}</label>
                <input
                    type="text"
                    id="action-input"
                    placeholder="Enter action (e.g., get_temperature)"
                    value={(*action_input).clone()}
                    onchange={on_action_change}
                    onkeypress={on_key_press}
                />
            </div>
            <button class="btn" onclick={on_submit}>{"Send Command"}</button>
        </div>
    }
}

#[cfg(any(feature = "server", feature = "wasm"))]
#[derive(Clone, PartialEq, Properties)]
struct DeviceControlsProps {
    pub devices: HashMap<String, DeviceTelemetry>,
}

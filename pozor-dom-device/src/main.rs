
use rand::Rng;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

const MQTT_BROKER: &str = "127.0.0.1";
const MQTT_PORT: u16 = 1883;

#[derive(Debug, Clone)]
struct Device {
    id: String,
    channel: String,
}

#[tokio::main]
async fn main() {
    println!("🔌 Позор-дом Device - MQTT Emulator");
    println!("Подключение к MQTT брокеру: {}:{}\n", MQTT_BROKER, MQTT_PORT);

    // Create devices for each channel - expanded device set
    let devices = vec![
        // WiFi devices
        Device {
            id: "device-wifi-001".to_string(),
            channel: "WiFi".to_string(),
        },
        Device {
            id: "device-wifi-002".to_string(),
            channel: "WiFi".to_string(),
        },
        Device {
            id: "device-wifi-003".to_string(),
            channel: "WiFi".to_string(),
        },
        // BLE devices
        Device {
            id: "device-ble-001".to_string(),
            channel: "BLE".to_string(),
        },
        Device {
            id: "device-ble-002".to_string(),
            channel: "BLE".to_string(),
        },
        Device {
            id: "device-ble-003".to_string(),
            channel: "BLE".to_string(),
        },
        // ZigBee devices
        Device {
            id: "device-zigbee-001".to_string(),
            channel: "ZigBee".to_string(),
        },
        Device {
            id: "device-zigbee-002".to_string(),
            channel: "ZigBee".to_string(),
        },
        Device {
            id: "device-zigbee-003".to_string(),
            channel: "ZigBee".to_string(),
        },
        // Additional device types
        Device {
            id: "sensor-temp-001".to_string(),
            channel: "WiFi".to_string(),
        },
        Device {
            id: "sensor-motion-001".to_string(),
            channel: "BLE".to_string(),
        },
        Device {
            id: "light-bulb-001".to_string(),
            channel: "ZigBee".to_string(),
        },
        Device {
            id: "thermostat-001".to_string(),
            channel: "WiFi".to_string(),
        },
        Device {
            id: "door-sensor-001".to_string(),
            channel: "BLE".to_string(),
        },
        Device {
            id: "smart-plug-001".to_string(),
            channel: "ZigBee".to_string(),
        },
    ];

    // Spawn a device instance for each channel
    for device in devices {
        tokio::spawn(async move {
            run_device(device).await;
        });
    }

    // Keep main task alive
    loop {
        sleep(Duration::from_secs(3600)).await;
    }
}

async fn run_device(device: Device) {
    let device_id = device.id.clone();
    let channel = device.channel.clone();

    println!("📱 Starting device: {} on {} channel", device_id, channel);

    let mut mqtt_options = MqttOptions::new(&device_id, MQTT_BROKER, MQTT_PORT);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

    // Spawn event loop handler
    let device_for_event = device_id.clone();
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    // Handle MQTT notifications (subscribed messages)
                    match notification {
                        rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish)) => {
                            if let Ok(payload) = std::str::from_utf8(&publish.payload) {
                                println!(
                                    "📥 [{}] Received command on {}: {}",
                                    device_for_event, publish.topic, payload
                                );
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("❌ [{}] MQTT Error: {}", device_for_event, e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    sleep(Duration::from_millis(500)).await;

    // Subscribe to command topic for this device
    let command_topic = format!("pozor-dom/hub/command/{}", device_id);
    match client.subscribe(&command_topic, QoS::AtMostOnce).await {
        Ok(_) => println!("✅ [{}] Subscribed to: {}", device_id, command_topic),
        Err(e) => eprintln!("❌ [{}] Subscribe error: {}", device_id, e),
    }

    // Publish telemetry data periodically
    loop {
        // Generate random values inside the loop to avoid Send issues
        let temperature = {
            let mut rng = rand::thread_rng();
            20.0 + rng.gen_range(-5.0..5.0)
        };
        
        let humidity = {
            let mut rng = rand::thread_rng();
            50.0 + rng.gen_range(-20.0..20.0)
        };
        
        let signal_strength = {
            let mut rng = rand::thread_rng();
            rng.gen_range(-100..-30)
        };

        let timestamp = chrono::Local::now().to_rfc3339();

        let telemetry = json!({
            "device_id": device_id,
            "channel": channel,
            "temperature": format!("{:.2}", temperature),
            "humidity": format!("{:.2}", humidity),
            "timestamp": timestamp,
            "signal_strength": signal_strength
        });

        let topic = format!("pozor-dom/device/{}/telemetry", device_id);
        match client
            .publish(
                &topic,
                QoS::AtLeastOnce,
                false,
                telemetry.to_string().into_bytes(),
            )
            .await
        {
            Ok(_) => println!(
                "📤 [{}] Telemetry: temp={:.2}°C, humidity={:.2}%",
                device_id, temperature, humidity
            ),
            Err(e) => eprintln!("❌ [{}] Publish error: {}", device_id, e),
        }

        sleep(Duration::from_secs(5)).await;
    }
}

use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::sync::Arc;
use std::time::Duration;
use serde_json::Value;

pub async fn setup_mqtt_client() -> (AsyncClient, rumqttc::EventLoop) {
    let mut opts = MqttOptions::new("pozor-dom-hub", "127.0.0.1", 1883);
    opts.set_keep_alive(Duration::from_secs(5));

    let (client, eventloop) = AsyncClient::new(opts, 100);

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Subscribe to telemetry
    match client.subscribe("pozor-dom/device/+/telemetry", QoS::AtMostOnce).await {
        Ok(_) => println!("✅ Hub subscribed to device telemetry"),
        Err(e) => eprintln!("❌ Hub failed to subscribe to telemetry: {}", e),
    }

    (client, eventloop)
}



pub async fn send_device_command(
    command: &Value,
    mqtt_client: &Arc<AsyncClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let (Some(device_id), Some(channel), Some(action)) = (
        command["device_id"].as_str(),
        command["channel"].as_str(),
        command["action"].as_str(),
    ) {
        let topic = format!("pozor-dom/hub/command/{}", device_id);

        println!(
            "� Relaying to MQTT - Device: {} | Channel: {} | Action: {}",
            device_id, channel, action
        );

        mqtt_client
            .publish(
                &topic,
                QoS::AtLeastOnce,
                false,
                command.to_string().into_bytes(),
            )
            .await?;

        println!("✅ Published to MQTT: {}", topic);
        Ok(())
    } else {
        Err("Invalid command format".into())
    }
}

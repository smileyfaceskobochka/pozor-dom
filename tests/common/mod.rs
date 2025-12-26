use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;

pub struct TestMqttClient {
    pub client: AsyncClient,
    pub received_messages: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl TestMqttClient {
    pub async fn new(client_id: &str) -> Self {
        let mut options = MqttOptions::new(client_id, "127.0.0.1", 1883);
        options.set_keep_alive(Duration::from_secs(5));

        let (client, _) = AsyncClient::new(options, 10);
        let received_messages = Arc::new(Mutex::new(HashMap::new()));

        Self {
            client,
            received_messages,
        }
    }

    pub async fn subscribe(&self, topic: &str) {
        self.client.subscribe(topic, QoS::AtLeastOnce).await.unwrap();
    }

    pub async fn get_messages(&self, topic: &str) -> Vec<String> {
        let messages = self.received_messages.lock().await;
        messages.get(topic).cloned().unwrap_or_default()
    }

    pub async fn publish(&self, topic: &str, payload: &str) {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload.as_bytes())
            .await
            .unwrap();
    }
}

pub async fn setup_mqtt_client() -> TestMqttClient {
    TestMqttClient::new("test-client").await
}

pub async fn setup_ws_client(url: &str) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("Failed to connect to WebSocket");

    ws_stream
}

pub async fn wait_for_mqtt_message(
    mqtt_client: &TestMqttClient,
    topic: &str,
    timeout_duration: Duration,
) -> Option<String> {
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        let messages = mqtt_client.get_messages(topic).await;
        if !messages.is_empty() {
            return Some(messages.last().unwrap().clone());
        }
        sleep(Duration::from_millis(100)).await;
    }

    None
}

pub async fn make_http_request(url: &str) -> Result<reqwest::Response, reqwest::Error> {
    reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
}

pub async fn make_http_post(url: &str, body: &str) -> Result<reqwest::Response, reqwest::Error> {
    reqwest::Client::new()
        .post(url)
        .header("content-type", "application/json")
        .body(body.to_string())
        .timeout(Duration::from_secs(5))
        .send()
        .await
}

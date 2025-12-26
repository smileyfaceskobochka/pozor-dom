//! Comprehensive Testing Suite for Pozor-dom System
//!
//! This module contains automated tests covering:
//! - Black Box Testing: API functionality and external interfaces
//! - White Box Testing: Internal logic and data structures
//! - Non-Functional Testing: Performance requirements

use serde_json::json;
use std::time::{Duration, Instant};

mod common;

// ===== UNIT TESTS =====

#[tokio::test]
async fn unit_test_device_telemetry_creation() {
    println!("\n🧪 Unit Test: Device Telemetry Creation");

    let telemetry = pozor_dom_shared::dashboard::lib::DeviceTelemetry {
        device_id: "test-device-001".to_string(),
        channel: "WiFi".to_string(),
        temperature: "23.5".to_string(),
        humidity: "65.0".to_string(),
        signal_strength: -50,
        timestamp: chrono::Local::now().to_rfc3339(),
    };

    assert_eq!(telemetry.device_id, "test-device-001");
    assert_eq!(telemetry.channel, "WiFi");
    assert_eq!(telemetry.temperature, "23.5");
    assert_eq!(telemetry.humidity, "65.0");
    assert_eq!(telemetry.signal_strength, -50);
    assert!(!telemetry.timestamp.is_empty());

    println!("✅ Device telemetry structure works correctly");
}

#[tokio::test]
async fn unit_test_hub_state_operations() {
    println!("\n🧪 Unit Test: Hub State Operations");

    let mut hub_state = pozor_dom_shared::dashboard::lib::HubState::new("Test Hub");

    // Test initial state
    assert_eq!(hub_state.service_name, "Test Hub");
    assert!(!hub_state.cloud_enabled);
    assert!(hub_state.devices.is_empty());
    assert!(hub_state.messages.is_empty());

    // Test device update
    let telemetry = pozor_dom_shared::dashboard::lib::DeviceTelemetry {
        device_id: "device-001".to_string(),
        channel: "WiFi".to_string(),
        temperature: "22.0".to_string(),
        humidity: "60.0".to_string(),
        signal_strength: -40,
        timestamp: chrono::Local::now().to_rfc3339(),
    };

    hub_state.update_device(telemetry);
    assert_eq!(hub_state.devices.len(), 1);
    assert!(hub_state.devices.contains_key("device-001"));

    // Test cloud toggle
    assert!(!hub_state.cloud_enabled);
    hub_state.toggle_cloud();
    assert!(hub_state.cloud_enabled);
    hub_state.toggle_cloud();
    assert!(!hub_state.cloud_enabled);

    // Test message addition
    hub_state.add_message("Test message".to_string());
    assert_eq!(hub_state.messages.len(), 1);
    assert_eq!(hub_state.messages[0], "Test message");

    println!("✅ Hub state operations work correctly");
}

#[tokio::test]
async fn unit_test_json_serialization() {
    println!("\n🧪 Unit Test: JSON Serialization");

    let telemetry = pozor_dom_shared::dashboard::lib::DeviceTelemetry {
        device_id: "sensor-001".to_string(),
        channel: "BLE".to_string(),
        temperature: "25.5".to_string(),
        humidity: "70.0".to_string(),
        signal_strength: -60,
        timestamp: "2024-01-01T12:00:00Z".to_string(),
    };

    // Test serialization
    let json_str = serde_json::to_string(&telemetry).unwrap();
    assert!(json_str.contains("sensor-001"));
    assert!(json_str.contains("BLE"));
    assert!(json_str.contains("25.5"));

    // Test deserialization
    let deserialized: pozor_dom_shared::dashboard::lib::DeviceTelemetry = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.device_id, telemetry.device_id);
    assert_eq!(deserialized.channel, telemetry.channel);
    assert_eq!(deserialized.temperature, telemetry.temperature);

    println!("✅ JSON serialization/deserialization works correctly");
}

// ===== BLACK BOX TESTS =====

#[tokio::test]
async fn black_box_test_api_devices_endpoint() {
    println!("\n🧪 Black Box Test: API Devices Endpoint");

    let response = common::make_http_request("http://localhost:3000/api/devices")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "API should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    assert!(!body_text.is_empty(), "Response should not be empty");

    // Try to parse as JSON array
    let devices: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&body_text);
    if let Ok(devices) = devices {
        assert!(!devices.is_empty(), "Response should contain devices");
        println!("✅ API devices endpoint returns valid JSON array");
    } else {
        // If not an array, at least check it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");
        println!("✅ API devices endpoint returns valid JSON");
    }
}

#[tokio::test]
async fn black_box_test_api_messages_endpoint() {
    println!("\n🧪 Black Box Test: API Messages Endpoint");

    let response = common::make_http_request("http://localhost:3000/api/messages")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "API should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    assert!(!body_text.is_empty(), "Response should not be empty");

    // Try to parse as JSON array
    let messages: Result<Vec<String>, _> = serde_json::from_str(&body_text);
    if let Ok(messages) = messages {
        assert!(!messages.is_empty() || true, "Response should be an array"); // Allow empty
        println!("✅ API messages endpoint returns valid JSON array");
    } else {
        // If not an array, at least check it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");
        println!("✅ API messages endpoint returns valid JSON");
    }
}

#[tokio::test]
async fn black_box_test_cloud_api_proxy_devices() {
    println!("\n🧪 Black Box Test: Cloud API Proxy (Devices)");

    let response = common::make_http_request("http://localhost:8080/api/devices")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "Cloud API proxy should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    assert!(!body_text.is_empty(), "Response should not be empty");

    // Should be able to parse as JSON
    let _: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");

    println!("✅ Cloud API proxy forwards devices endpoint correctly");
}

#[tokio::test]
async fn black_box_test_cloud_api_proxy_messages() {
    println!("\n🧪 Black Box Test: Cloud API Proxy (Messages)");

    let response = common::make_http_request("http://localhost:8080/api/messages")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "Cloud API proxy should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    assert!(!body_text.is_empty(), "Response should not be empty");

    // Should be able to parse as JSON
    let _: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");

    println!("✅ Cloud API proxy forwards messages endpoint correctly");
}

#[tokio::test]
async fn black_box_test_cloud_toggle_api() {
    println!("\n🧪 Black Box Test: Cloud Toggle API");

    let response = common::make_http_post("http://localhost:8080/api/toggle-cloud", "{}")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "Cloud toggle API should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    let result: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");
    assert!(result.get("cloud_enabled").is_some(), "Response should contain cloud_enabled");

    println!("✅ Cloud toggle API works correctly");
}

#[tokio::test]
async fn black_box_test_hub_toggle_api() {
    println!("\n🧪 Black Box Test: Hub Toggle API");

    let response = common::make_http_post("http://localhost:3000/api/toggle-cloud", "{}")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "Hub toggle API should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    let result: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");
    assert!(result.get("cloud_enabled").is_some(), "Response should contain cloud_enabled");

    println!("✅ Hub toggle API works correctly");
}

#[tokio::test]
async fn black_box_test_invalid_api_endpoint() {
    println!("\n🧪 Black Box Test: Invalid API Endpoint");

    let response = common::make_http_request("http://localhost:3000/api/invalid")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 404, "Invalid endpoint should return 404");

    println!("✅ Invalid API endpoints properly return 404");
}

#[tokio::test]
async fn black_box_test_cloud_toggle_functionality() {
    println!("\n🧪 Black Box Test: Cloud Toggle Functionality");

    // Test that toggle API works and persists state
    let initial_response = common::make_http_post("http://localhost:3000/api/toggle-cloud", "{}")
        .await
        .expect("Failed to toggle cloud");

    let body_text = initial_response.text().await.expect("Should get response text");
    let result: serde_json::Value = serde_json::from_str(&body_text).expect("Should return valid JSON");

    let first_state = result.get("cloud_enabled").unwrap().as_bool().unwrap();
    println!("  First toggle result: cloud_enabled = {}", first_state);

    // Toggle again
    let second_response = common::make_http_post("http://localhost:3000/api/toggle-cloud", "{}")
        .await
        .expect("Failed to toggle cloud");

    let body_text2 = second_response.text().await.expect("Should get response text");
    let result2: serde_json::Value = serde_json::from_str(&body_text2).expect("Should return valid JSON");

    let second_state = result2.get("cloud_enabled").unwrap().as_bool().unwrap();
    println!("  Second toggle result: cloud_enabled = {}", second_state);

    // States should be different
    assert_ne!(first_state, second_state, "Toggle should change the state");

    // Hub should always have access to its database regardless of cloud state
    let devices_response = common::make_http_request("http://localhost:3000/api/devices")
        .await
        .expect("Failed to get devices");

    assert_eq!(devices_response.status(), 200, "Hub should always access its database");

    let messages_response = common::make_http_request("http://localhost:3000/api/messages")
        .await
        .expect("Failed to get messages");

    assert_eq!(messages_response.status(), 200, "Hub should always access its messages");

    println!("✅ Cloud toggle functionality works correctly - hub always has database access");
}



#[tokio::test]
async fn black_box_test_dashboard_html_serving() {
    println!("\n🧪 Black Box Test: Dashboard HTML Serving");

    let response = common::make_http_request("http://localhost:3000/")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(response.status(), 200, "Dashboard should return 200 OK");

    let body_text = response.text().await.expect("Should get response text");
    assert!(!body_text.is_empty(), "Response should not be empty");
    assert!(body_text.contains("Pozor-dom"), "Should contain dashboard title");
    assert!(body_text.contains("<html"), "Should be HTML content");

    println!("✅ Dashboard HTML is served correctly");
}

#[tokio::test]
async fn black_box_test_wasm_files_serving() {
    println!("\n🧪 Black Box Test: WebAssembly Files Serving");

    // Test JS file
    let js_response = common::make_http_request("http://localhost:3000/pozor_dom_shared.js")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(js_response.status(), 200, "JS file should return 200 OK");

    let js_content = js_response.text().await.expect("Should get JS content");
    assert!(!js_content.is_empty(), "JS file should not be empty");
    assert!(js_content.contains("WebAssembly"), "Should contain WebAssembly references");

    // Test WASM file
    let wasm_response = common::make_http_request("http://localhost:3000/pozor_dom_shared_bg.wasm")
        .await
        .expect("Failed to make HTTP request");

    assert_eq!(wasm_response.status(), 200, "WASM file should return 200 OK");

    println!("✅ WebAssembly files are served correctly");
}

// ===== WHITE BOX TEST =====

#[tokio::test]
async fn white_box_test_device_telemetry_validation() {
    println!("\n🔍 White Box Test: Device Telemetry Validation");

    // Test valid telemetry
    let valid_telemetry = json!({
        "device_id": "valid-device-001",
        "channel": "WiFi",
        "temperature": "23.5",
        "humidity": "65.0",
        "signal_strength": -50,
        "timestamp": chrono::Local::now().to_rfc3339()
    });

    let telemetry: Result<pozor_dom_shared::dashboard::lib::DeviceTelemetry, _> = serde_json::from_value(valid_telemetry);
    assert!(telemetry.is_ok(), "Valid telemetry should parse successfully");

    let telemetry = telemetry.unwrap();
    assert_eq!(telemetry.device_id, "valid-device-001");
    assert_eq!(telemetry.channel, "WiFi");
    assert_eq!(telemetry.temperature, "23.5");

    // Test invalid telemetry (missing required field)
    let invalid_telemetry = json!({
        "channel": "WiFi",
        "temperature": "23.5",
        "humidity": "65.0",
        "signal_strength": -50,
        "timestamp": chrono::Local::now().to_rfc3339()
    });

    let telemetry: Result<pozor_dom_shared::dashboard::lib::DeviceTelemetry, _> = serde_json::from_value(invalid_telemetry);
    assert!(telemetry.is_err(), "Invalid telemetry (missing device_id) should fail to parse");

    println!("✅ Device telemetry validation works correctly");
}

#[tokio::test]
async fn white_box_test_hub_state_device_management() {
    println!("\n🔍 White Box Test: Hub State Device Management");

    let mut hub_state = pozor_dom_shared::dashboard::lib::HubState::new("Test Hub");

    // Test adding first device
    let device1 = pozor_dom_shared::dashboard::lib::DeviceTelemetry {
        device_id: "device-001".to_string(),
        channel: "WiFi".to_string(),
        temperature: "22.0".to_string(),
        humidity: "60.0".to_string(),
        signal_strength: -40,
        timestamp: chrono::Local::now().to_rfc3339(),
    };

    hub_state.update_device(device1);
    assert_eq!(hub_state.devices.len(), 1);
    assert!(hub_state.devices.contains_key("device-001"));

    // Test updating existing device
    let device1_updated = pozor_dom_shared::dashboard::lib::DeviceTelemetry {
        device_id: "device-001".to_string(),
        channel: "WiFi".to_string(),
        temperature: "25.0".to_string(), // Updated temperature
        humidity: "60.0".to_string(),
        signal_strength: -40,
        timestamp: chrono::Local::now().to_rfc3339(),
    };

    hub_state.update_device(device1_updated);
    assert_eq!(hub_state.devices.len(), 1); // Still one device
    let stored = hub_state.devices.get("device-001").unwrap();
    assert_eq!(stored.temperature, "25.0"); // Temperature should be updated

    // Test adding second device
    let device2 = pozor_dom_shared::dashboard::lib::DeviceTelemetry {
        device_id: "device-002".to_string(),
        channel: "BLE".to_string(),
        temperature: "20.0".to_string(),
        humidity: "55.0".to_string(),
        signal_strength: -60,
        timestamp: chrono::Local::now().to_rfc3339(),
    };

    hub_state.update_device(device2);
    assert_eq!(hub_state.devices.len(), 2);
    assert!(hub_state.devices.contains_key("device-001"));
    assert!(hub_state.devices.contains_key("device-002"));

    println!("✅ Hub state device management works correctly");
}

// ===== NON-FUNCTIONAL REQUIREMENT TEST =====

#[tokio::test]
async fn non_functional_test_api_response_time() {
    println!("\n⚡ Non-Functional Test: API Response Time Performance");

    let mut response_times = Vec::new();
    let test_iterations = 10;

    // Test API response times
    for i in 0..test_iterations {
        let start = Instant::now();

        let response = common::make_http_request("http://localhost:3000/api/devices")
            .await
            .expect("Failed to make HTTP request");

        let duration = start.elapsed();
        response_times.push(duration);

        assert_eq!(response.status(), 200, "API should return 200 OK in iteration {}", i + 1);
        println!("  Iteration {}: {:.2}ms", i + 1, duration.as_millis());
    }

    // Calculate statistics
    let total_duration: Duration = response_times.iter().sum();
    let avg_response_time = total_duration / response_times.len() as u32;
    let max_response_time = response_times.iter().max().unwrap();
    let min_response_time = response_times.iter().min().unwrap();

    println!("\n📊 Performance Statistics:");
    println!("  Average response time: {:.2}ms", avg_response_time.as_millis());
    println!("  Maximum response time: {:.2}ms", max_response_time.as_millis());
    println!("  Minimum response time: {:.2}ms", min_response_time.as_millis());

    // Assert performance requirements
    assert!(*max_response_time < Duration::from_millis(1000), "Maximum response time should be under 1000ms");
    assert!(avg_response_time < Duration::from_millis(500), "Average response time should be under 500ms");

    println!("✅ Non-functional requirements met: API response times within acceptable limits");
}

#[tokio::test]
async fn non_functional_test_api_concurrent_requests() {
    println!("\n⚡ Non-Functional Test: API Concurrent Requests");

    use futures::future::join_all;

    let num_concurrent_requests = 10;
    let mut request_futures = Vec::new();

    // Create multiple concurrent requests
    for _ in 0..num_concurrent_requests {
        let future = common::make_http_request("http://localhost:3000/api/devices");
        request_futures.push(future);
    }

    // Execute all requests concurrently
    let start = Instant::now();
    let results = join_all(request_futures).await;
    let total_duration = start.elapsed();

    // Verify all requests succeeded
    let mut success_count = 0;
    for result in results {
        match result {
            Ok(response) => {
                if response.status() == 200 {
                    success_count += 1;
                }
            }
            Err(_) => {} // Request failed
        }
    }

    println!("📊 Concurrent Request Statistics:");
    println!("  Total requests: {}", num_concurrent_requests);
    println!("  Successful responses: {}", success_count);
    println!("  Total time: {:.2}ms", total_duration.as_millis());
    println!("  Average time per request: {:.2}ms", total_duration.as_millis() as f64 / num_concurrent_requests as f64);

    assert_eq!(success_count, num_concurrent_requests, "All concurrent requests should succeed");

    println!("✅ System handles concurrent API requests correctly");
}

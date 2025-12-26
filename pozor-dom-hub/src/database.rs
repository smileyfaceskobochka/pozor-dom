use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use pozor_dom_shared::dashboard::lib::DeviceTelemetry;
use chrono::Utc;

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Create devices table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                channel TEXT NOT NULL,
                temperature TEXT NOT NULL,
                humidity TEXT NOT NULL,
                signal_strength INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                last_seen TEXT NOT NULL
            )",
            [],
        )?;

        // Create messages table for logging
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;

        // Create config table for storing configuration
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create indexes for better performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_channel ON devices(channel)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_timestamp ON devices(timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp)",
            [],
        )?;

        Ok(Database { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn save_device(&self, telemetry: &DeviceTelemetry) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO devices
             (device_id, channel, temperature, humidity, signal_strength, timestamp, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            [
                &telemetry.device_id,
                &telemetry.channel,
                &telemetry.temperature,
                &telemetry.humidity,
                &telemetry.signal_strength.to_string(),
                &telemetry.timestamp,
                &now,
            ],
        )?;

        Ok(())
    }

    pub fn load_devices(&self) -> Result<HashMap<String, DeviceTelemetry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT device_id, channel, temperature, humidity, signal_strength, timestamp
             FROM devices ORDER BY last_seen DESC"
        )?;

        let device_iter = stmt.query_map([], |row| {
            Ok(DeviceTelemetry {
                device_id: row.get(0)?,
                channel: row.get(1)?,
                temperature: row.get(2)?,
                humidity: row.get(3)?,
                signal_strength: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut devices = HashMap::new();
        for device in device_iter {
            let device = device?;
            devices.insert(device.device_id.clone(), device);
        }

        Ok(devices)
    }

    pub fn get_cloud_enabled(&self) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM config WHERE key = ?1")?;
        let result: Result<String> = stmt.query_row(["cloud_enabled"], |row| row.get(0));

        match result {
            Ok(value) => Ok(value == "true"),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Default to true if not set
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }

    pub fn set_cloud_enabled(&self, enabled: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let value = if enabled { "true" } else { "false" };

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value, updated_at) VALUES (?1, ?2, ?3)",
            ["cloud_enabled", value, &now],
        )?;

        Ok(())
    }
}

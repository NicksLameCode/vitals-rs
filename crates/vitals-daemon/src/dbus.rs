use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::interface;

/// Cached sensor readings for D-Bus exposure.
pub struct SensorData {
    /// Map of key -> (label, value, category, format)
    pub readings: HashMap<String, (String, f64, String, String)>,
    /// Map of key -> (label, text_value, category, format) for text values
    pub text_readings: HashMap<String, (String, String, String, String)>,
}

impl SensorData {
    pub fn new() -> Self {
        Self {
            readings: HashMap::new(),
            text_readings: HashMap::new(),
        }
    }
}

/// D-Bus interface for Vitals sensor data.
pub struct VitalsSensors {
    pub data: Arc<Mutex<SensorData>>,
    pub time_series: Arc<Mutex<vitals_core::history::TimeSeriesStore>>,
}

#[interface(name = "com.corecoding.Vitals.Sensors")]
impl VitalsSensors {
    /// Get all current numeric sensor readings.
    /// Returns a map of key -> (label, value, category, format).
    fn get_readings(&self) -> HashMap<String, (String, f64, String, String)> {
        let data = self.data.lock().unwrap();
        data.readings.clone()
    }

    /// Get all current text sensor readings.
    fn get_text_readings(&self) -> HashMap<String, (String, String, String, String)> {
        let data = self.data.lock().unwrap();
        data.text_readings.clone()
    }

    /// Get time-series data for a specific sensor key.
    /// Returns an array of (timestamp, value) pairs.
    fn get_time_series(&self, key: String) -> Vec<(f64, f64)> {
        let ts = self.time_series.lock().unwrap();
        ts.get(&key)
            .iter()
            .filter_map(|point| point.v.map(|v| (point.t, v)))
            .collect()
    }

    /// Get the current configuration as a TOML string.
    fn get_config(&self) -> String {
        let config = vitals_core::config::AppConfig::load();
        toml::to_string_pretty(&config).unwrap_or_default()
    }

    /// Set configuration from a TOML string.
    fn set_config(&self, toml_str: String) -> bool {
        match toml::from_str::<vitals_core::config::AppConfig>(&toml_str) {
            Ok(config) => config.save().is_ok(),
            Err(_) => false,
        }
    }

    /// Signal emitted when readings change.
    #[zbus(signal)]
    async fn readings_changed(signal_emitter: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;
}

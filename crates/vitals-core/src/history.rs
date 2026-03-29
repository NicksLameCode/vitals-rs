use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::sensors::SensorFormat;

/// A single time-series data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePoint {
    pub t: f64,
    pub v: Option<f64>,
}

/// Persistent time-series data compatible with the GJS extension's history.json format.
#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    version: u32,
    #[serde(rename = "timeSeries")]
    time_series: HashMap<String, Vec<TimePoint>>,
    #[serde(rename = "timeSeriesFormat")]
    time_series_format: HashMap<String, String>,
}

/// Manages time-series data for all sensors.
pub struct TimeSeriesStore {
    series: HashMap<String, Vec<TimePoint>>,
    formats: HashMap<String, SensorFormat>,
    max_duration_secs: u32,
    max_points: usize,
}

impl TimeSeriesStore {
    pub fn new(max_duration_secs: u32) -> Self {
        Self {
            series: HashMap::new(),
            formats: HashMap::new(),
            max_duration_secs: max_duration_secs.max(60),
            max_points: 3600,
        }
    }

    fn now() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Push a new data point for a sensor key.
    pub fn push(&mut self, key: &str, value: f64, format: SensorFormat, interval: u32) {
        if !format.is_graphable() || !value.is_finite() {
            return;
        }

        self.formats.insert(key.to_string(), format);
        let now = Self::now();
        let buf = self
            .series
            .entry(key.to_string())
            .or_insert_with(Vec::new);

        let min_interval = interval.max(1) as f64 - 0.5;

        // If the last point is recent enough, update it in place
        if let Some(last) = buf.last_mut() {
            if last.v.is_some() && (now - last.t) < min_interval {
                last.v = Some(value);
                return;
            }
        }

        // Fill gaps with null points
        if let Some(last) = buf.last() {
            let gap = now - last.t;
            if gap > interval.max(1) as f64 * 3.0 {
                let mut fill_t = last.t + interval.max(1) as f64;
                while fill_t < now - interval.max(1) as f64 * 0.5 {
                    buf.push(TimePoint {
                        t: fill_t,
                        v: None,
                    });
                    fill_t += interval.max(1) as f64;
                }
            }
        }

        buf.push(TimePoint {
            t: now,
            v: Some(value),
        });

        // Evict old points
        let cutoff = now - self.max_duration_secs as f64;
        buf.retain(|p| p.t >= cutoff);
        while buf.len() > self.max_points {
            buf.remove(0);
        }
    }

    /// Get the time series for a given key.
    pub fn get(&self, key: &str) -> &[TimePoint] {
        self.series.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get the format associated with a key.
    pub fn get_format(&self, key: &str) -> Option<SensorFormat> {
        self.formats.get(key).copied()
    }

    /// Clear all time-series data.
    pub fn clear(&mut self) {
        self.series.clear();
        self.formats.clear();
    }

    /// Save time-series data to a JSON file (compatible with GJS format).
    pub fn save(&self, path: &Path) -> Result<()> {
        let file = HistoryFile {
            version: 1,
            time_series: self.series.clone(),
            time_series_format: self
                .formats
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().to_string()))
                .collect(),
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string(&file)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load time-series data from a JSON file (compatible with GJS format).
    pub fn load(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let contents = std::fs::read_to_string(path)?;
        let file: HistoryFile = serde_json::from_str(&contents)?;

        if file.version != 1 {
            return Ok(());
        }

        let now = Self::now();
        let cutoff = now - self.max_duration_secs as f64;

        for (key, mut points) in file.time_series {
            // Remove old points
            points.retain(|p| p.t >= cutoff);
            // Remove leading nulls
            while points.first().map(|p| p.v.is_none()).unwrap_or(false) {
                points.remove(0);
            }
            if !points.is_empty() {
                self.series.insert(key, points);
            }
        }

        // Convert format strings back to SensorFormat
        for (key, fmt_str) in file.time_series_format {
            if let Some(fmt) = parse_format_str(&fmt_str) {
                self.formats.insert(key, fmt);
            }
        }

        Ok(())
    }
}

fn parse_format_str(s: &str) -> Option<SensorFormat> {
    match s {
        "percent" => Some(SensorFormat::Percent),
        "temp" => Some(SensorFormat::Temp),
        "fan" => Some(SensorFormat::Fan),
        "in" => Some(SensorFormat::Voltage),
        "hertz" => Some(SensorFormat::Hertz),
        "memory" => Some(SensorFormat::Memory),
        "storage" => Some(SensorFormat::Storage),
        "speed" => Some(SensorFormat::Speed),
        "uptime" => Some(SensorFormat::Uptime),
        "runtime" => Some(SensorFormat::Runtime),
        "watt" => Some(SensorFormat::Watt),
        "watt-gpu" => Some(SensorFormat::WattGpu),
        "watt-hour" => Some(SensorFormat::WattHour),
        "milliamp" => Some(SensorFormat::Milliamp),
        "milliamp-hour" => Some(SensorFormat::MilliampHour),
        "load" => Some(SensorFormat::Load),
        "pcie" => Some(SensorFormat::Pcie),
        "string" => Some(SensorFormat::StringVal),
        _ => None,
    }
}

/// Tracks previous sensor values to detect changes and compute aggregates.
pub struct ChangeDetector {
    history: HashMap<String, HashMap<String, (String, f64)>>,
}

impl ChangeDetector {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
        }
    }

    /// Check if a sensor value has changed from the last known value.
    /// Returns true if the value is new or different.
    pub fn has_changed(&mut self, category: &str, key: &str, formatted_value: &str, raw_value: f64) -> bool {
        let cat_history = self.history.entry(category.to_string()).or_default();

        if let Some((prev_formatted, _prev_raw)) = cat_history.get(key) {
            if prev_formatted == formatted_value {
                return false;
            }
        }

        cat_history.insert(key.to_string(), (formatted_value.to_string(), raw_value));
        true
    }

    /// Reset history for all categories except network counters.
    pub fn reset(&mut self) {
        // Keep network-rx and network-tx to preserve speed calculations
        self.history.retain(|k, _| k.starts_with("network-"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let mut store = TimeSeriesStore::new(3600);
        store.push("test_key", 42.0, SensorFormat::Temp, 5);
        let series = store.get("test_key");
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].v, Some(42.0));
    }

    #[test]
    fn test_non_graphable_ignored() {
        let mut store = TimeSeriesStore::new(3600);
        store.push("test_key", 42.0, SensorFormat::StringVal, 5);
        assert!(store.get("test_key").is_empty());
    }

    #[test]
    fn test_nan_ignored() {
        let mut store = TimeSeriesStore::new(3600);
        store.push("test_key", f64::NAN, SensorFormat::Temp, 5);
        assert!(store.get("test_key").is_empty());
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");

        let mut store = TimeSeriesStore::new(3600);
        store.push("cpu_temp", 45000.0, SensorFormat::Temp, 5);
        store.save(&path).unwrap();

        let mut store2 = TimeSeriesStore::new(3600);
        store2.load(&path).unwrap();

        let series = store2.get("cpu_temp");
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].v, Some(45000.0));
        assert_eq!(store2.get_format("cpu_temp"), Some(SensorFormat::Temp));
    }

    #[test]
    fn test_change_detector() {
        let mut cd = ChangeDetector::new();
        assert!(cd.has_changed("temp", "cpu", "45°C", 45000.0));
        assert!(!cd.has_changed("temp", "cpu", "45°C", 45000.0));
        assert!(cd.has_changed("temp", "cpu", "46°C", 46000.0));
    }
}

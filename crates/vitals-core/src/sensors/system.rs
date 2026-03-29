use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct SystemProvider {
    include_static_info: bool,
}

impl SystemProvider {
    pub fn new(include_static_info: bool) -> Self {
        Self { include_static_info }
    }

    /// Parse /proc/sys/fs/file-nr contents into sensor readings.
    pub fn parse_file_nr(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::System;
        if let Some(count) = contents.split('\t').next() {
            readings.push(SensorReading {
                label: "Open Files".to_string(),
                value: SensorValue::Text(count.trim().to_string()),
                category: cat,
                format: SensorFormat::StringVal,
                key: "_system_open_files_".to_string(),
            });
        }
        readings
    }

    /// Parse /proc/loadavg contents into sensor readings.
    pub fn parse_loadavg(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::System;
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Ok(load1) = parts[0].parse::<f64>() {
                readings.push(SensorReading {
                    label: "Load 1m".to_string(),
                    value: SensorValue::Numeric(load1),
                    category: cat.clone(),
                    format: SensorFormat::Load,
                    key: "_system_load_1m_".to_string(),
                });
                readings.push(SensorReading {
                    label: "system".to_string(),
                    value: SensorValue::Numeric(load1),
                    category: cat.clone(),
                    format: SensorFormat::Load,
                    key: "_system_group_".to_string(),
                });
            }
            if let Ok(load5) = parts[1].parse::<f64>() {
                readings.push(SensorReading {
                    label: "Load 5m".to_string(),
                    value: SensorValue::Numeric(load5),
                    category: cat.clone(),
                    format: SensorFormat::Load,
                    key: "_system_load_5m_".to_string(),
                });
            }
            if let Ok(load15) = parts[2].parse::<f64>() {
                readings.push(SensorReading {
                    label: "Load 15m".to_string(),
                    value: SensorValue::Numeric(load15),
                    category: cat.clone(),
                    format: SensorFormat::Load,
                    key: "_system_load_15m_".to_string(),
                });
            }

            let proc_parts: Vec<&str> = parts[3].split('/').collect();
            if proc_parts.len() == 2 {
                readings.push(SensorReading {
                    label: "Threads Active".to_string(),
                    value: SensorValue::Text(proc_parts[0].to_string()),
                    category: cat.clone(),
                    format: SensorFormat::StringVal,
                    key: "_system_threads_active_".to_string(),
                });
                readings.push(SensorReading {
                    label: "Threads Total".to_string(),
                    value: SensorValue::Text(proc_parts[1].to_string()),
                    category: cat,
                    format: SensorFormat::StringVal,
                    key: "_system_threads_total_".to_string(),
                });
            }
        }
        readings
    }

    /// Parse /proc/uptime contents into sensor readings.
    pub fn parse_uptime(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if let Some(uptime_str) = parts.first() {
            if let Ok(uptime) = uptime_str.parse::<f64>() {
                readings.push(SensorReading {
                    label: "Uptime".to_string(),
                    value: SensorValue::Numeric(uptime),
                    category: SensorCategory::System,
                    format: SensorFormat::Uptime,
                    key: "_system_uptime_".to_string(),
                });
            }
        }
        readings
    }

    /// Parse /proc/version contents into sensor readings.
    pub fn parse_version(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if parts.len() >= 3 {
            readings.push(SensorReading {
                label: "Kernel".to_string(),
                value: SensorValue::Text(parts[2].to_string()),
                category: SensorCategory::System,
                format: SensorFormat::StringVal,
                key: "_system_kernel_".to_string(),
            });
        }
        readings
    }
}

impl SensorProvider for SystemProvider {
    fn query(&mut self, _dwell: f64) -> Vec<SensorReading> {
        let mut readings = Vec::new();

        // Open files count from /proc/sys/fs/file-nr
        if let Ok(contents) = std::fs::read_to_string("/proc/sys/fs/file-nr") {
            readings.extend(Self::parse_file_nr(&contents));
        }

        // Load averages from /proc/loadavg
        if let Ok(contents) = std::fs::read_to_string("/proc/loadavg") {
            readings.extend(Self::parse_loadavg(&contents));
        }

        // Uptime from /proc/uptime
        if let Ok(contents) = std::fs::read_to_string("/proc/uptime") {
            readings.extend(Self::parse_uptime(&contents));
        }

        // Kernel version from /proc/version (static info)
        if self.include_static_info {
            if let Ok(contents) = std::fs::read_to_string("/proc/version") {
                readings.extend(Self::parse_version(&contents));
            }
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::System
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_reading<'a>(readings: &'a [SensorReading], key: &str) -> Option<&'a SensorReading> {
        readings.iter().find(|r| r.key == key)
    }

    fn numeric_value(readings: &[SensorReading], key: &str) -> Option<f64> {
        find_reading(readings, key).and_then(|r| r.value.as_f64())
    }

    fn text_value(readings: &[SensorReading], key: &str) -> Option<String> {
        find_reading(readings, key).and_then(|r| match &r.value {
            SensorValue::Text(s) => Some(s.clone()),
            _ => None,
        })
    }

    // --- file-nr tests ---

    #[test]
    fn parse_file_nr_standard() {
        let contents = "2048\t0\t1048576\n";
        let readings = SystemProvider::parse_file_nr(contents);
        assert_eq!(readings.len(), 1);
        assert_eq!(text_value(&readings, "_system_open_files_").unwrap(), "2048");
    }

    #[test]
    fn parse_file_nr_empty() {
        let readings = SystemProvider::parse_file_nr("");
        // Empty string: split('\t').next() returns Some(""), so we get a reading with empty text
        assert_eq!(readings.len(), 1);
        assert_eq!(text_value(&readings, "_system_open_files_").unwrap(), "");
    }

    // --- loadavg tests ---

    #[test]
    fn parse_loadavg_standard() {
        let contents = "0.52 0.38 0.41 3/1234 56789\n";
        let readings = SystemProvider::parse_loadavg(contents);

        let load1 = numeric_value(&readings, "_system_load_1m_").unwrap();
        assert!((load1 - 0.52).abs() < 1e-6);

        let load5 = numeric_value(&readings, "_system_load_5m_").unwrap();
        assert!((load5 - 0.38).abs() < 1e-6);

        let load15 = numeric_value(&readings, "_system_load_15m_").unwrap();
        assert!((load15 - 0.41).abs() < 1e-6);

        // Group summary should equal load1
        let group = numeric_value(&readings, "_system_group_").unwrap();
        assert!((group - 0.52).abs() < 1e-6);

        // Threads
        assert_eq!(text_value(&readings, "_system_threads_active_").unwrap(), "3");
        assert_eq!(text_value(&readings, "_system_threads_total_").unwrap(), "1234");
    }

    #[test]
    fn parse_loadavg_high_load() {
        let contents = "12.50 8.30 4.10 15/2000 99999\n";
        let readings = SystemProvider::parse_loadavg(contents);
        let load1 = numeric_value(&readings, "_system_load_1m_").unwrap();
        assert!((load1 - 12.50).abs() < 1e-6);
    }

    #[test]
    fn parse_loadavg_too_short() {
        let contents = "0.50 0.30\n";
        let readings = SystemProvider::parse_loadavg(contents);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_loadavg_empty() {
        let readings = SystemProvider::parse_loadavg("");
        assert!(readings.is_empty());
    }

    // --- uptime tests ---

    #[test]
    fn parse_uptime_standard() {
        let contents = "123456.78 234567.89\n";
        let readings = SystemProvider::parse_uptime(contents);
        assert_eq!(readings.len(), 1);
        let uptime = numeric_value(&readings, "_system_uptime_").unwrap();
        assert!((uptime - 123456.78).abs() < 1e-6);
    }

    #[test]
    fn parse_uptime_empty() {
        let readings = SystemProvider::parse_uptime("");
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_uptime_garbage() {
        let readings = SystemProvider::parse_uptime("not-a-number");
        assert!(readings.is_empty());
    }

    // --- version tests ---

    #[test]
    fn parse_version_standard() {
        let contents = "Linux version 6.8.0-40-generic (buildd@lcy02) (gcc ...)";
        let readings = SystemProvider::parse_version(contents);
        assert_eq!(readings.len(), 1);
        assert_eq!(
            text_value(&readings, "_system_kernel_").unwrap(),
            "6.8.0-40-generic"
        );
    }

    #[test]
    fn parse_version_too_short() {
        let contents = "Linux version";
        let readings = SystemProvider::parse_version(contents);
        assert!(readings.is_empty());
    }
}

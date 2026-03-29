use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct NetworkProvider {
    include_public_ip: bool,
    next_ip_check: f64,
    cached_public_ip: Option<String>,
}

impl NetworkProvider {
    pub fn new(include_public_ip: bool) -> Self {
        Self {
            include_public_ip,
            next_ip_check: 0.0,
            cached_public_ip: None,
        }
    }

    fn read_interface_bytes(iface: &str, direction: &str) -> Option<u64> {
        let path = format!("/sys/class/net/{iface}/statistics/{direction}_bytes");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    /// Parse /proc/net/wireless contents into WiFi quality/signal readings.
    pub fn parse_wireless(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Network;

        for (i, line) in contents.lines().enumerate() {
            // Skip the two header lines
            if i < 2 {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                if let Ok(quality) = parts[2].trim_end_matches('.').parse::<f64>() {
                    readings.push(SensorReading {
                        label: "WiFi Link Quality".to_string(),
                        value: SensorValue::Numeric(quality / 70.0),
                        category: cat.clone(),
                        format: SensorFormat::Percent,
                        key: "_network_wifi_quality_".to_string(),
                    });
                }
                if let Ok(signal) = parts[3].trim_end_matches('.').parse::<f64>() {
                    readings.push(SensorReading {
                        label: "WiFi Signal Level".to_string(),
                        value: SensorValue::Text(signal.to_string()),
                        category: cat.clone(),
                        format: SensorFormat::StringVal,
                        key: "_network_wifi_signal_".to_string(),
                    });
                }
            }
        }

        readings
    }
}

impl SensorProvider for NetworkProvider {
    fn query(&mut self, dwell: f64) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Network;

        // Read network interface statistics
        let net_dir = std::path::Path::new("/sys/class/net");
        if let Ok(entries) = std::fs::read_dir(net_dir) {
            for entry in entries.flatten() {
                let iface = entry.file_name().to_string_lossy().to_string();

                for direction in &["tx", "rx"] {
                    // Skip rx for loopback (tx and rx are identical)
                    if iface == "lo" && *direction == "rx" {
                        continue;
                    }

                    if let Some(bytes) = Self::read_interface_bytes(&iface, direction) {
                        let key = format!("_network_{iface}_{direction}_");
                        let label = if iface == "lo" {
                            iface.clone()
                        } else {
                            format!("{iface} {direction}")
                        };

                        // Store raw bytes for speed calculation (handled by change detector)
                        let _sensor_type = if iface == "lo" {
                            "network"
                        } else {
                            match *direction {
                                "rx" => "network-rx",
                                "tx" => "network-tx",
                                _ => "network",
                            }
                        };

                        readings.push(SensorReading {
                            label,
                            value: SensorValue::Numeric(bytes as f64),
                            category: cat.clone(),
                            format: SensorFormat::Storage,
                            key,
                        });
                    }
                }
            }
        }

        // WiFi signal quality from /proc/net/wireless
        if let Ok(contents) = std::fs::read_to_string("/proc/net/wireless") {
            readings.extend(Self::parse_wireless(&contents));
        }

        // Public IP check (every hour)
        if self.include_public_ip {
            if self.next_ip_check <= 0.0 {
                self.next_ip_check = 3600.0;

                // Attempt to fetch public IP (blocking, but fast enough for a simple HTTP call)
                match reqwest::blocking::get("https://ipv4.corecoding.com") {
                    Ok(resp) => {
                        if let Ok(text) = resp.text() {
                            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(ip) = obj.get("IPv4").and_then(|v| v.as_str()) {
                                    self.cached_public_ip = Some(ip.to_string());
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
            self.next_ip_check -= dwell;

            if let Some(ip) = &self.cached_public_ip {
                readings.push(SensorReading {
                    label: "Public IP".to_string(),
                    value: SensorValue::Text(ip.clone()),
                    category: cat,
                    format: SensorFormat::StringVal,
                    key: "_network_public_ip_".to_string(),
                });
            }
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Network
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

    const WIRELESS_SAMPLE: &str = "\
Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
 wlan0: 0000   52.  -58.  -256        0      0      0      0      0        0
";

    #[test]
    fn parse_wireless_standard() {
        let readings = NetworkProvider::parse_wireless(WIRELESS_SAMPLE);
        assert_eq!(readings.len(), 2);

        let quality = numeric_value(&readings, "_network_wifi_quality_").unwrap();
        // 52 / 70 = 0.7428...
        assert!((quality - 52.0 / 70.0).abs() < 1e-6);

        let signal = text_value(&readings, "_network_wifi_signal_").unwrap();
        assert_eq!(signal, "-58");
    }

    #[test]
    fn parse_wireless_max_quality() {
        let contents = "\
Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
 wlan0: 0000   70.  -30.  -256        0      0      0      0      0        0
";
        let readings = NetworkProvider::parse_wireless(contents);
        let quality = numeric_value(&readings, "_network_wifi_quality_").unwrap();
        assert!((quality - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_wireless_zero_quality() {
        let contents = "\
Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
 wlan0: 0000   0.  -90.  -256        0      0      0      0      0        0
";
        let readings = NetworkProvider::parse_wireless(contents);
        let quality = numeric_value(&readings, "_network_wifi_quality_").unwrap();
        assert!((quality - 0.0).abs() < 1e-6);
    }

    #[test]
    fn parse_wireless_empty() {
        let readings = NetworkProvider::parse_wireless("");
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_wireless_header_only() {
        let contents = "\
Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
";
        let readings = NetworkProvider::parse_wireless(contents);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_wireless_multiple_interfaces() {
        let contents = "\
Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
 wlan0: 0000   52.  -58.  -256        0      0      0      0      0        0
 wlan1: 0000   35.  -73.  -256        0      0      0      0      0        0
";
        let readings = NetworkProvider::parse_wireless(contents);
        // Two interfaces, each with quality + signal = 4 readings
        assert_eq!(readings.len(), 4);
    }
}

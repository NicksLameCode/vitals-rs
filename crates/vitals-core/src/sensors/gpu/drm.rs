use crate::sensors::{SensorCategory, SensorFormat, SensorReading, SensorValue};

/// A discovered DRM GPU card.
pub struct DrmCard {
    pub index: u8,
    pub vendor: String,
}

/// Discover GPU cards via /sys/class/drm/card*/device/vendor.
pub fn discover_drm_cards() -> Vec<DrmCard> {
    let mut cards = Vec::new();

    for i in 0..10u8 {
        let vendor_path = format!("/sys/class/drm/card{i}/device/vendor");
        if let Ok(vendor) = std::fs::read_to_string(&vendor_path) {
            cards.push(DrmCard {
                index: i,
                vendor: vendor.trim().to_string(),
            });
        }
    }

    cards
}

impl DrmCard {
    /// Query sensor readings for this DRM card.
    pub fn query(&self) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let gpu_num = self.index + 1;
        let type_name = format!("gpu#{gpu_num}");
        let cat = SensorCategory::Gpu(gpu_num);

        match self.vendor.as_str() {
            "0x1002" => {
                // AMD GPU
                let base = format!("/sys/class/drm/card{}/device", self.index);

                if let Ok(val) = read_trimmed_int(&format!("{base}/gpu_busy_percent")) {
                    readings.push(SensorReading {
                        label: "Graphics".to_string(),
                        value: SensorValue::Numeric(val as f64 * 0.01),
                        category: cat.clone(),
                        format: SensorFormat::Percent,
                        key: format!("_{type_name}_group_"),
                    });
                    readings.push(SensorReading {
                        label: "Vendor".to_string(),
                        value: SensorValue::Text("AMD".to_string()),
                        category: cat.clone(),
                        format: SensorFormat::StringVal,
                        key: format!("_{type_name}_vendor_"),
                    });
                    readings.push(SensorReading {
                        label: "Usage".to_string(),
                        value: SensorValue::Numeric(val as f64 * 0.01),
                        category: cat.clone(),
                        format: SensorFormat::Percent,
                        key: format!("_{type_name}_usage_"),
                    });
                }

                if let Ok(used) = read_trimmed_int(&format!("{base}/mem_info_vram_used")) {
                    readings.push(SensorReading {
                        label: "Memory Used".to_string(),
                        value: SensorValue::Numeric(used as f64 / 1000.0),
                        category: cat.clone(),
                        format: SensorFormat::Memory,
                        key: format!("_{type_name}_vram_used_"),
                    });
                }

                if let Ok(total) = read_trimmed_int(&format!("{base}/mem_info_vram_total")) {
                    readings.push(SensorReading {
                        label: "Memory Total".to_string(),
                        value: SensorValue::Numeric(total as f64 / 1000.0),
                        category: cat,
                        format: SensorFormat::Memory,
                        key: format!("_{type_name}_vram_total_"),
                    });
                }
            }
            _ => {
                // For other vendors, show basic info
                let vendor_name = match self.vendor.as_str() {
                    "0x10DE" => "NVIDIA",
                    "0x13B5" => "ARM",
                    "0x5143" => "Qualcomm",
                    "0x8086" => "Intel",
                    _ => &self.vendor,
                };

                readings.push(SensorReading {
                    label: "Graphics".to_string(),
                    value: SensorValue::Text(vendor_name.to_string()),
                    category: cat,
                    format: SensorFormat::StringVal,
                    key: format!("_{type_name}_group_"),
                });
            }
        }

        readings
    }
}

fn read_trimmed_int(path: &str) -> Result<u64, ()> {
    std::fs::read_to_string(path)
        .map_err(|_| ())
        .and_then(|s| s.trim().parse().map_err(|_| ()))
}

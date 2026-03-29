//! Shared hwmon discovery logic for temperature, voltage, and fan sensors.
//! Walks /sys/class/hwmon/ to find sensor input files and their labels.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::sensors::{SensorCategory, SensorFormat};

/// A discovered hardware monitor sensor.
#[derive(Debug, Clone)]
pub struct HwmonSensor {
    pub name: String,
    pub label: String,
    pub input_path: PathBuf,
    pub category: SensorCategory,
    pub format: SensorFormat,
    pub key: String,
}

/// Sensor type prefix mappings: (file_prefix, category, format).
const SENSOR_TYPES: &[(&str, SensorCategory, SensorFormat)] = &[
    ("temp", SensorCategory::Temperature, SensorFormat::Temp),
    ("in", SensorCategory::Voltage, SensorFormat::Voltage),
    ("fan", SensorCategory::Fan, SensorFormat::Fan),
];

/// Discover all hwmon sensors of the specified categories.
pub fn discover_hwmon_sensors(
    show_temp: bool,
    show_voltage: bool,
    show_fan: bool,
    hide_zeros: bool,
) -> Vec<HwmonSensor> {
    let hwbase = Path::new("/sys/class/hwmon");
    let mut sensors = Vec::new();
    let mut used_labels: HashMap<String, u32> = HashMap::new();

    let entries = match std::fs::read_dir(hwbase) {
        Ok(e) => e,
        Err(_) => return sensors,
    };

    for entry in entries.flatten() {
        let hwmon_dir = entry.path();
        let hwmon_name = entry.file_name().to_string_lossy().to_string();

        // Read the sensor module name
        let (sensor_name, base_path) = if let Ok(name) = read_trimmed(&hwmon_dir.join("name")) {
            (name, hwmon_dir.clone())
        } else if let Ok(name) = read_trimmed(&hwmon_dir.join("device/name")) {
            (name, hwmon_dir.join("device"))
        } else {
            continue;
        };

        // For coretemp, try to get the CPU package prefix from temp1_label
        let prefix = if sensor_name == "coretemp" {
            read_trimmed(&base_path.join("temp1_label")).unwrap_or_else(|_| sensor_name.clone())
        } else {
            sensor_name.clone()
        };

        // List all files in the hwmon directory
        let files = match std::fs::read_dir(&base_path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        // Group sensors by their numeric index
        let mut trisensors: HashMap<String, SensorFileGroup> = HashMap::new();

        for file_entry in files.flatten() {
            let filename = file_entry.file_name().to_string_lossy().to_string();

            for &(type_prefix, ref category, format) in SENSOR_TYPES {
                // Check if this sensor type is enabled
                let enabled = match category {
                    SensorCategory::Temperature => show_temp,
                    SensorCategory::Voltage => show_voltage,
                    SensorCategory::Fan => show_fan,
                    _ => false,
                };
                if !enabled {
                    continue;
                }

                // Match files like "temp1_input", "temp1_label", "in0_input", "fan1_input"
                if filename.starts_with(type_prefix) {
                    if filename.ends_with("_input") {
                        let key =
                            format!("{}{}", hwmon_name, &filename[..filename.len() - "_input".len()]);
                        let group = trisensors.entry(key).or_insert_with(|| SensorFileGroup {
                            category: category.clone(),
                            format,
                            input: None,
                            label: None,
                        });
                        group.input = Some(base_path.join(&filename));
                    } else if filename.ends_with("_label") {
                        let key =
                            format!("{}{}", hwmon_name, &filename[..filename.len() - "_label".len()]);
                        let group = trisensors.entry(key).or_insert_with(|| SensorFileGroup {
                            category: category.clone(),
                            format,
                            input: None,
                            label: None,
                        });
                        group.label = Some(base_path.join(&filename));
                    }
                }
            }
        }

        // Process discovered sensor file groups
        for (file_key, group) in &trisensors {
            let input_path = match &group.input {
                Some(p) => p,
                None => continue,
            };

            // Read current value to check for zeros
            let value = match read_trimmed(input_path) {
                Ok(v) => v.parse::<f64>().unwrap_or(0.0),
                Err(_) => continue,
            };

            // Hide zeros for temp/voltage but not for fans
            if hide_zeros
                && value == 0.0
                && group.category != SensorCategory::Fan
            {
                continue;
            }

            // Determine the label
            let label_text = if let Some(label_path) = &group.label {
                read_trimmed(label_path).unwrap_or_else(|_| prefix.clone())
            } else {
                // Extract the sensor number from the file key for the extra suffix
                let extra = input_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| {
                        f.split('_')
                            .next()
                            .unwrap_or("")
                            .to_string()
                    })
                    .unwrap_or_default();
                format!("{prefix} {extra}")
            };

            // Build composite label: "module_name label_text"
            let mut full_label = if sensor_name != label_text {
                format!("{sensor_name} {label_text}")
            } else {
                label_text.clone()
            };

            // Apply known label cleanups (matching GJS behavior)
            full_label = clean_label(&full_label);

            // Handle duplicate labels
            let dedup_key = format!("{}:{}", group.category, full_label);
            let count = used_labels.entry(dedup_key).or_insert(0);
            *count += 1;
            if *count > 1 {
                full_label = format!("{full_label} {count}");
            }

            let sensor_key = format!(
                "_{}_{}_{}_",
                group.category,
                file_key.replace('/', "_"),
                group.format.as_str()
            );

            sensors.push(HwmonSensor {
                name: sensor_name.clone(),
                label: full_label,
                input_path: input_path.clone(),
                category: group.category.clone(),
                format: group.format,
                key: sensor_key,
            });
        }
    }

    sensors
}

struct SensorFileGroup {
    category: SensorCategory,
    format: SensorFormat,
    input: Option<PathBuf>,
    label: Option<PathBuf>,
}

/// Apply known label transformations matching the GJS extension behavior.
fn clean_label(label: &str) -> String {
    let mut l = label.to_string();
    if l == "acpitz temp1" {
        l = "ACPI Thermal Zone".to_string();
    } else if l == "pch_cannonlake temp1" {
        l = "Platform Controller Hub".to_string();
    } else if l == "iwlwifi_1 temp1" {
        l = "Wireless Adapter".to_string();
    } else if l == "Package id 0" {
        l = "Processor 0".to_string();
    } else if l == "Package id 1" {
        l = "Processor 1".to_string();
    }
    l = l.replace("Package id", "CPU");
    l
}

/// Read a file and return its trimmed contents.
pub fn read_trimmed(path: &Path) -> std::io::Result<String> {
    let contents = std::fs::read_to_string(path)?;
    Ok(contents.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_label_acpitz() {
        assert_eq!(clean_label("acpitz temp1"), "ACPI Thermal Zone");
    }

    #[test]
    fn clean_label_pch_cannonlake() {
        assert_eq!(clean_label("pch_cannonlake temp1"), "Platform Controller Hub");
    }

    #[test]
    fn clean_label_iwlwifi() {
        assert_eq!(clean_label("iwlwifi_1 temp1"), "Wireless Adapter");
    }

    #[test]
    fn clean_label_package_id_0() {
        assert_eq!(clean_label("Package id 0"), "Processor 0");
    }

    #[test]
    fn clean_label_package_id_1() {
        assert_eq!(clean_label("Package id 1"), "Processor 1");
    }

    #[test]
    fn clean_label_package_id_generic() {
        // Package id 2 is not a special case, but the general replacement still applies
        assert_eq!(clean_label("Package id 2"), "CPU 2");
    }

    #[test]
    fn clean_label_passthrough() {
        // Unknown labels should pass through unchanged
        assert_eq!(clean_label("some random sensor"), "some random sensor");
    }

    #[test]
    fn clean_label_empty() {
        assert_eq!(clean_label(""), "");
    }

    #[test]
    fn read_trimmed_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_file");
        std::fs::write(&path, "  hello world  \n").unwrap();
        assert_eq!(read_trimmed(&path).unwrap(), "hello world");
    }

    #[test]
    fn read_trimmed_missing_file() {
        let path = Path::new("/tmp/nonexistent_hwmon_test_file_xyz");
        assert!(read_trimmed(path).is_err());
    }
}

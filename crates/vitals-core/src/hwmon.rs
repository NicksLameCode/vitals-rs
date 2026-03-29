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
    /// Canonical device path (excluding the hwmonN component), used for stable ordering
    /// when multiple instances of the same driver exist.
    pub device_path: String,
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

        // Resolve the canonical device path for stable ordering across reboots.
        // e.g., /sys/devices/pci0000:00/.../hwmon -> stable PCI/I2C topology path
        let device_path = std::fs::canonicalize(&hwmon_dir)
            .ok()
            .and_then(|p| p.parent().map(|pp| pp.to_string_lossy().to_string()))
            .unwrap_or_else(|| hwmon_dir.to_string_lossy().to_string());

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
                            format!("{}{}", sensor_name, &filename[..filename.len() - "_input".len()]);
                        let group = trisensors.entry(key).or_insert_with(|| SensorFileGroup {
                            category: category.clone(),
                            format,
                            input: None,
                            label: None,
                        });
                        group.input = Some(base_path.join(&filename));
                    } else if filename.ends_with("_label") {
                        let key =
                            format!("{}{}", sensor_name, &filename[..filename.len() - "_label".len()]);
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

            sensors.push(HwmonSensor {
                name: sensor_name.clone(),
                label: full_label,
                input_path: input_path.clone(),
                category: group.category.clone(),
                format: group.format,
                key: file_key.clone(),
                device_path: device_path.clone(),
            });
        }
    }

    dedup_sensor_keys(&mut sensors);

    sensors
}

/// Sort sensors deterministically and deduplicate keys that collide when
/// multiple instances of the same driver exist (e.g. two `spd5118` chips).
fn dedup_sensor_keys(sensors: &mut Vec<HwmonSensor>) {
    // Sort by (preliminary key, device_path) so the same physical device
    // always receives the same dedup suffix regardless of read_dir order.
    sensors.sort_by(|a, b| a.key.cmp(&b.key).then_with(|| a.device_path.cmp(&b.device_path)));

    // Append _N suffix for duplicate preliminary keys
    let mut key_counts: HashMap<String, u32> = HashMap::new();
    for sensor in sensors.iter_mut() {
        let count = key_counts.entry(sensor.key.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            sensor.key = format!("{}_{}", sensor.key, count);
        }
    }

    // Build final sensor_key from the (possibly deduped) file_key
    for sensor in sensors.iter_mut() {
        sensor.key = format!(
            "_{}_{}_{}_",
            sensor.category,
            sensor.key.replace('/', "_"),
            sensor.format.as_str()
        );
    }
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

    fn make_sensor(name: &str, file_key: &str, device_path: &str) -> HwmonSensor {
        HwmonSensor {
            name: name.into(),
            label: format!("{name} temp"),
            input_path: PathBuf::from(format!("{device_path}/temp1_input")),
            category: SensorCategory::Temperature,
            format: SensorFormat::Temp,
            key: file_key.into(),
            device_path: device_path.into(),
        }
    }

    #[test]
    fn dedup_single_driver_no_suffix() {
        let mut sensors = vec![make_sensor("coretemp", "coretemptemp1", "/sys/devices/pci/hwmon")];
        dedup_sensor_keys(&mut sensors);
        assert_eq!(sensors[0].key, "_temperature_coretemptemp1_temp_");
    }

    #[test]
    fn dedup_different_drivers_no_suffix() {
        let mut sensors = vec![
            make_sensor("coretemp", "coretemptemp1", "/sys/devices/pci-a/hwmon"),
            make_sensor("acpitz", "acpitztemp1", "/sys/devices/pci-b/hwmon"),
        ];
        dedup_sensor_keys(&mut sensors);
        assert!(sensors.iter().any(|s| s.key == "_temperature_coretemptemp1_temp_"));
        assert!(sensors.iter().any(|s| s.key == "_temperature_acpitztemp1_temp_"));
    }

    #[test]
    fn dedup_duplicate_drivers_get_suffix() {
        let mut sensors = vec![
            make_sensor("spd5118", "spd5118temp1", "/sys/devices/i2c/17-0050/hwmon"),
            make_sensor("spd5118", "spd5118temp1", "/sys/devices/i2c/17-0051/hwmon"),
        ];
        dedup_sensor_keys(&mut sensors);
        assert_eq!(sensors[0].key, "_temperature_spd5118temp1_temp_");
        assert_eq!(sensors[1].key, "_temperature_spd5118temp1_2_temp_");
    }

    #[test]
    fn dedup_stable_regardless_of_input_order() {
        let mut sensors_a = vec![
            make_sensor("spd5118", "spd5118temp1", "/sys/devices/i2c/17-0050/hwmon"),
            make_sensor("spd5118", "spd5118temp1", "/sys/devices/i2c/17-0051/hwmon"),
        ];
        let mut sensors_b = vec![
            make_sensor("spd5118", "spd5118temp1", "/sys/devices/i2c/17-0051/hwmon"),
            make_sensor("spd5118", "spd5118temp1", "/sys/devices/i2c/17-0050/hwmon"),
        ];
        dedup_sensor_keys(&mut sensors_a);
        dedup_sensor_keys(&mut sensors_b);

        let keys_a: Vec<_> = sensors_a.iter().map(|s| s.key.as_str()).collect();
        let keys_b: Vec<_> = sensors_b.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys_a, keys_b);
    }
}

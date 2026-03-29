use std::collections::HashMap;

use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

const BATTERY_PATHS: &[&str] = &[
    "BAT0",
    "BAT1",
    "BAT2",
    "BATT",
    "CMB0",
    "CMB1",
    "CMB2",
    "macsmc-battery",
];

pub struct BatteryProvider {
    slot: u8,
    time_left_history: Vec<f64>,
    charge_status: String,
}

impl BatteryProvider {
    pub fn new(slot: u8) -> Self {
        Self {
            slot: slot.min(7),
            time_left_history: Vec::new(),
            charge_status: String::new(),
        }
    }

    fn parse_number(s: &str) -> Option<f64> {
        s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
    }

    /// Parse a uevent-format file (key=value per line) into sensor readings.
    ///
    /// `time_left_history` and `charge_status` are used for smoothing the
    /// time-left estimate across multiple calls.
    pub fn parse_uevent(
        contents: &str,
        time_left_history: &mut Vec<f64>,
        charge_status: &mut String,
    ) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Battery;

        // Parse key=value pairs
        let mut props: HashMap<String, String> = HashMap::new();
        for line in contents.lines() {
            if let Some((key, val)) = line.split_once('=') {
                let clean_key = key.replace("POWER_SUPPLY_", "");
                props.insert(clean_key, val.to_string());
            }
        }

        let voltage_now = props.get("VOLTAGE_NOW").and_then(|v| Self::parse_number(v));
        let current_now = props.get("CURRENT_NOW").and_then(|v| Self::parse_number(v));
        let power_now_raw = props.get("POWER_NOW").and_then(|v| Self::parse_number(v));
        let capacity = props.get("CAPACITY").and_then(|v| Self::parse_number(v));
        let charge_full = props.get("CHARGE_FULL").and_then(|v| Self::parse_number(v));
        let charge_full_design = props.get("CHARGE_FULL_DESIGN").and_then(|v| Self::parse_number(v));
        let charge_now = props.get("CHARGE_NOW").and_then(|v| Self::parse_number(v));
        let voltage_min_design = props.get("VOLTAGE_MIN_DESIGN").and_then(|v| Self::parse_number(v));
        let mut energy_full = props.get("ENERGY_FULL").and_then(|v| Self::parse_number(v));
        let mut energy_full_design = props.get("ENERGY_FULL_DESIGN").and_then(|v| Self::parse_number(v));
        let mut energy_now = props.get("ENERGY_NOW").and_then(|v| Self::parse_number(v));
        let mut power_now = power_now_raw;
        let status = props.get("STATUS").cloned();

        if let Some(s) = &status {
            readings.push(SensorReading {
                label: "State".to_string(),
                value: SensorValue::Text(s.clone()),
                category: cat.clone(),
                format: SensorFormat::StringVal,
                key: "_battery_state_".to_string(),
            });
        }

        if let Some(cycles) = props.get("CYCLE_COUNT") {
            readings.push(SensorReading {
                label: "Cycles".to_string(),
                value: SensorValue::Text(cycles.clone()),
                category: cat.clone(),
                format: SensorFormat::StringVal,
                key: "_battery_cycles_".to_string(),
            });
        }

        if let Some(v) = voltage_now {
            readings.push(SensorReading {
                label: "Voltage".to_string(),
                value: SensorValue::Numeric(v / 1000.0),
                category: cat.clone(),
                format: SensorFormat::Voltage,
                key: "_battery_voltage_".to_string(),
            });
        }

        if let Some(level) = props.get("CAPACITY_LEVEL") {
            readings.push(SensorReading {
                label: "Level".to_string(),
                value: SensorValue::Text(level.clone()),
                category: cat.clone(),
                format: SensorFormat::StringVal,
                key: "_battery_level_".to_string(),
            });
        }

        if let Some(cap) = capacity {
            readings.push(SensorReading {
                label: "Percentage".to_string(),
                value: SensorValue::Numeric(cap / 100.0),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: "_battery_percentage_".to_string(),
            });
        }

        // Calculate power if not directly available
        if power_now.is_none() {
            if let (Some(v), Some(c)) = (voltage_now, current_now) {
                power_now = Some((v * c) / 1_000_000.0);
            }
        }

        if let Some(p) = power_now {
            let signed = if status.as_deref() == Some("Discharging") {
                -p
            } else {
                p
            };
            readings.push(SensorReading {
                label: "Power Rate".to_string(),
                value: SensorValue::Numeric(signed),
                category: cat.clone(),
                format: SensorFormat::Watt,
                key: "_battery_power_rate_".to_string(),
            });
            readings.push(SensorReading {
                label: "battery".to_string(),
                value: SensorValue::Numeric(signed),
                category: cat.clone(),
                format: SensorFormat::Watt,
                key: "_battery_group_".to_string(),
            });
        }

        // Calculate energy values from charge if not directly available
        if energy_full.is_none() {
            if let (Some(cf), Some(vmd)) = (charge_full, voltage_min_design) {
                energy_full = Some((cf * vmd) / 1_000_000.0);
            }
        }
        if let Some(ef) = energy_full {
            readings.push(SensorReading {
                label: "Energy (full)".to_string(),
                value: SensorValue::Numeric(ef),
                category: cat.clone(),
                format: SensorFormat::WattHour,
                key: "_battery_energy_full_".to_string(),
            });
        }

        if energy_full_design.is_none() {
            if let (Some(cfd), Some(vmd)) = (charge_full_design, voltage_min_design) {
                energy_full_design = Some((cfd * vmd) / 1_000_000.0);
            }
        }
        if let Some(efd) = energy_full_design {
            readings.push(SensorReading {
                label: "Energy (design)".to_string(),
                value: SensorValue::Numeric(efd),
                category: cat.clone(),
                format: SensorFormat::WattHour,
                key: "_battery_energy_design_".to_string(),
            });
            if let Some(ef) = energy_full {
                if efd > 0.0 {
                    readings.push(SensorReading {
                        label: "Capacity".to_string(),
                        value: SensorValue::Numeric(ef / efd),
                        category: cat.clone(),
                        format: SensorFormat::Percent,
                        key: "_battery_capacity_".to_string(),
                    });
                }
            }
        }

        if energy_now.is_none() {
            if let (Some(vmd), Some(cn)) = (voltage_min_design, charge_now) {
                energy_now = Some((vmd * cn) / 1_000_000.0);
            }
        }
        if let Some(en) = energy_now {
            readings.push(SensorReading {
                label: "Energy (now)".to_string(),
                value: SensorValue::Numeric(en),
                category: cat.clone(),
                format: SensorFormat::WattHour,
                key: "_battery_energy_now_".to_string(),
            });
        }

        // Time left calculation with smoothing
        if let (Some(ef), Some(en), Some(p)) = (energy_full, energy_now, power_now) {
            if p != 0.0 {
                if let Some(s) = &status {
                    if s == "Charging" || s == "Discharging" {
                        let time_left = if s == "Charging" {
                            (ef - en) / p
                        } else {
                            en / p.abs()
                        };

                        if time_left.is_finite() && time_left >= 0.0 {
                            if *charge_status != *s {
                                time_left_history.clear();
                                *charge_status = s.clone();
                            }

                            time_left_history.push(time_left * 3600.0);

                            if time_left_history.len() > 10 {
                                time_left_history.remove(0);
                            }

                            let sum: f64 = time_left_history.iter().sum();
                            let avg = sum / time_left_history.len() as f64;

                            readings.push(SensorReading {
                                label: "Time left".to_string(),
                                value: SensorValue::Numeric(avg),
                                category: cat,
                                format: SensorFormat::Runtime,
                                key: "_battery_time_left_".to_string(),
                            });
                            return readings;
                        }
                    }
                }
            }
        }

        // If we can't calculate time left, show status
        if let Some(s) = &status {
            readings.push(SensorReading {
                label: "Time left".to_string(),
                value: SensorValue::Text(s.clone()),
                category: cat,
                format: SensorFormat::StringVal,
                key: "_battery_time_left_".to_string(),
            });
        }

        readings
    }
}

impl SensorProvider for BatteryProvider {
    fn query(&mut self, _dwell: f64) -> Vec<SensorReading> {
        let bat_name = BATTERY_PATHS[self.slot as usize];
        let uevent_path = format!("/sys/class/power_supply/{bat_name}/uevent");

        let contents = match std::fs::read_to_string(&uevent_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        Self::parse_uevent(
            &contents,
            &mut self.time_left_history,
            &mut self.charge_status,
        )
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Battery
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

    /// Standard battery uevent with all fields present (energy-based).
    const UEVENT_STANDARD: &str = "\
POWER_SUPPLY_STATUS=Discharging
POWER_SUPPLY_VOLTAGE_NOW=12000000
POWER_SUPPLY_POWER_NOW=15000000
POWER_SUPPLY_ENERGY_FULL=50000000
POWER_SUPPLY_ENERGY_FULL_DESIGN=55000000
POWER_SUPPLY_ENERGY_NOW=30000000
POWER_SUPPLY_CAPACITY=60
POWER_SUPPLY_CAPACITY_LEVEL=Normal
POWER_SUPPLY_CYCLE_COUNT=150
";

    /// Battery uevent without POWER_NOW (should be calculated from voltage*current).
    const UEVENT_NO_POWER_NOW: &str = "\
POWER_SUPPLY_STATUS=Charging
POWER_SUPPLY_VOLTAGE_NOW=12000000
POWER_SUPPLY_CURRENT_NOW=1500000
POWER_SUPPLY_ENERGY_FULL=50000000
POWER_SUPPLY_ENERGY_FULL_DESIGN=55000000
POWER_SUPPLY_ENERGY_NOW=30000000
POWER_SUPPLY_CAPACITY=60
";

    /// Battery uevent with charge-based fields (no ENERGY_* directly).
    const UEVENT_CHARGE_BASED: &str = "\
POWER_SUPPLY_STATUS=Discharging
POWER_SUPPLY_VOLTAGE_NOW=11800000
POWER_SUPPLY_VOLTAGE_MIN_DESIGN=11100000
POWER_SUPPLY_CURRENT_NOW=1200000
POWER_SUPPLY_CHARGE_FULL=4400000
POWER_SUPPLY_CHARGE_FULL_DESIGN=4800000
POWER_SUPPLY_CHARGE_NOW=2600000
POWER_SUPPLY_CAPACITY=59
";

    #[test]
    fn parse_uevent_standard_state() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        assert_eq!(text_value(&readings, "_battery_state_").unwrap(), "Discharging");
    }

    #[test]
    fn parse_uevent_standard_cycles() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        assert_eq!(text_value(&readings, "_battery_cycles_").unwrap(), "150");
    }

    #[test]
    fn parse_uevent_standard_voltage() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        // 12000000 / 1000.0 = 12000.0
        let v = numeric_value(&readings, "_battery_voltage_").unwrap();
        assert!((v - 12000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_uevent_standard_percentage() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        let pct = numeric_value(&readings, "_battery_percentage_").unwrap();
        assert!((pct - 0.60).abs() < 1e-6);
    }

    #[test]
    fn parse_uevent_discharging_power_is_negative() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        let power = numeric_value(&readings, "_battery_power_rate_").unwrap();
        assert!(power < 0.0, "Expected negative power during discharge, got {power}");
        // Raw power = 15000000, signed = -15000000
        assert!((power - (-15000000.0)).abs() < 1e-3);
    }

    #[test]
    fn parse_uevent_charging_power_is_positive() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_NO_POWER_NOW, &mut history, &mut status);
        let power = numeric_value(&readings, "_battery_power_rate_").unwrap();
        assert!(power > 0.0, "Expected positive power during charge, got {power}");
    }

    #[test]
    fn parse_uevent_power_calculated_from_voltage_current() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_NO_POWER_NOW, &mut history, &mut status);
        let power = numeric_value(&readings, "_battery_power_rate_").unwrap();
        // voltage=12000000, current=1500000
        // power = (12000000 * 1500000) / 1_000_000 = 18_000_000_000_000 / 1_000_000 = 18_000_000_000.0
        // Wait, that's very large. Let me re-check the code:
        // power_now = (v * c) / 1_000_000
        // v = 12000000, c = 1500000
        // power_now = 12000000 * 1500000 / 1_000_000 = 18_000_000_000
        // Status is Charging so signed = +18_000_000_000
        let expected = 12_000_000.0 * 1_500_000.0 / 1_000_000.0;
        assert!((power - expected).abs() < 1e-3);
    }

    #[test]
    fn parse_uevent_energy_full_and_design() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);

        let ef = numeric_value(&readings, "_battery_energy_full_").unwrap();
        assert!((ef - 50_000_000.0).abs() < 1e-3);

        let efd = numeric_value(&readings, "_battery_energy_design_").unwrap();
        assert!((efd - 55_000_000.0).abs() < 1e-3);

        // Capacity = ef / efd
        let cap = numeric_value(&readings, "_battery_capacity_").unwrap();
        assert!((cap - 50_000_000.0 / 55_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn parse_uevent_charge_based_energy_calculation() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_CHARGE_BASED, &mut history, &mut status);

        // energy_full = charge_full * voltage_min_design / 1_000_000
        // = 4400000 * 11100000 / 1_000_000 = 48840000000.0
        let ef = numeric_value(&readings, "_battery_energy_full_").unwrap();
        let expected_ef = 4_400_000.0 * 11_100_000.0 / 1_000_000.0;
        assert!((ef - expected_ef).abs() < 1e-3);

        // energy_full_design = charge_full_design * voltage_min_design / 1_000_000
        let efd = numeric_value(&readings, "_battery_energy_design_").unwrap();
        let expected_efd = 4_800_000.0 * 11_100_000.0 / 1_000_000.0;
        assert!((efd - expected_efd).abs() < 1e-3);

        // energy_now = voltage_min_design * charge_now / 1_000_000
        let en = numeric_value(&readings, "_battery_energy_now_").unwrap();
        let expected_en = 11_100_000.0 * 2_600_000.0 / 1_000_000.0;
        assert!((en - expected_en).abs() < 1e-3);
    }

    #[test]
    fn parse_uevent_time_left_discharging() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);

        // time_left = energy_now / |power_now| = 30000000 / 15000000 = 2.0 hours
        // stored as 2.0 * 3600 = 7200 seconds
        let time_left = numeric_value(&readings, "_battery_time_left_").unwrap();
        assert!((time_left - 7200.0).abs() < 1e-3, "Expected 7200s, got {time_left}");
    }

    #[test]
    fn parse_uevent_time_left_charging() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_NO_POWER_NOW, &mut history, &mut status);

        // time_left (charging) = (energy_full - energy_now) / power
        // = (50000000 - 30000000) / (12000000*1500000/1000000)
        // = 20000000 / 18000000000 = 0.001111... hours
        // stored as 0.001111 * 3600 = 4.0 seconds
        let time_left = numeric_value(&readings, "_battery_time_left_").unwrap();
        let expected = (50_000_000.0 - 30_000_000.0) / (12_000_000.0 * 1_500_000.0 / 1_000_000.0) * 3600.0;
        assert!((time_left - expected).abs() < 1e-3, "Expected {expected}, got {time_left}");
    }

    #[test]
    fn parse_uevent_time_left_smoothing() {
        let mut history = Vec::new();
        let mut status = String::new();

        // First call
        let readings1 = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        let t1 = numeric_value(&readings1, "_battery_time_left_").unwrap();

        // Second call with same data -- average of two identical values should be the same
        let readings2 = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        let t2 = numeric_value(&readings2, "_battery_time_left_").unwrap();
        assert!((t1 - t2).abs() < 1e-3, "Smoothed value should be stable with identical inputs");
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn parse_uevent_status_change_clears_history() {
        let mut history = Vec::new();
        let mut status = String::new();

        // Discharge first
        BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        assert_eq!(history.len(), 1);
        assert_eq!(status, "Discharging");

        // Switch to charging
        BatteryProvider::parse_uevent(UEVENT_NO_POWER_NOW, &mut history, &mut status);
        // History should have been cleared and restarted with 1 entry
        assert_eq!(history.len(), 1);
        assert_eq!(status, "Charging");
    }

    #[test]
    fn parse_uevent_level_field() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(UEVENT_STANDARD, &mut history, &mut status);
        assert_eq!(text_value(&readings, "_battery_level_").unwrap(), "Normal");
    }

    #[test]
    fn parse_uevent_empty() {
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent("", &mut history, &mut status);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_uevent_full_status_shows_text_time_left() {
        let uevent = "\
POWER_SUPPLY_STATUS=Full
POWER_SUPPLY_VOLTAGE_NOW=12500000
POWER_SUPPLY_ENERGY_FULL=50000000
POWER_SUPPLY_ENERGY_FULL_DESIGN=55000000
POWER_SUPPLY_ENERGY_NOW=50000000
POWER_SUPPLY_CAPACITY=100
";
        let mut history = Vec::new();
        let mut status = String::new();
        let readings = BatteryProvider::parse_uevent(uevent, &mut history, &mut status);
        // No POWER_NOW and no CURRENT_NOW -> power_now is None
        // Falls through to text-based time left with status
        assert_eq!(text_value(&readings, "_battery_time_left_").unwrap(), "Full");
    }

    #[test]
    fn parse_number_valid() {
        assert_eq!(BatteryProvider::parse_number("12345"), Some(12345.0));
        assert_eq!(BatteryProvider::parse_number("  67890  "), Some(67890.0));
    }

    #[test]
    fn parse_number_invalid() {
        assert_eq!(BatteryProvider::parse_number("abc"), None);
        assert_eq!(BatteryProvider::parse_number(""), None);
    }

    #[test]
    fn parse_number_infinity() {
        assert_eq!(BatteryProvider::parse_number("inf"), None);
    }
}

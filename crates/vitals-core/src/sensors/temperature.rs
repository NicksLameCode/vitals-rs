use crate::hwmon::{self, HwmonSensor};
use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct TemperatureProvider {
    sensors: Vec<HwmonSensor>,
    discovered: bool,
}

impl TemperatureProvider {
    pub fn new() -> Self {
        Self {
            sensors: Vec::new(),
            discovered: false,
        }
    }
}

impl SensorProvider for TemperatureProvider {
    fn discover(&mut self) -> Vec<SensorReading> {
        let all = hwmon::discover_hwmon_sensors(true, false, false, false);
        self.sensors = all
            .into_iter()
            .filter(|s| s.category == SensorCategory::Temperature)
            .collect();
        self.discovered = true;

        // Return initial readings
        self.query(0.0)
    }

    fn query(&mut self, _dwell: f64) -> Vec<SensorReading> {
        let mut readings = Vec::new();

        for sensor in &self.sensors {
            match hwmon::read_trimmed(&sensor.input_path) {
                Ok(val_str) => {
                    if let Ok(value) = val_str.parse::<f64>() {
                        readings.push(SensorReading {
                            label: sensor.label.clone(),
                            value: SensorValue::Numeric(value),
                            category: SensorCategory::Temperature,
                            format: SensorFormat::Temp,
                            key: sensor.key.clone(),
                        });
                    }
                }
                Err(_) => {
                    readings.push(SensorReading {
                        label: sensor.label.clone(),
                        value: SensorValue::Disabled,
                        category: SensorCategory::Temperature,
                        format: SensorFormat::Temp,
                        key: sensor.key.clone(),
                    });
                }
            }
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Temperature
    }
}

use crate::hwmon::{self, HwmonSensor};
use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct FanProvider {
    sensors: Vec<HwmonSensor>,
}

impl FanProvider {
    pub fn new() -> Self {
        Self {
            sensors: Vec::new(),
        }
    }
}

impl SensorProvider for FanProvider {
    fn discover(&mut self) -> Vec<SensorReading> {
        let all = hwmon::discover_hwmon_sensors(false, false, true, false);
        self.sensors = all
            .into_iter()
            .filter(|s| s.category == SensorCategory::Fan)
            .collect();
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
                            category: SensorCategory::Fan,
                            format: SensorFormat::Fan,
                            key: sensor.key.clone(),
                        });
                    }
                }
                Err(_) => {
                    readings.push(SensorReading {
                        label: sensor.label.clone(),
                        value: SensorValue::Disabled,
                        category: SensorCategory::Fan,
                        format: SensorFormat::Fan,
                        key: sensor.key.clone(),
                    });
                }
            }
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Fan
    }
}

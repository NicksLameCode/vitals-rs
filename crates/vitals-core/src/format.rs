use crate::config::AppConfig;
use crate::sensors::SensorFormat;

/// Formats raw sensor values into human-readable strings.
/// Direct port of values.js `_legible()`.
pub struct ValueFormatter<'a> {
    config: &'a AppConfig,
}

const DECIMAL_UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
const BINARY_UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"];
const HERTZ_UNITS: &[&str] = &["Hz", "KHz", "MHz", "GHz", "THz", "PHz", "EHz", "ZHz"];

impl<'a> ValueFormatter<'a> {
    pub fn new(config: &'a AppConfig) -> Self {
        Self { config }
    }

    /// Format a raw sensor value into a human-readable string.
    pub fn format(&self, value: f64, format: SensorFormat) -> String {
        let hp = self.config.general.use_higher_precision;

        match format {
            SensorFormat::Percent => {
                let v = (value * 100.0).min(100.0);
                if hp {
                    format!("{v:.1}%")
                } else {
                    format!("{:.0}%", v)
                }
            }

            SensorFormat::Temp => {
                let mut v = value / 1000.0;
                let unit = if self.config.temperature.unit == 1 {
                    v = (9.0 / 5.0) * v + 32.0;
                    "\u{00b0}F"
                } else {
                    "\u{00b0}C"
                };
                if hp {
                    format!("{v:.1}{unit}")
                } else {
                    format!("{:.0}{unit}", v)
                }
            }

            SensorFormat::Fan => {
                format!("{:.0} RPM", value)
            }

            SensorFormat::Voltage => {
                let v = value / 1000.0;
                let sign = if v >= 0.0 { "+" } else { "-" };
                if hp {
                    format!("{sign}{:.2} V", v.abs())
                } else {
                    format!("{sign}{:.1} V", v.abs())
                }
            }

            SensorFormat::Hertz => {
                let (v, exp) = scale_value(value, 1000.0);
                let unit = HERTZ_UNITS.get(exp).unwrap_or(&"Hz");
                if hp {
                    format!("{v:.2} {unit}")
                } else {
                    format!("{v:.1} {unit}")
                }
            }

            SensorFormat::Memory => {
                let use_decimal = self.config.memory.measurement == 1;
                let base = if use_decimal { 1000.0 } else { 1024.0 };
                let units = if use_decimal {
                    DECIMAL_UNITS
                } else {
                    BINARY_UNITS
                };
                // Memory values from /proc/meminfo are in KiB, so multiply by base
                let adjusted = value * base;
                let (v, exp) = scale_value(adjusted, base);
                let unit = units.get(exp).unwrap_or(&"B");
                if hp {
                    format!("{v:.2} {unit}")
                } else {
                    format!("{v:.1} {unit}")
                }
            }

            SensorFormat::Storage => {
                let use_decimal = self.config.storage.measurement == 1;
                let base = if use_decimal { 1000.0 } else { 1024.0 };
                let units = if use_decimal {
                    DECIMAL_UNITS
                } else {
                    BINARY_UNITS
                };
                let (v, exp) = scale_value(value, base);
                let unit = units.get(exp).unwrap_or(&"B");
                if hp {
                    format!("{v:.2} {unit}")
                } else {
                    format!("{v:.1} {unit}")
                }
            }

            SensorFormat::Speed => {
                let use_bps = self.config.network.speed_format == 1;
                let adjusted = if use_bps { value * 8.0 } else { value };
                let (v, exp) = scale_value(adjusted, 1000.0);
                let unit = if use_bps {
                    DECIMAL_UNITS
                        .get(exp)
                        .unwrap_or(&"B")
                        .replace('B', "bps")
                } else {
                    format!("{}/s", DECIMAL_UNITS.get(exp).unwrap_or(&"B"))
                };
                if hp {
                    format!("{v:.1} {unit}")
                } else {
                    format!("{v:.0} {unit}")
                }
            }

            SensorFormat::Uptime => format_duration(value, hp),

            SensorFormat::Runtime => format_duration(value, false),

            SensorFormat::Milliamp => {
                let v = value / 1000.0;
                if hp {
                    format!("{v:.1} mA")
                } else {
                    format!("{:.0} mA", v)
                }
            }

            SensorFormat::MilliampHour => {
                let v = value / 1000.0;
                if hp {
                    format!("{v:.1} mAh")
                } else {
                    format!("{:.0} mAh", v)
                }
            }

            SensorFormat::Watt => {
                let v = value / 1_000_000.0;
                let sign = if v > 0.0 { "+" } else { "" };
                if hp {
                    format!("{sign}{v:.2} W")
                } else {
                    format!("{sign}{v:.1} W")
                }
            }

            SensorFormat::WattGpu => {
                if hp {
                    format!("{value:.2} W")
                } else {
                    format!("{value:.1} W")
                }
            }

            SensorFormat::WattHour => {
                let v = value / 1_000_000.0;
                if hp {
                    format!("{v:.2} Wh")
                } else {
                    format!("{v:.1} Wh")
                }
            }

            SensorFormat::Load => {
                if hp {
                    format!("{value:.2}")
                } else {
                    format!("{value:.1}")
                }
            }

            SensorFormat::Pcie => {
                // PCIe format handled as string passthrough
                format!("PCIe {value}")
            }

            SensorFormat::StringVal => value.to_string(),
        }
    }

    /// Format a string sensor value (passthrough for non-numeric values).
    pub fn format_string(&self, value: &str) -> String {
        value.to_string()
    }
}

/// Scale a value by a base unit, returning (scaled_value, exponent).
fn scale_value(value: f64, base: f64) -> (f64, usize) {
    if value <= 0.0 {
        return (0.0, 0);
    }

    let mut exp = (value.ln() / base.ln()).floor() as usize;
    let threshold = base.powi(exp as i32) * (base - 0.05);
    if value >= threshold {
        exp += 1;
    }
    let scaled = value / base.powi(exp as i32);
    (scaled, exp)
}

/// Format a duration in seconds to a human-readable string like "2d 3h 15m".
pub fn format_duration(seconds: f64, show_seconds: bool) -> String {
    let total = seconds.abs() as u64;

    let days = total / 86400;
    let hours = (total % 86400) / 3600;
    let mins = (total % 3600) / 60;
    let secs = total % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if mins > 0 {
        parts.push(format!("{mins}m"));
    }
    if (show_seconds || total < 60) && parts.is_empty() || (show_seconds && secs > 0) {
        parts.push(format!("{secs}s"));
    }

    if parts.is_empty() {
        "0s".to_string()
    } else {
        parts.join(" ")
    }
}

/// Format a duration for the history graph axis labels.
pub fn format_duration_short(seconds: f64) -> String {
    let total = seconds.abs().round() as u64;
    if total < 60 {
        return format!("{total}s");
    }
    let m = total / 60;
    if m < 60 {
        return format!("{m}m");
    }
    let h = m / 60;
    let rm = m % 60;
    if rm == 0 {
        format!("{h}h")
    } else {
        format!("{h}h {rm}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> AppConfig {
        AppConfig::default()
    }

    fn hp_config() -> AppConfig {
        let mut c = AppConfig::default();
        c.general.use_higher_precision = true;
        c
    }

    #[test]
    fn test_percent() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(0.45, SensorFormat::Percent), "45%");
        assert_eq!(f.format(1.5, SensorFormat::Percent), "100%");
    }

    #[test]
    fn test_percent_hp() {
        let cfg = hp_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(0.456, SensorFormat::Percent), "45.6%");
    }

    #[test]
    fn test_temp_celsius() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(45000.0, SensorFormat::Temp), "45\u{00b0}C");
    }

    #[test]
    fn test_temp_fahrenheit() {
        let mut cfg = default_config();
        cfg.temperature.unit = 1;
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(100_000.0, SensorFormat::Temp), "212\u{00b0}F");
    }

    #[test]
    fn test_fan() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(1200.0, SensorFormat::Fan), "1200 RPM");
    }

    #[test]
    fn test_voltage() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(12000.0, SensorFormat::Voltage), "+12.0 V");
        assert_eq!(f.format(-5000.0, SensorFormat::Voltage), "-5.0 V");
    }

    #[test]
    fn test_hertz() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(3_500_000_000.0, SensorFormat::Hertz), "3.5 GHz");
    }

    #[test]
    fn test_memory_decimal() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        // 8_000_000 KiB * 1000 = 8e9 bytes -> 8.0 GB
        assert_eq!(f.format(8_000_000.0, SensorFormat::Memory), "8.0 GB");
    }

    #[test]
    fn test_memory_binary() {
        let mut cfg = default_config();
        cfg.memory.measurement = 0;
        let f = ValueFormatter::new(&cfg);
        // 1_048_576 KiB * 1024 = 1 GiB
        assert_eq!(f.format(1_048_576.0, SensorFormat::Memory), "1.0 GiB");
    }

    #[test]
    fn test_watt() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        // 15_000_000 microwatts = 15W
        assert_eq!(f.format(15_000_000.0, SensorFormat::Watt), "+15.0 W");
        assert_eq!(f.format(-10_000_000.0, SensorFormat::Watt), "-10.0 W");
    }

    #[test]
    fn test_uptime() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(90061.0, SensorFormat::Uptime), "1d 1h 1m");
    }

    #[test]
    fn test_load() {
        let cfg = default_config();
        let f = ValueFormatter::new(&cfg);
        assert_eq!(f.format(1.23, SensorFormat::Load), "1.2");
    }

    #[test]
    fn test_format_duration_short() {
        assert_eq!(format_duration_short(30.0), "30s");
        assert_eq!(format_duration_short(120.0), "2m");
        assert_eq!(format_duration_short(3600.0), "1h");
        assert_eq!(format_duration_short(3660.0), "1h 1m");
    }
}

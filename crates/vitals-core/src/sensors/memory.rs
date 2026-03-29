use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct MemoryProvider;

impl MemoryProvider {
    pub fn new() -> Self {
        Self
    }

    /// Parse /proc/meminfo contents into sensor readings.
    pub fn parse_meminfo(contents: &str) -> Vec<SensorReading> {
        let mut total: f64 = 0.0;
        let mut avail: f64 = 0.0;
        let mut swap_total: f64 = 0.0;
        let mut swap_free: f64 = 0.0;
        let mut cached: f64 = 0.0;
        let mut mem_free: f64 = 0.0;

        for line in contents.lines() {
            if let Some(v) = parse_meminfo_value(line, "MemTotal:") {
                total = v;
            } else if let Some(v) = parse_meminfo_value(line, "MemAvailable:") {
                avail = v;
            } else if let Some(v) = parse_meminfo_value(line, "SwapTotal:") {
                swap_total = v;
            } else if let Some(v) = parse_meminfo_value(line, "SwapFree:") {
                swap_free = v;
            } else if let Some(v) = parse_meminfo_value(line, "Cached:") {
                // Only match "Cached:", not "SwapCached:"
                if line.starts_with("Cached:") {
                    cached = v;
                }
            } else if let Some(v) = parse_meminfo_value(line, "MemFree:") {
                mem_free = v;
            }
        }

        let used = total - avail;
        let utilized = if total > 0.0 { used / total } else { 0.0 };
        let swap_used = swap_total - swap_free;
        let swap_utilized = if swap_total > 0.0 {
            swap_used / swap_total
        } else {
            0.0
        };

        let cat = SensorCategory::Memory;
        vec![
            reading("Usage", utilized, cat.clone(), SensorFormat::Percent, "_memory_usage_"),
            reading("memory", utilized, SensorCategory::Memory, SensorFormat::Percent, "_memory_group_"),
            reading("Physical", total, cat.clone(), SensorFormat::Memory, "_memory_physical_"),
            reading("Available", avail, cat.clone(), SensorFormat::Memory, "_memory_available_"),
            reading("Allocated", used, cat.clone(), SensorFormat::Memory, "_memory_allocated_"),
            reading("Cached", cached, cat.clone(), SensorFormat::Memory, "_memory_cached_"),
            reading("Free", mem_free, cat.clone(), SensorFormat::Memory, "_memory_free_"),
            reading("Swap Total", swap_total, cat.clone(), SensorFormat::Memory, "_memory_swap_total_"),
            reading("Swap Free", swap_free, cat.clone(), SensorFormat::Memory, "_memory_swap_free_"),
            reading("Swap Used", swap_used, cat.clone(), SensorFormat::Memory, "_memory_swap_used_"),
            reading("Swap Usage", swap_utilized, cat, SensorFormat::Percent, "_memory_swap_usage_"),
        ]
    }
}

impl SensorProvider for MemoryProvider {
    fn query(&mut self, _dwell: f64) -> Vec<SensorReading> {
        match std::fs::read_to_string("/proc/meminfo") {
            Ok(contents) => Self::parse_meminfo(&contents),
            Err(_) => Vec::new(),
        }
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Memory
    }
}

/// Parse a line like "MemTotal:       16384000 kB" and return the numeric value.
fn parse_meminfo_value(line: &str, key: &str) -> Option<f64> {
    if !line.starts_with(key) {
        return None;
    }
    let rest = line[key.len()..].trim();
    let num_str = rest.split_whitespace().next()?;
    num_str.parse::<f64>().ok()
}

fn reading(
    label: &str,
    value: f64,
    category: SensorCategory,
    format: SensorFormat,
    key: &str,
) -> SensorReading {
    SensorReading {
        label: label.to_string(),
        value: SensorValue::Numeric(value),
        category,
        format,
        key: key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MEMINFO: &str = "\
MemTotal:       16244256 kB
MemFree:          524288 kB
MemAvailable:    8122128 kB
Buffers:          262144 kB
Cached:          4194304 kB
SwapCached:        65536 kB
SwapTotal:       8388608 kB
SwapFree:        7340032 kB
";

    #[test]
    fn test_parse_meminfo() {
        let readings = MemoryProvider::parse_meminfo(SAMPLE_MEMINFO);
        assert_eq!(readings.len(), 11);

        // Usage = (total - avail) / total = (16244256 - 8122128) / 16244256
        let usage = &readings[0];
        assert_eq!(usage.label, "Usage");
        if let SensorValue::Numeric(v) = usage.value {
            assert!((v - 0.5).abs() < 0.01);
        } else {
            panic!("Expected numeric value");
        }

        // Physical = total
        let physical = &readings[2];
        assert_eq!(physical.label, "Physical");
        if let SensorValue::Numeric(v) = physical.value {
            assert!((v - 16244256.0).abs() < 1.0);
        }
    }
}

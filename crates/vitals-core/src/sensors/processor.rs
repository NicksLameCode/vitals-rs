use std::collections::HashMap;

use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct ProcessorProvider {
    last_core_totals: HashMap<String, u64>,
    last_speeds: Vec<f64>,
    uses_cpu_info: bool,
    include_static_info: bool,
    static_returned: bool,
}

struct CpuStats {
    user: u64,
    nice: u64,
    system: u64,
}

impl ProcessorProvider {
    pub fn new(include_static_info: bool) -> Self {
        Self {
            last_core_totals: HashMap::new(),
            last_speeds: Vec::new(),
            uses_cpu_info: true, // assume true until we detect cpufreq
            include_static_info,
            static_returned: false,
        }
    }

    /// Parse /proc/stat contents and compute CPU usage deltas.
    ///
    /// `last_totals` is updated in-place with the new totals. On the first call
    /// (when `last_totals` is empty or a core has no previous entry), no usage
    /// readings are emitted for that core.
    ///
    /// Returns (readings, core_count).
    pub fn parse_proc_stat(
        contents: &str,
        last_totals: &mut HashMap<String, u64>,
        dwell: f64,
    ) -> (Vec<SensorReading>, usize) {
        let mut readings = Vec::new();
        let cat = SensorCategory::Processor;

        let mut statistics: HashMap<String, CpuStats> = HashMap::new();

        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("cpu") {
                let trimmed = format!("cpu{rest}");
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 5 {
                    let cpu_name = parts[0].to_string();
                    let values: Vec<u64> = parts[1..]
                        .iter()
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if values.len() >= 3 {
                        statistics.insert(
                            cpu_name,
                            CpuStats {
                                user: values[0],
                                nice: values[1],
                                system: values[2],
                            },
                        );
                    }
                }
            }
        }

        let cores = statistics.len().saturating_sub(1);

        for (cpu, stats) in &statistics {
            let total = stats.user + stats.nice + stats.system;

            if let Some(&prev_total) = last_totals.get(cpu) {
                if prev_total > 0 {
                    let sample_dwell = if dwell > 0.0 { dwell } else { 1.0 };
                    let delta = (total as f64 - prev_total as f64) / sample_dwell;

                    if cpu == "cpu" {
                        if cores > 0 {
                            let usage = delta / cores as f64 / 100.0;
                            readings.push(SensorReading {
                                label: "processor".to_string(),
                                value: SensorValue::Numeric(usage),
                                category: cat.clone(),
                                format: SensorFormat::Percent,
                                key: "_processor_group_".to_string(),
                            });
                            readings.push(SensorReading {
                                label: "Usage".to_string(),
                                value: SensorValue::Numeric(usage),
                                category: cat.clone(),
                                format: SensorFormat::Percent,
                                key: "_processor_usage_".to_string(),
                            });
                        }
                    } else {
                        let core_num = cpu.strip_prefix("cpu").unwrap_or("0");
                        readings.push(SensorReading {
                            label: format!("Core {core_num}"),
                            value: SensorValue::Numeric(delta / 100.0),
                            category: cat.clone(),
                            format: SensorFormat::Percent,
                            key: format!("_processor_core{core_num}_"),
                        });
                    }
                }
            }

            last_totals.insert(cpu.clone(), total);
        }

        (readings, cores)
    }

    /// Parse /proc/cpuinfo to extract MHz frequency values.
    /// Returns frequencies in Hz.
    pub fn parse_cpuinfo_frequencies(contents: &str) -> Vec<f64> {
        let mut freqs = Vec::new();
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("cpu MHz") {
                if let Some(val_str) = rest.split(':').nth(1) {
                    if let Ok(mhz) = val_str.trim().parse::<f64>() {
                        freqs.push(mhz * 1_000_000.0);
                    }
                }
            }
        }
        freqs
    }

    /// Build frequency readings (avg, max, min) from a set of Hz values.
    pub fn frequency_readings(speeds: &[f64]) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Processor;

        if !speeds.is_empty() {
            let sum: f64 = speeds.iter().sum();
            let avg = sum / speeds.len() as f64;
            readings.push(SensorReading {
                label: "Frequency".to_string(),
                value: SensorValue::Numeric(avg),
                category: cat.clone(),
                format: SensorFormat::Hertz,
                key: "_processor_frequency_".to_string(),
            });

            let max = speeds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            readings.push(SensorReading {
                label: "Max frequency".to_string(),
                value: SensorValue::Numeric(max),
                category: cat.clone(),
                format: SensorFormat::Hertz,
                key: "_processor_max_frequency_".to_string(),
            });

            let min = speeds.iter().cloned().fold(f64::INFINITY, f64::min);
            readings.push(SensorReading {
                label: "Min frequency".to_string(),
                value: SensorValue::Numeric(min),
                category: cat,
                format: SensorFormat::Hertz,
                key: "_processor_min_frequency_".to_string(),
            });
        }

        readings
    }
}

impl SensorProvider for ProcessorProvider {
    fn discover(&mut self) -> Vec<SensorReading> {
        // Check if CPU frequency scaling is available
        if std::path::Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq").exists() {
            self.uses_cpu_info = false;
        }
        Vec::new()
    }

    fn query(&mut self, dwell: f64) -> Vec<SensorReading> {
        let mut readings = Vec::new();

        // Parse /proc/stat for CPU usage
        if let Ok(contents) = std::fs::read_to_string("/proc/stat") {
            let (stat_readings, cores) =
                Self::parse_proc_stat(&contents, &mut self.last_core_totals, dwell);
            readings.extend(stat_readings);

            // Read CPU frequencies
            if !self.uses_cpu_info {
                self.last_speeds.clear();
                for core in 0..cores {
                    let path = format!(
                        "/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_cur_freq"
                    );
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        if let Ok(freq) = contents.trim().parse::<f64>() {
                            self.last_speeds.push(freq);
                        }
                    }
                }
            }
        }

        // Report frequencies
        let speeds = if self.uses_cpu_info {
            if let Ok(contents) = std::fs::read_to_string("/proc/cpuinfo") {
                Self::parse_cpuinfo_frequencies(&contents)
            } else {
                Vec::new()
            }
        } else {
            // cpufreq values are in KHz, convert to Hz
            self.last_speeds.iter().map(|s| s * 1000.0).collect()
        };

        readings.extend(Self::frequency_readings(&speeds));

        // Static CPU info (first time only)
        if self.include_static_info && !self.static_returned {
            self.static_returned = true;
            if let Ok(contents) = std::fs::read_to_string("/proc/cpuinfo") {
                let cat = SensorCategory::Processor;
                let mut vendor_id = String::new();
                let mut bogomips = String::new();
                let mut sockets: HashMap<String, bool> = HashMap::new();
                let mut cache = String::new();

                for line in contents.lines() {
                    if let Some(rest) = line.strip_prefix("vendor_id") {
                        if let Some(val) = rest.split(':').nth(1) {
                            vendor_id = val.trim().to_string();
                        }
                    } else if let Some(rest) = line.strip_prefix("bogomips") {
                        if let Some(val) = rest.split(':').nth(1) {
                            bogomips = val.trim().to_string();
                        }
                    } else if let Some(rest) = line.strip_prefix("physical id") {
                        if let Some(val) = rest.split(':').nth(1) {
                            sockets.insert(val.trim().to_string(), true);
                        }
                    } else if let Some(rest) = line.strip_prefix("cache size") {
                        if let Some(val) = rest.split(':').nth(1) {
                            let val = val.trim().replace(" KB", "");
                            if let Ok(kb) = val.parse::<f64>() {
                                cache = kb.to_string();
                            }
                        }
                    }
                }

                if !vendor_id.is_empty() {
                    readings.push(SensorReading {
                        label: "Vendor".to_string(),
                        value: SensorValue::Text(vendor_id),
                        category: cat.clone(),
                        format: SensorFormat::StringVal,
                        key: "_processor_vendor_".to_string(),
                    });
                }
                if !bogomips.is_empty() {
                    readings.push(SensorReading {
                        label: "Bogomips".to_string(),
                        value: SensorValue::Text(bogomips),
                        category: cat.clone(),
                        format: SensorFormat::StringVal,
                        key: "_processor_bogomips_".to_string(),
                    });
                }
                if !sockets.is_empty() {
                    readings.push(SensorReading {
                        label: "Sockets".to_string(),
                        value: SensorValue::Text(sockets.len().to_string()),
                        category: cat.clone(),
                        format: SensorFormat::StringVal,
                        key: "_processor_sockets_".to_string(),
                    });
                }
                if !cache.is_empty() {
                    readings.push(SensorReading {
                        label: "Cache".to_string(),
                        value: SensorValue::Numeric(cache.parse().unwrap_or(0.0)),
                        category: cat,
                        format: SensorFormat::Memory,
                        key: "_processor_cache_".to_string(),
                    });
                }
            }

            // Process time from uptime
            if let Ok(contents) = std::fs::read_to_string("/proc/uptime") {
                let parts: Vec<&str> = contents.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let (Ok(uptime), Ok(idle)) =
                        (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                    {
                        let cores = self.last_core_totals.len().saturating_sub(1).max(1);
                        let process_time = uptime - idle / cores as f64;
                        readings.push(SensorReading {
                            label: "Process Time".to_string(),
                            value: SensorValue::Numeric(process_time),
                            category: SensorCategory::Processor,
                            format: SensorFormat::Uptime,
                            key: "_processor_process_time_".to_string(),
                        });
                    }
                }
            }
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Processor
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

    const PROC_STAT_SAMPLE: &str = "\
cpu  10000 500 3000 80000 200 0 100 0 0 0
cpu0 2500 125 750 20000 50 0 25 0 0 0
cpu1 2500 125 750 20000 50 0 25 0 0 0
cpu2 2500 125 750 20000 50 0 25 0 0 0
cpu3 2500 125 750 20000 50 0 25 0 0 0
intr 123456
";

    const PROC_STAT_SAMPLE2: &str = "\
cpu  10500 520 3100 80500 210 0 110 0 0 0
cpu0 2600 130 780 20100 52 0 28 0 0 0
cpu1 2650 130 770 20100 53 0 27 0 0 0
cpu2 2600 130 775 20150 52 0 27 0 0 0
cpu3 2650 130 775 20150 53 0 28 0 0 0
intr 123789
";

    #[test]
    fn parse_proc_stat_first_call_no_readings() {
        let mut last_totals = HashMap::new();
        let (readings, cores) =
            ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE, &mut last_totals, 1.0);
        // First call: no previous data, so no usage readings
        assert!(readings.is_empty());
        // But the totals should be stored
        assert!(!last_totals.is_empty());
        assert_eq!(cores, 4);
    }

    #[test]
    fn parse_proc_stat_second_call_produces_readings() {
        let mut last_totals = HashMap::new();
        // First call seeds the totals
        let _ = ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE, &mut last_totals, 1.0);

        // Second call with new data should produce delta readings
        let (readings, cores) =
            ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE2, &mut last_totals, 1.0);
        assert_eq!(cores, 4);

        // Should have processor group + usage + 4 per-core readings = 6
        assert!(readings.len() >= 6, "Expected at least 6 readings, got {}", readings.len());

        // Check aggregate usage reading exists
        let usage = numeric_value(&readings, "_processor_usage_");
        assert!(usage.is_some(), "Missing aggregate usage reading");

        // Check per-core readings exist
        for i in 0..4 {
            let key = format!("_processor_core{i}_");
            assert!(
                find_reading(&readings, &key).is_some(),
                "Missing core {i} reading"
            );
        }
    }

    #[test]
    fn parse_proc_stat_usage_delta_calculation() {
        let mut last_totals = HashMap::new();
        let _ = ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE, &mut last_totals, 1.0);
        let (readings, _) =
            ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE2, &mut last_totals, 1.0);

        // Aggregate: old total = 10000+500+3000 = 13500, new total = 10500+520+3100 = 14120
        // delta = (14120-13500)/1.0 = 620
        // usage = 620 / 4 / 100 = 1.55
        let usage = numeric_value(&readings, "_processor_usage_").unwrap();
        assert!((usage - 1.55).abs() < 1e-6, "Usage was {usage}, expected 1.55");
    }

    #[test]
    fn parse_proc_stat_respects_dwell() {
        let mut last_totals = HashMap::new();
        let _ = ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE, &mut last_totals, 2.0);
        let (readings, _) =
            ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE2, &mut last_totals, 2.0);

        // With dwell=2.0: delta = 620/2.0 = 310, usage = 310/4/100 = 0.775
        let usage = numeric_value(&readings, "_processor_usage_").unwrap();
        assert!((usage - 0.775).abs() < 1e-6, "Usage was {usage}, expected 0.775");
    }

    #[test]
    fn parse_proc_stat_zero_dwell_defaults_to_one() {
        let mut last_totals = HashMap::new();
        let _ = ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE, &mut last_totals, 0.0);
        let (readings, _) =
            ProcessorProvider::parse_proc_stat(PROC_STAT_SAMPLE2, &mut last_totals, 0.0);

        // dwell=0.0 should behave like dwell=1.0
        let usage = numeric_value(&readings, "_processor_usage_").unwrap();
        assert!((usage - 1.55).abs() < 1e-6, "Usage was {usage}, expected 1.55");
    }

    #[test]
    fn parse_proc_stat_empty_string() {
        let mut last_totals = HashMap::new();
        let (readings, cores) = ProcessorProvider::parse_proc_stat("", &mut last_totals, 1.0);
        assert!(readings.is_empty());
        assert_eq!(cores, 0);
    }

    #[test]
    fn parse_proc_stat_single_cpu_no_cores() {
        let stat = "cpu  1000 200 300 5000 10 0 5 0 0 0\n";
        let mut last_totals = HashMap::new();
        let (_, cores) = ProcessorProvider::parse_proc_stat(stat, &mut last_totals, 1.0);
        // Only aggregate "cpu" line, no per-core lines => cores = 0
        assert_eq!(cores, 0);
    }

    // --- cpuinfo frequency tests ---

    #[test]
    fn parse_cpuinfo_frequencies_standard() {
        let cpuinfo = "\
processor\t: 0
vendor_id\t: GenuineIntel
cpu MHz\t\t: 3400.000
cache size\t: 12288 KB

processor\t: 1
vendor_id\t: GenuineIntel
cpu MHz\t\t: 3200.500
cache size\t: 12288 KB
";
        let freqs = ProcessorProvider::parse_cpuinfo_frequencies(cpuinfo);
        assert_eq!(freqs.len(), 2);
        assert!((freqs[0] - 3_400_000_000.0).abs() < 1e-3);
        assert!((freqs[1] - 3_200_500_000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_cpuinfo_frequencies_empty() {
        let freqs = ProcessorProvider::parse_cpuinfo_frequencies("");
        assert!(freqs.is_empty());
    }

    #[test]
    fn parse_cpuinfo_frequencies_no_mhz_lines() {
        let cpuinfo = "processor\t: 0\nvendor_id\t: GenuineIntel\n";
        let freqs = ProcessorProvider::parse_cpuinfo_frequencies(cpuinfo);
        assert!(freqs.is_empty());
    }

    // --- frequency_readings tests ---

    #[test]
    fn frequency_readings_multiple() {
        let speeds = vec![1_000_000.0, 2_000_000.0, 3_000_000.0];
        let readings = ProcessorProvider::frequency_readings(&speeds);
        assert_eq!(readings.len(), 3);

        let avg = numeric_value(&readings, "_processor_frequency_").unwrap();
        assert!((avg - 2_000_000.0).abs() < 1e-3);

        let max = numeric_value(&readings, "_processor_max_frequency_").unwrap();
        assert!((max - 3_000_000.0).abs() < 1e-3);

        let min = numeric_value(&readings, "_processor_min_frequency_").unwrap();
        assert!((min - 1_000_000.0).abs() < 1e-3);
    }

    #[test]
    fn frequency_readings_single() {
        let speeds = vec![2_500_000.0];
        let readings = ProcessorProvider::frequency_readings(&speeds);
        assert_eq!(readings.len(), 3);
        // avg = max = min for a single value
        let avg = numeric_value(&readings, "_processor_frequency_").unwrap();
        let max = numeric_value(&readings, "_processor_max_frequency_").unwrap();
        let min = numeric_value(&readings, "_processor_min_frequency_").unwrap();
        assert!((avg - max).abs() < 1e-6);
        assert!((avg - min).abs() < 1e-6);
    }

    #[test]
    fn frequency_readings_empty() {
        let readings = ProcessorProvider::frequency_readings(&[]);
        assert!(readings.is_empty());
    }
}

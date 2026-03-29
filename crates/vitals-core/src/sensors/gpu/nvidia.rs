use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

use crate::sensors::{SensorCategory, SensorFormat, SensorReading, SensorValue};

pub struct NvidiaGpuProvider {
    child: Option<Child>,
    receiver: mpsc::Receiver<String>,
    bad_split_count: u32,
    static_returned: bool,
    include_static_info: bool,
}

impl NvidiaGpuProvider {
    /// Try to start nvidia-smi. Returns None if nvidia-smi is not available.
    pub fn try_new(update_time: u32, include_static_info: bool) -> Option<Self> {
        let (tx, rx) = mpsc::channel();

        let query = build_query_string(include_static_info);
        let interval = update_time.max(1).to_string();

        let mut child = Command::new("nvidia-smi")
            .arg(format!("--query-gpu={query}"))
            .arg("--format=csv,noheader,nounits")
            .arg("-l")
            .arg(&interval)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdout = child.stdout.take()?;
        let sender = tx;

        // Spawn a reader thread
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if sender.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Some(Self {
            child: Some(child),
            receiver: rx,
            bad_split_count: 0,
            static_returned: false,
            include_static_info,
        })
    }

    pub fn query(&mut self) -> Vec<SensorReading> {
        let mut readings = Vec::new();

        // Collect all available lines (non-blocking)
        let mut lines = Vec::new();
        while let Ok(line) = self.receiver.try_recv() {
            lines.push(line);
        }

        if lines.is_empty() {
            return readings;
        }

        // Process each GPU's data line
        let multi_gpu = lines.len() > 1;
        for (i, line) in lines.iter().enumerate() {
            let gpu_num = (i + 1) as u8;
            let (parsed, bad_split) =
                Self::parse_csv_line(line, gpu_num, multi_gpu, self.static_returned);
            readings.extend(parsed);
            if bad_split {
                self.bad_split_count += 1;
                if self.bad_split_count >= 3 {
                    self.shutdown();
                }
            } else {
                self.bad_split_count = 0;
            }
        }

        // After first successful parse with static info, reconfigure to stop querying static info
        if !self.static_returned && self.include_static_info {
            self.static_returned = true;
        }

        readings
    }

    /// Parse a single CSV line from nvidia-smi output.
    ///
    /// Returns (readings, bad_split) where bad_split is true if the line had
    /// fewer fields than expected.
    pub fn parse_csv_line(
        csv: &str,
        gpu_num: u8,
        multi_gpu: bool,
        static_returned: bool,
    ) -> (Vec<SensorReading>, bool) {
        let mut readings = Vec::new();
        let expected_len = 19;
        let fields: Vec<&str> = csv.split(',').map(|s| s.trim()).collect();

        if fields.len() < expected_len {
            return (readings, true);
        }

        let type_name = format!("gpu#{gpu_num}");
        let _global_label = if multi_gpu {
            format!("GPU {gpu_num}")
        } else {
            "GPU".to_string()
        };

        let parse_f64 = |s: &str| -> Option<f64> {
            let trimmed = s.trim();
            if trimmed == "N/A" || trimmed == "[N/A]" {
                return None;
            }
            trimmed.parse::<f64>().ok().filter(|v| v.is_finite())
        };

        let label = fields[0];
        let fan_speed_pct = parse_f64(fields[1]);
        let temp_gpu = parse_f64(fields[2]);
        let temp_mem = parse_f64(fields[3]);
        let mem_total = parse_f64(fields[4]);
        let mem_used = parse_f64(fields[5]);
        let mem_reserved = parse_f64(fields[6]);
        let mem_free = parse_f64(fields[7]);
        let util_gpu = parse_f64(fields[8]);
        let util_mem = parse_f64(fields[9]);
        let util_encoder = parse_f64(fields[10]);
        let util_decoder = parse_f64(fields[11]);
        let clock_gpu = parse_f64(fields[12]);
        let clock_mem = parse_f64(fields[13]);
        let clock_video = parse_f64(fields[14]);
        let power_draw = parse_f64(fields[15]);
        let power_avg = parse_f64(fields[16]);
        let link_gen = fields.get(17).copied();
        let link_width = fields.get(18).copied();

        let cat = SensorCategory::Gpu(gpu_num);

        // Group summary - utilization
        if let Some(u) = util_gpu {
            readings.push(SensorReading {
                label: "Graphics".to_string(),
                value: SensorValue::Numeric(u * 0.01),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: format!("_{type_name}_group_"),
            });
        }

        // Name
        readings.push(SensorReading {
            label: "Name".to_string(),
            value: SensorValue::Text(label.to_string()),
            category: cat.clone(),
            format: SensorFormat::StringVal,
            key: format!("_{type_name}_name_"),
        });

        // Fan
        if let Some(f) = fan_speed_pct {
            readings.push(SensorReading {
                label: "Fan".to_string(),
                value: SensorValue::Numeric(f * 0.01),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: format!("_{type_name}_fan_"),
            });
        }

        // Temperature
        if let Some(t) = temp_gpu {
            readings.push(SensorReading {
                label: "Temperature".to_string(),
                value: SensorValue::Numeric(t * 1000.0),
                category: cat.clone(),
                format: SensorFormat::Temp,
                key: format!("_{type_name}_temp_"),
            });
        }
        if let Some(t) = temp_mem {
            readings.push(SensorReading {
                label: "Memory Temperature".to_string(),
                value: SensorValue::Numeric(t * 1000.0),
                category: cat.clone(),
                format: SensorFormat::Temp,
                key: format!("_{type_name}_temp_mem_"),
            });
        }

        // Memory
        if let (Some(total), Some(used)) = (mem_total, mem_used) {
            if total > 0.0 {
                readings.push(SensorReading {
                    label: "Memory Usage".to_string(),
                    value: SensorValue::Numeric(used / total),
                    category: cat.clone(),
                    format: SensorFormat::Percent,
                    key: format!("_{type_name}_mem_usage_"),
                });
            }
        }
        if let Some(v) = mem_total {
            readings.push(SensorReading {
                label: "Memory Total".to_string(),
                value: SensorValue::Numeric(v * 1000.0),
                category: cat.clone(),
                format: SensorFormat::Memory,
                key: format!("_{type_name}_mem_total_"),
            });
        }
        if let Some(v) = mem_used {
            readings.push(SensorReading {
                label: "Memory Used".to_string(),
                value: SensorValue::Numeric(v * 1000.0),
                category: cat.clone(),
                format: SensorFormat::Memory,
                key: format!("_{type_name}_mem_used_"),
            });
        }
        if let Some(v) = mem_reserved {
            readings.push(SensorReading {
                label: "Memory Reserved".to_string(),
                value: SensorValue::Numeric(v * 1000.0),
                category: cat.clone(),
                format: SensorFormat::Memory,
                key: format!("_{type_name}_mem_reserved_"),
            });
        }
        if let Some(v) = mem_free {
            readings.push(SensorReading {
                label: "Memory Free".to_string(),
                value: SensorValue::Numeric(v * 1000.0),
                category: cat.clone(),
                format: SensorFormat::Memory,
                key: format!("_{type_name}_mem_free_"),
            });
        }

        // Utilization
        if let Some(u) = util_gpu {
            readings.push(SensorReading {
                label: "Utilization".to_string(),
                value: SensorValue::Numeric(u * 0.01),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: format!("_{type_name}_util_"),
            });
        }
        if let Some(u) = util_mem {
            readings.push(SensorReading {
                label: "Memory Utilization".to_string(),
                value: SensorValue::Numeric(u * 0.01),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: format!("_{type_name}_util_mem_"),
            });
        }
        if let Some(u) = util_encoder {
            readings.push(SensorReading {
                label: "Encoder Utilization".to_string(),
                value: SensorValue::Numeric(u * 0.01),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: format!("_{type_name}_util_encoder_"),
            });
        }
        if let Some(u) = util_decoder {
            readings.push(SensorReading {
                label: "Decoder Utilization".to_string(),
                value: SensorValue::Numeric(u * 0.01),
                category: cat.clone(),
                format: SensorFormat::Percent,
                key: format!("_{type_name}_util_decoder_"),
            });
        }

        // Clock speeds
        if let Some(c) = clock_gpu {
            readings.push(SensorReading {
                label: "Frequency".to_string(),
                value: SensorValue::Numeric(c * 1_000_000.0),
                category: cat.clone(),
                format: SensorFormat::Hertz,
                key: format!("_{type_name}_clock_gpu_"),
            });
        }
        if let Some(c) = clock_mem {
            readings.push(SensorReading {
                label: "Memory Frequency".to_string(),
                value: SensorValue::Numeric(c * 1_000_000.0),
                category: cat.clone(),
                format: SensorFormat::Hertz,
                key: format!("_{type_name}_clock_mem_"),
            });
        }
        if let Some(c) = clock_video {
            readings.push(SensorReading {
                label: "Encoder/Decoder Frequency".to_string(),
                value: SensorValue::Numeric(c * 1_000_000.0),
                category: cat.clone(),
                format: SensorFormat::Hertz,
                key: format!("_{type_name}_clock_video_"),
            });
        }

        // Power
        if let Some(p) = power_draw {
            readings.push(SensorReading {
                label: "Power".to_string(),
                value: SensorValue::Numeric(p),
                category: cat.clone(),
                format: SensorFormat::WattGpu,
                key: format!("_{type_name}_power_"),
            });
        }
        if let Some(p) = power_avg {
            readings.push(SensorReading {
                label: "Average Power".to_string(),
                value: SensorValue::Numeric(p),
                category: cat.clone(),
                format: SensorFormat::WattGpu,
                key: format!("_{type_name}_power_avg_"),
            });
        }

        // PCIe link
        if let (Some(r#gen), Some(width)) = (link_gen, link_width) {
            if !r#gen.is_empty() && !width.is_empty() {
                let pcie_str = format!("{gen}x{width}");
                readings.push(SensorReading {
                    label: "Link Speed".to_string(),
                    value: SensorValue::Text(format!("PCIe {pcie_str}")),
                    category: cat.clone(),
                    format: SensorFormat::Pcie,
                    key: format!("_{type_name}_pcie_"),
                });
            }
        }

        // Static info (fields 19+)
        if fields.len() >= 32 && !static_returned {
            let static_fields = &fields[19..];
            let static_names = [
                ("Temperature Limit", SensorFormat::Temp, true),
                ("Power Limit", SensorFormat::WattGpu, false),
                ("Maximum Link Gen", SensorFormat::StringVal, false),
                ("Maximum Link Width", SensorFormat::StringVal, false),
                ("Addressing Mode", SensorFormat::StringVal, false),
                ("Driver Version", SensorFormat::StringVal, false),
                ("vBIOS Version", SensorFormat::StringVal, false),
                ("Serial Number", SensorFormat::StringVal, false),
                ("Domain Number", SensorFormat::StringVal, false),
                ("Bus Number", SensorFormat::StringVal, false),
                ("Device Number", SensorFormat::StringVal, false),
                ("Device ID", SensorFormat::StringVal, false),
                ("Sub Device ID", SensorFormat::StringVal, false),
            ];

            for (i, (name, fmt, is_temp)) in static_names.iter().enumerate() {
                if i < static_fields.len() {
                    let val = static_fields[i].trim();
                    if !val.is_empty() && val != "N/A" && val != "[N/A]" {
                        let value = if *is_temp {
                            if let Ok(t) = val.parse::<f64>() {
                                SensorValue::Numeric(t * 1000.0)
                            } else {
                                continue;
                            }
                        } else if *fmt == SensorFormat::WattGpu {
                            if let Ok(w) = val.parse::<f64>() {
                                SensorValue::Numeric(w)
                            } else {
                                continue;
                            }
                        } else {
                            SensorValue::Text(val.to_string())
                        };

                        readings.push(SensorReading {
                            label: name.to_string(),
                            value,
                            category: cat.clone(),
                            format: *fmt,
                            key: format!("_{type_name}_static_{i}_"),
                        });
                    }
                }
            }
        }

        (readings, false)
    }

    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for NvidiaGpuProvider {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn build_query_string(include_static: bool) -> String {
    let mut query = String::from(
        "name,\
         fan.speed,\
         temperature.gpu,temperature.memory,\
         memory.total,memory.used,memory.reserved,memory.free,\
         utilization.gpu,utilization.memory,utilization.encoder,utilization.decoder,\
         clocks.gr,clocks.mem,clocks.video,\
         power.draw.instant,power.draw.average,\
         pcie.link.gen.gpucurrent,pcie.link.width.current",
    );

    if include_static {
        query.push_str(
            ",temperature.gpu.tlimit,\
             power.limit,\
             pcie.link.gen.max,pcie.link.width.max,\
             addressing_mode,\
             driver_version,vbios_version,serial,\
             pci.domain,pci.bus,pci.device,pci.device_id,pci.sub_device_id",
        );
    }

    query
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

    // 19-field CSV: name, fan, temp_gpu, temp_mem, mem_total, mem_used, mem_reserved, mem_free,
    //   util_gpu, util_mem, util_enc, util_dec, clock_gpu, clock_mem, clock_video,
    //   power_draw, power_avg, link_gen, link_width
    const CSV_NORMAL: &str =
        "NVIDIA GeForce RTX 4090, 45, 62, 78, 24576, 4096, 256, 20224, 35, 12, 0, 5, 2100, 10501, 1950, 125.50, 98.30, 4, 16";

    #[test]
    fn parse_csv_line_normal_name() {
        let (readings, bad) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);
        assert!(!bad);
        assert_eq!(
            text_value(&readings, "_gpu#1_name_").unwrap(),
            "NVIDIA GeForce RTX 4090"
        );
    }

    #[test]
    fn parse_csv_line_normal_fan() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);
        let fan = numeric_value(&readings, "_gpu#1_fan_").unwrap();
        assert!((fan - 0.45).abs() < 1e-6);
    }

    #[test]
    fn parse_csv_line_normal_temperature() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);
        // GPU temp: 62 * 1000 = 62000 millidegrees
        let temp = numeric_value(&readings, "_gpu#1_temp_").unwrap();
        assert!((temp - 62000.0).abs() < 1e-3);

        // Memory temp: 78 * 1000 = 78000
        let temp_mem = numeric_value(&readings, "_gpu#1_temp_mem_").unwrap();
        assert!((temp_mem - 78000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_csv_line_normal_memory() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);

        // Memory usage: 4096/24576
        let mem_usage = numeric_value(&readings, "_gpu#1_mem_usage_").unwrap();
        assert!((mem_usage - 4096.0 / 24576.0).abs() < 1e-6);

        // Memory total: 24576 * 1000
        let mem_total = numeric_value(&readings, "_gpu#1_mem_total_").unwrap();
        assert!((mem_total - 24576000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_csv_line_normal_utilization() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);

        let util = numeric_value(&readings, "_gpu#1_util_").unwrap();
        assert!((util - 0.35).abs() < 1e-6);

        let util_mem = numeric_value(&readings, "_gpu#1_util_mem_").unwrap();
        assert!((util_mem - 0.12).abs() < 1e-6);

        let util_dec = numeric_value(&readings, "_gpu#1_util_decoder_").unwrap();
        assert!((util_dec - 0.05).abs() < 1e-6);
    }

    #[test]
    fn parse_csv_line_normal_clocks() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);

        let clock_gpu = numeric_value(&readings, "_gpu#1_clock_gpu_").unwrap();
        assert!((clock_gpu - 2_100_000_000.0).abs() < 1e-3);

        let clock_mem = numeric_value(&readings, "_gpu#1_clock_mem_").unwrap();
        assert!((clock_mem - 10_501_000_000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_csv_line_normal_power() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);

        let power = numeric_value(&readings, "_gpu#1_power_").unwrap();
        assert!((power - 125.50).abs() < 1e-6);

        let power_avg = numeric_value(&readings, "_gpu#1_power_avg_").unwrap();
        assert!((power_avg - 98.30).abs() < 1e-6);
    }

    #[test]
    fn parse_csv_line_normal_pcie() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);
        let pcie = text_value(&readings, "_gpu#1_pcie_").unwrap();
        assert_eq!(pcie, "PCIe 4x16");
    }

    #[test]
    fn parse_csv_line_short_is_bad_split() {
        let short_csv = "NVIDIA GeForce RTX 4090, 45, 62";
        let (readings, bad) = NvidiaGpuProvider::parse_csv_line(short_csv, 1, false, false);
        assert!(bad);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_csv_line_empty_is_bad_split() {
        let (readings, bad) = NvidiaGpuProvider::parse_csv_line("", 1, false, false);
        assert!(bad);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_csv_line_na_values_handled() {
        let csv_na = "NVIDIA GeForce RTX 4090, N/A, 62, [N/A], 24576, 4096, 256, 20224, 35, 12, 0, 5, 2100, 10501, 1950, 125.50, 98.30, 4, 16";
        let (readings, bad) = NvidiaGpuProvider::parse_csv_line(csv_na, 1, false, false);
        assert!(!bad);

        // Fan should be absent due to N/A
        assert!(find_reading(&readings, "_gpu#1_fan_").is_none());

        // Memory temp should be absent due to [N/A]
        assert!(find_reading(&readings, "_gpu#1_temp_mem_").is_none());

        // GPU temp should still be present
        assert!(find_reading(&readings, "_gpu#1_temp_").is_some());
    }

    #[test]
    fn parse_csv_line_multi_gpu_numbering() {
        let (readings_gpu1, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, true, false);
        let (readings_gpu2, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 2, true, false);

        // GPU 1 readings should use gpu#1 keys
        assert!(find_reading(&readings_gpu1, "_gpu#1_name_").is_some());
        assert!(find_reading(&readings_gpu1, "_gpu#1_temp_").is_some());

        // GPU 2 readings should use gpu#2 keys
        assert!(find_reading(&readings_gpu2, "_gpu#2_name_").is_some());
        assert!(find_reading(&readings_gpu2, "_gpu#2_temp_").is_some());

        // Cross-check: gpu#1 readings should not have gpu#2 keys
        assert!(find_reading(&readings_gpu1, "_gpu#2_name_").is_none());
    }

    #[test]
    fn parse_csv_line_group_reading_uses_utilization() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);
        let group = numeric_value(&readings, "_gpu#1_group_").unwrap();
        // util_gpu = 35, so group = 0.35
        assert!((group - 0.35).abs() < 1e-6);
    }

    #[test]
    fn parse_csv_line_encoder_zero_utilization() {
        let (readings, _) = NvidiaGpuProvider::parse_csv_line(CSV_NORMAL, 1, false, false);
        let util_enc = numeric_value(&readings, "_gpu#1_util_encoder_").unwrap();
        assert!((util_enc - 0.0).abs() < 1e-6);
    }

    #[test]
    fn build_query_string_without_static() {
        let query = build_query_string(false);
        assert!(!query.contains("driver_version"));
        assert!(!query.contains("temperature.gpu.tlimit"));
        assert!(query.contains("name"));
        assert!(query.contains("fan.speed"));
    }

    #[test]
    fn build_query_string_with_static() {
        let query = build_query_string(true);
        assert!(query.contains("driver_version"));
        assert!(query.contains("temperature.gpu.tlimit"));
        assert!(query.contains("pci.device_id"));
    }

    #[test]
    fn parse_csv_line_exactly_19_fields() {
        // Exactly 19 comma-separated values (minimum required)
        let csv = "Name, 50, 70, 80, 8192, 2048, 128, 6016, 25, 10, 5, 3, 1500, 7000, 1200, 100.0, 80.0, 3, 8";
        let (readings, bad) = NvidiaGpuProvider::parse_csv_line(csv, 1, false, false);
        assert!(!bad);
        assert!(!readings.is_empty());
    }

    #[test]
    fn parse_csv_line_18_fields_is_bad() {
        // 18 fields = too few
        let csv = "Name, 50, 70, 80, 8192, 2048, 128, 6016, 25, 10, 5, 3, 1500, 7000, 1200, 100.0, 80.0, 3";
        let (readings, bad) = NvidiaGpuProvider::parse_csv_line(csv, 1, false, false);
        assert!(bad);
        assert!(readings.is_empty());
    }
}

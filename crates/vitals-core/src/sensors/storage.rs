use crate::sensors::{SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue};

pub struct StorageProvider {
    storage_path: String,
    storage_device: Option<String>,
    last_read: u64,
    last_write: u64,
}

impl StorageProvider {
    pub fn new(storage_path: String) -> Self {
        Self {
            storage_path,
            storage_device: None,
            last_read: 0,
            last_write: 0,
        }
    }

    fn find_storage_device(&mut self) {
        if let Ok(contents) = std::fs::read_to_string("/proc/mounts") {
            for line in contents.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == self.storage_path {
                    self.storage_device = Some(parts[0].to_string());
                    return;
                }
            }
        }
    }

    /// Parse /proc/spl/kstat/zfs/arcstats contents into ARC sensor readings.
    pub fn parse_arcstats(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Storage;
        let mut target: u64 = 0;
        let mut maximum: u64 = 0;
        let mut current: u64 = 0;

        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                match parts[0] {
                    "c" => {
                        if let Ok(v) = parts[2].parse() {
                            target = v;
                        }
                    }
                    "c_max" => {
                        if let Ok(v) = parts[2].parse() {
                            maximum = v;
                        }
                    }
                    "size" => {
                        if let Ok(v) = parts[2].parse() {
                            current = v;
                        }
                    }
                    _ => {}
                }
            }
        }

        if target > 0 {
            readings.push(SensorReading {
                label: "ARC Target".to_string(),
                value: SensorValue::Numeric(target as f64),
                category: cat.clone(),
                format: SensorFormat::Storage,
                key: "_storage_arc_target_".to_string(),
            });
            readings.push(SensorReading {
                label: "ARC Maximum".to_string(),
                value: SensorValue::Numeric(maximum as f64),
                category: cat.clone(),
                format: SensorFormat::Storage,
                key: "_storage_arc_maximum_".to_string(),
            });
            readings.push(SensorReading {
                label: "ARC Current".to_string(),
                value: SensorValue::Numeric(current as f64),
                category: cat,
                format: SensorFormat::Storage,
                key: "_storage_arc_current_".to_string(),
            });
        }

        readings
    }

    /// Parse /proc/diskstats contents for a given device and compute I/O readings.
    ///
    /// `device` is the full path like "/dev/sda".
    /// `last_read` and `last_write` track cumulative bytes for rate calculation.
    /// `dwell` is seconds since last poll.
    ///
    /// Returns the readings and updates `last_read`/`last_write` in place.
    pub fn parse_diskstats(
        contents: &str,
        device: &str,
        last_read: &mut u64,
        last_write: &mut u64,
        dwell: f64,
    ) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Storage;

        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 {
                let dev_name = format!("/dev/{}", parts[2]);
                if dev_name == *device {
                    if let (Ok(read_sectors), Ok(write_sectors)) =
                        (parts[5].parse::<u64>(), parts[9].parse::<u64>())
                    {
                        let read = read_sectors * 512;
                        let write = write_sectors * 512;
                        let sample_dwell = if dwell > 0.0 { dwell } else { 1.0 };

                        readings.push(SensorReading {
                            label: "Read total".to_string(),
                            value: SensorValue::Numeric(read as f64),
                            category: cat.clone(),
                            format: SensorFormat::Storage,
                            key: "_storage_read_total_".to_string(),
                        });
                        readings.push(SensorReading {
                            label: "Write total".to_string(),
                            value: SensorValue::Numeric(write as f64),
                            category: cat.clone(),
                            format: SensorFormat::Storage,
                            key: "_storage_write_total_".to_string(),
                        });

                        if *last_read > 0 {
                            readings.push(SensorReading {
                                label: "Read rate".to_string(),
                                value: SensorValue::Numeric(
                                    (read - *last_read) as f64 / sample_dwell,
                                ),
                                category: cat.clone(),
                                format: SensorFormat::Storage,
                                key: "_storage_read_rate_".to_string(),
                            });
                            readings.push(SensorReading {
                                label: "Write rate".to_string(),
                                value: SensorValue::Numeric(
                                    (write - *last_write) as f64 / sample_dwell,
                                ),
                                category: cat,
                                format: SensorFormat::Storage,
                                key: "_storage_write_rate_".to_string(),
                            });
                        }

                        *last_read = read;
                        *last_write = write;
                    }
                    break;
                }
            }
        }

        readings
    }
}

impl SensorProvider for StorageProvider {
    fn discover(&mut self) -> Vec<SensorReading> {
        self.find_storage_device();
        Vec::new()
    }

    fn query(&mut self, dwell: f64) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        let cat = SensorCategory::Storage;

        // ZFS ARC stats
        if let Ok(contents) = std::fs::read_to_string("/proc/spl/kstat/zfs/arcstats") {
            readings.extend(Self::parse_arcstats(&contents));
        }

        // Disk I/O stats from /proc/diskstats
        if let Some(device) = &self.storage_device.clone() {
            if let Ok(contents) = std::fs::read_to_string("/proc/diskstats") {
                readings.extend(Self::parse_diskstats(
                    &contents,
                    device,
                    &mut self.last_read,
                    &mut self.last_write,
                    dwell,
                ));
            }
        }

        // Filesystem usage via statvfs (replaces GTop dependency)
        if let Ok(stat) = nix::sys::statvfs::statvfs(self.storage_path.as_str()) {
            let block_size = stat.block_size() as u64;
            let total = stat.blocks() * block_size;
            let free = stat.blocks_free() * block_size;
            let avail = stat.blocks_available() * block_size;
            let used = total - free;
            let _reserved = (total - avail) - used;

            let free_pct = if total > 0 {
                (free as f64 / total as f64 * 100.0).round()
            } else {
                0.0
            };
            let used_pct = if total > 0 {
                (used as f64 / total as f64 * 100.0).round()
            } else {
                0.0
            };

            readings.push(SensorReading {
                label: "Total".to_string(),
                value: SensorValue::Numeric(total as f64),
                category: cat.clone(),
                format: SensorFormat::Storage,
                key: "_storage_total_".to_string(),
            });
            readings.push(SensorReading {
                label: "Used".to_string(),
                value: SensorValue::Numeric(used as f64),
                category: cat.clone(),
                format: SensorFormat::Storage,
                key: "_storage_used_".to_string(),
            });
            readings.push(SensorReading {
                label: "Free".to_string(),
                value: SensorValue::Numeric(avail as f64),
                category: cat.clone(),
                format: SensorFormat::Storage,
                key: "_storage_free_".to_string(),
            });
            readings.push(SensorReading {
                label: "Used %".to_string(),
                value: SensorValue::Text(format!("{used_pct}%")),
                category: cat.clone(),
                format: SensorFormat::StringVal,
                key: "_storage_used_pct_".to_string(),
            });
            readings.push(SensorReading {
                label: "Free %".to_string(),
                value: SensorValue::Text(format!("{free_pct}%")),
                category: cat.clone(),
                format: SensorFormat::StringVal,
                key: "_storage_free_pct_".to_string(),
            });
            // Group summary
            readings.push(SensorReading {
                label: "storage".to_string(),
                value: SensorValue::Numeric(avail as f64),
                category: cat,
                format: SensorFormat::Storage,
                key: "_storage_group_".to_string(),
            });
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Storage
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

    // --- arcstats tests ---

    const ARCSTATS_SAMPLE: &str = "\
5 1 0x01 86 4832 7741953495
name                            type data
hits                            4    12345678
misses                          4    1234567
demand_data_hits                4    9876543
c                               4    8589934592
c_max                           4    17179869184
size                            4    6442450944
";

    #[test]
    fn parse_arcstats_standard() {
        let readings = StorageProvider::parse_arcstats(ARCSTATS_SAMPLE);
        assert_eq!(readings.len(), 3);

        let target = numeric_value(&readings, "_storage_arc_target_").unwrap();
        assert!((target - 8_589_934_592.0).abs() < 1e-3);

        let maximum = numeric_value(&readings, "_storage_arc_maximum_").unwrap();
        assert!((maximum - 17_179_869_184.0).abs() < 1e-3);

        let current = numeric_value(&readings, "_storage_arc_current_").unwrap();
        assert!((current - 6_442_450_944.0).abs() < 1e-3);
    }

    #[test]
    fn parse_arcstats_zero_target_no_readings() {
        let contents = "name type data\nhits 4 100\n";
        let readings = StorageProvider::parse_arcstats(contents);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_arcstats_empty() {
        let readings = StorageProvider::parse_arcstats("");
        assert!(readings.is_empty());
    }

    // --- diskstats tests ---

    // Columns: major minor name reads_completed reads_merged sectors_read ms_reading
    //          writes_completed writes_merged sectors_written ms_writing ...
    const DISKSTATS_SAMPLE: &str = "\
   8       0 sda 12345 6789 200000 54321 9876 5432 100000 12345 0 43210 66666
   8       1 sda1 11111 5555 180000 44444 8888 4444 90000 11111 0 38888 55555
   8      16 sdb 1000 500 50000 2000 500 250 25000 1000 0 2500 3000
";

    #[test]
    fn parse_diskstats_first_call() {
        let mut last_read = 0u64;
        let mut last_write = 0u64;
        let readings = StorageProvider::parse_diskstats(
            DISKSTATS_SAMPLE,
            "/dev/sda",
            &mut last_read,
            &mut last_write,
            1.0,
        );

        // First call: should have read_total and write_total but no rates
        assert_eq!(readings.len(), 2);

        let read_total = numeric_value(&readings, "_storage_read_total_").unwrap();
        assert!((read_total - 200000.0 * 512.0).abs() < 1e-3);

        let write_total = numeric_value(&readings, "_storage_write_total_").unwrap();
        assert!((write_total - 100000.0 * 512.0).abs() < 1e-3);

        // last_read and last_write should be updated
        assert_eq!(last_read, 200000 * 512);
        assert_eq!(last_write, 100000 * 512);
    }

    #[test]
    fn parse_diskstats_second_call_with_rates() {
        let mut last_read = 100000u64 * 512;
        let mut last_write = 50000u64 * 512;

        let readings = StorageProvider::parse_diskstats(
            DISKSTATS_SAMPLE,
            "/dev/sda",
            &mut last_read,
            &mut last_write,
            2.0,
        );

        // Should have total + rate readings = 4
        assert_eq!(readings.len(), 4);

        let read_rate = numeric_value(&readings, "_storage_read_rate_").unwrap();
        let expected_read_rate = (200000 * 512 - 100000 * 512) as f64 / 2.0;
        assert!((read_rate - expected_read_rate).abs() < 1e-3);

        let write_rate = numeric_value(&readings, "_storage_write_rate_").unwrap();
        let expected_write_rate = (100000 * 512 - 50000 * 512) as f64 / 2.0;
        assert!((write_rate - expected_write_rate).abs() < 1e-3);
    }

    #[test]
    fn parse_diskstats_specific_device() {
        let mut last_read = 0u64;
        let mut last_write = 0u64;

        let readings = StorageProvider::parse_diskstats(
            DISKSTATS_SAMPLE,
            "/dev/sdb",
            &mut last_read,
            &mut last_write,
            1.0,
        );

        assert_eq!(readings.len(), 2);
        let read_total = numeric_value(&readings, "_storage_read_total_").unwrap();
        assert!((read_total - 50000.0 * 512.0).abs() < 1e-3);
    }

    #[test]
    fn parse_diskstats_device_not_found() {
        let mut last_read = 0u64;
        let mut last_write = 0u64;

        let readings = StorageProvider::parse_diskstats(
            DISKSTATS_SAMPLE,
            "/dev/nvme0n1",
            &mut last_read,
            &mut last_write,
            1.0,
        );

        assert!(readings.is_empty());
        assert_eq!(last_read, 0);
    }

    #[test]
    fn parse_diskstats_empty() {
        let mut last_read = 0u64;
        let mut last_write = 0u64;
        let readings =
            StorageProvider::parse_diskstats("", "/dev/sda", &mut last_read, &mut last_write, 1.0);
        assert!(readings.is_empty());
    }

    #[test]
    fn parse_diskstats_zero_dwell_defaults_to_one() {
        let mut last_read = 100000u64 * 512;
        let mut last_write = 50000u64 * 512;

        let readings = StorageProvider::parse_diskstats(
            DISKSTATS_SAMPLE,
            "/dev/sda",
            &mut last_read,
            &mut last_write,
            0.0,
        );

        let read_rate = numeric_value(&readings, "_storage_read_rate_").unwrap();
        // dwell=0 -> sample_dwell=1.0
        let expected = (200000 * 512 - 100000 * 512) as f64 / 1.0;
        assert!((read_rate - expected).abs() < 1e-3);
    }
}

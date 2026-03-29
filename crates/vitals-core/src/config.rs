use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub temperature: TemperatureConfig,
    #[serde(default)]
    pub voltage: VoltageConfig,
    #[serde(default)]
    pub fan: FanConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub processor: ProcessorConfig,
    #[serde(default)]
    pub system: SystemConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub battery: BatteryConfig,
    #[serde(default)]
    pub gpu: GpuConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub hot_sensors: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            temperature: TemperatureConfig::default(),
            voltage: VoltageConfig::default(),
            fan: FanConfig::default(),
            memory: MemoryConfig::default(),
            processor: ProcessorConfig::default(),
            system: SystemConfig::default(),
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            battery: BatteryConfig::default(),
            gpu: GpuConfig::default(),
            history: HistoryConfig::default(),
            hot_sensors: vec![
                "_memory_usage_".to_string(),
                "_system_load_1m_".to_string(),
                "__network-rx_max__".to_string(),
            ],
        }
    }
}

impl AppConfig {
    /// Returns the path to the config file: ~/.config/vitals/config.toml
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vitals")
            .join("config.toml")
    }

    /// Returns the path to the cache directory: ~/.cache/vitals/
    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("vitals")
    }

    /// Returns the path to the history cache file.
    pub fn history_path() -> PathBuf {
        Self::cache_dir().join("history.json")
    }

    /// Load configuration from TOML file. Returns default config if file doesn't exist.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => return config,
                    Err(e) => log::warn!("Failed to parse config: {e}"),
                },
                Err(e) => log::warn!("Failed to read config: {e}"),
            }
        }
        Self::default()
    }

    /// Save configuration to TOML file.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Seconds between sensor polls (1-60).
    pub update_time: u32,
    /// Show one extra digit after decimal.
    pub use_higher_precision: bool,
    /// Display sensors in alphabetical order.
    pub alphabetize: bool,
    /// Hide sensors that report zero values.
    pub hide_zeros: bool,
    /// Keep sensor widths fixed to prevent UI jitter.
    pub fixed_widths: bool,
    /// Hide icons, show only sensor values.
    pub hide_icons: bool,
    /// Center the dropdown menu.
    pub menu_centered: bool,
    /// Icon style: 0 = Original, 1 = GNOME.
    pub icon_style: u32,
    /// Command to launch system monitor.
    pub monitor_cmd: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            update_time: 5,
            use_higher_precision: false,
            alphabetize: true,
            hide_zeros: false,
            fixed_widths: true,
            hide_icons: false,
            menu_centered: false,
            icon_style: 0,
            monitor_cmd: "gnome-system-monitor".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureConfig {
    pub show: bool,
    /// 0 = Celsius, 1 = Fahrenheit.
    pub unit: u32,
}

impl Default for TemperatureConfig {
    fn default() -> Self {
        Self {
            show: true,
            unit: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoltageConfig {
    pub show: bool,
}

impl Default for VoltageConfig {
    fn default() -> Self {
        Self { show: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanConfig {
    pub show: bool,
}

impl Default for FanConfig {
    fn default() -> Self {
        Self { show: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub show: bool,
    /// 0 = binary (GiB), 1 = decimal (GB).
    pub measurement: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            show: true,
            measurement: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    pub show: bool,
    pub include_static_info: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            show: true,
            include_static_info: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub show: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self { show: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub show: bool,
    pub include_public_ip: bool,
    /// 0 = bytes/s, 1 = bits/s.
    pub speed_format: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            show: true,
            include_public_ip: true,
            speed_format: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub show: bool,
    pub path: String,
    /// 0 = binary (GiB), 1 = decimal (GB).
    pub measurement: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            show: true,
            path: "/".to_string(),
            measurement: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryConfig {
    pub show: bool,
    /// Battery slot index (0-7).
    pub slot: u8,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        Self {
            show: false,
            slot: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    pub show: bool,
    pub include_static_info: bool,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            show: false,
            include_static_info: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    pub show_graphs: bool,
    /// How many seconds of history to keep.
    pub duration_seconds: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            show_graphs: true,
            duration_seconds: 3600,
        }
    }
}

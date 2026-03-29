pub mod battery;
pub mod fan;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod processor;
pub mod storage;
pub mod system;
pub mod temperature;
pub mod voltage;

use std::sync::Arc;

use crate::config::AppConfig;

/// Identifies which category a sensor belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SensorCategory {
    Temperature,
    Voltage,
    Fan,
    Memory,
    Processor,
    System,
    Network,
    Storage,
    Battery,
    Gpu(u8),
}

impl std::fmt::Display for SensorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Temperature => write!(f, "temperature"),
            Self::Voltage => write!(f, "voltage"),
            Self::Fan => write!(f, "fan"),
            Self::Memory => write!(f, "memory"),
            Self::Processor => write!(f, "processor"),
            Self::System => write!(f, "system"),
            Self::Network => write!(f, "network"),
            Self::Storage => write!(f, "storage"),
            Self::Battery => write!(f, "battery"),
            Self::Gpu(n) => write!(f, "gpu#{n}"),
        }
    }
}

/// The physical unit/format of a reading, maps to values.js _legible() cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SensorFormat {
    Percent,
    Temp,
    Fan,
    Voltage,
    Hertz,
    Memory,
    Storage,
    Speed,
    Uptime,
    Runtime,
    Watt,
    WattGpu,
    WattHour,
    Milliamp,
    MilliampHour,
    Load,
    Pcie,
    StringVal,
}

impl SensorFormat {
    /// Returns the string key used in serialization, matching the GJS format names.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Percent => "percent",
            Self::Temp => "temp",
            Self::Fan => "fan",
            Self::Voltage => "in",
            Self::Hertz => "hertz",
            Self::Memory => "memory",
            Self::Storage => "storage",
            Self::Speed => "speed",
            Self::Uptime => "uptime",
            Self::Runtime => "runtime",
            Self::Watt => "watt",
            Self::WattGpu => "watt-gpu",
            Self::WattHour => "watt-hour",
            Self::Milliamp => "milliamp",
            Self::MilliampHour => "milliamp-hour",
            Self::Load => "load",
            Self::Pcie => "pcie",
            Self::StringVal => "string",
        }
    }

    /// Whether this format can be graphed in the history view.
    pub fn is_graphable(&self) -> bool {
        matches!(
            self,
            Self::Temp
                | Self::Voltage
                | Self::Fan
                | Self::Percent
                | Self::Hertz
                | Self::Memory
                | Self::Speed
                | Self::Storage
                | Self::Watt
                | Self::WattGpu
                | Self::Milliamp
                | Self::MilliampHour
                | Self::Load
        )
    }
}

/// A single sensor value.
#[derive(Debug, Clone)]
pub enum SensorValue {
    Numeric(f64),
    Text(String),
    Disabled,
}

impl SensorValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Numeric(v) => Some(*v),
            _ => None,
        }
    }
}

/// A single sensor reading returned by a provider.
#[derive(Debug, Clone)]
pub struct SensorReading {
    pub label: String,
    pub value: SensorValue,
    pub category: SensorCategory,
    pub format: SensorFormat,
    /// Unique key for this reading (e.g., "_temperature_coretemptemp1_temp_").
    pub key: String,
}

/// Trait that each sensor module implements.
pub trait SensorProvider: Send {
    /// One-time hardware discovery, called before first query.
    fn discover(&mut self) -> Vec<SensorReading> {
        Vec::new()
    }

    /// Query current readings. `dwell` is seconds since last poll.
    fn query(&mut self, dwell: f64) -> Vec<SensorReading>;

    /// Category this provider covers.
    fn category(&self) -> SensorCategory;

    /// Clean up resources (kill subprocesses, etc.).
    fn shutdown(&mut self) {}
}

/// Coordinator that holds all providers and manages polling.
pub struct SensorManager {
    providers: Vec<Box<dyn SensorProvider>>,
    discovered: bool,
}

impl SensorManager {
    /// Create a new SensorManager with providers based on config.
    pub fn new(config: &Arc<AppConfig>) -> Self {
        let mut providers: Vec<Box<dyn SensorProvider>> = Vec::new();

        if config.temperature.show {
            providers.push(Box::new(temperature::TemperatureProvider::new()));
        }
        if config.voltage.show {
            providers.push(Box::new(voltage::VoltageProvider::new()));
        }
        if config.fan.show {
            providers.push(Box::new(fan::FanProvider::new()));
        }
        if config.memory.show {
            providers.push(Box::new(memory::MemoryProvider::new()));
        }
        if config.processor.show {
            providers.push(Box::new(processor::ProcessorProvider::new(
                config.processor.include_static_info,
            )));
        }
        if config.system.show {
            providers.push(Box::new(system::SystemProvider::new(
                config.processor.include_static_info,
            )));
        }
        if config.network.show {
            providers.push(Box::new(network::NetworkProvider::new(
                config.network.include_public_ip,
            )));
        }
        if config.storage.show {
            providers.push(Box::new(storage::StorageProvider::new(
                config.storage.path.clone(),
            )));
        }
        if config.battery.show {
            providers.push(Box::new(battery::BatteryProvider::new(config.battery.slot)));
        }
        if config.gpu.show {
            providers.push(Box::new(gpu::GpuProvider::new(
                config.general.update_time,
                config.gpu.include_static_info,
            )));
        }

        Self {
            providers,
            discovered: false,
        }
    }

    /// Query all providers and return collected readings.
    pub fn query_all(&mut self, dwell: f64) -> Vec<SensorReading> {
        if !self.discovered {
            self.discovered = true;
            for provider in &mut self.providers {
                provider.discover();
            }
        }

        let mut readings = Vec::new();
        for provider in &mut self.providers {
            readings.extend(provider.query(dwell));
        }
        readings
    }

    /// Shutdown all providers.
    pub fn shutdown(&mut self) {
        for provider in &mut self.providers {
            provider.shutdown();
        }
    }
}

impl Drop for SensorManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub mod amd;
pub mod drm;
pub mod nvidia;

use crate::sensors::{SensorCategory, SensorProvider, SensorReading};

/// Unified GPU provider that coordinates NVIDIA (nvidia-smi) and AMD/other (DRM sysfs).
pub struct GpuProvider {
    nvidia: Option<nvidia::NvidiaGpuProvider>,
    drm_cards: Vec<drm::DrmCard>,
    update_time: u32,
    include_static_info: bool,
    discovered: bool,
}

impl GpuProvider {
    pub fn new(update_time: u32, include_static_info: bool) -> Self {
        Self {
            nvidia: None,
            drm_cards: Vec::new(),
            update_time,
            include_static_info,
            discovered: false,
        }
    }
}

impl SensorProvider for GpuProvider {
    fn discover(&mut self) -> Vec<SensorReading> {
        // Try to start nvidia-smi
        self.nvidia = nvidia::NvidiaGpuProvider::try_new(self.update_time, self.include_static_info);

        // If no nvidia-smi, discover DRM cards
        if self.nvidia.is_none() {
            self.drm_cards = drm::discover_drm_cards();
        }

        self.discovered = true;
        Vec::new()
    }

    fn query(&mut self, _dwell: f64) -> Vec<SensorReading> {
        let mut readings = Vec::new();

        if let Some(nvidia) = &mut self.nvidia {
            readings.extend(nvidia.query());
        } else {
            for card in &self.drm_cards {
                readings.extend(card.query());
            }
        }

        readings
    }

    fn category(&self) -> SensorCategory {
        SensorCategory::Gpu(1)
    }

    fn shutdown(&mut self) {
        if let Some(nvidia) = &mut self.nvidia {
            nvidia.shutdown();
        }
    }
}

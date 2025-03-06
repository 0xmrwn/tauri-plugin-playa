mod config;
mod platform;

pub use config::{Platform, PerformanceLevel, GpuVendor, SystemInfo, PresetConfig};
pub use platform::detection::detect_system_info;

// Re-export the public API functions
pub use config::{
    list_available_presets,
    get_preset_details,
    apply_preset,
    get_recommended_preset,
}; 
use std::collections::HashMap;
use std::sync::OnceLock;
use crate::Result;
use crate::Error;

// Define the platform enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOS,
    Windows,
    Linux,
}

// Define the performance level enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceLevel {
    Fast,        // Optimized for performance
    Balanced,    // Balanced performance/quality
    HighQuality, // Optimized for quality
}

// Define the GPU vendor enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    AMD,
    Intel,
    Apple,
    Unknown,
}

// Define the system info struct
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub platform: Platform,
    pub gpu_vendor: GpuVendor,
    pub is_high_end: bool,
}

// Define the preset configuration struct
#[derive(Debug, Clone)]
pub struct PresetConfig {
    pub name: String,
    pub description: String,
    pub platform: Option<Platform>,
    pub performance_level: PerformanceLevel,
    pub config_options: HashMap<String, String>,
}

// Global preset registry (initialized on first access)
static PRESET_REGISTRY: OnceLock<HashMap<String, PresetConfig>> = OnceLock::new();

// Initialize the preset registry
fn get_preset_registry() -> &'static HashMap<String, PresetConfig> {
    PRESET_REGISTRY.get_or_init(|| {
        let mut presets = HashMap::new();
        
        // Add macOS presets
        presets.insert("macos-balanced".to_string(), create_macos_balanced_preset());
        presets.insert("macos-high-quality".to_string(), create_macos_high_quality_preset());
        presets.insert("macos-fast".to_string(), create_macos_fast_preset());
        
        // Add Windows presets
        presets.insert("windows-nvidia-balanced".to_string(), create_windows_nvidia_balanced_preset());
        presets.insert("windows-amd-balanced".to_string(), create_windows_amd_balanced_preset());
        presets.insert("windows-intel-balanced".to_string(), create_windows_intel_balanced_preset());
        presets.insert("windows-nvidia-high-quality".to_string(), create_windows_nvidia_high_quality_preset());
        presets.insert("windows-amd-high-quality".to_string(), create_windows_amd_high_quality_preset());
        presets.insert("windows-intel-fast".to_string(), create_windows_intel_fast_preset());
        
        // Add Linux presets
        presets.insert("linux-balanced".to_string(), create_linux_balanced_preset());
        presets.insert("linux-high-quality".to_string(), create_linux_high_quality_preset());
        presets.insert("linux-fast".to_string(), create_linux_fast_preset());
        
        presets
    })
}

// Public API functions

/// Get a list of all available presets
pub fn list_available_presets() -> Vec<String> {
    get_preset_registry().keys().cloned().collect()
}

/// Get details about a specific preset
pub fn get_preset_details(preset_name: &str) -> Option<&'static PresetConfig> {
    get_preset_registry().get(preset_name)
}

/// Apply a preset to the mpv configuration
pub fn apply_preset(preset_name: &str) -> Result<Vec<String>> {
    match get_preset_registry().get(preset_name) {
        Some(preset) => {
            // Convert preset to mpv command line arguments
            let args: Vec<String> = preset.config_options
                .iter()
                .map(|(key, value)| format!("--{}={}", key, value))
                .collect();
            
            Ok(args)
        },
        None => Err(Error::ConfigError(format!("Preset '{}' not found", preset_name))),
    }
}

/// Get the recommended preset based on the current system
pub fn get_recommended_preset() -> String {
    let system_info = super::platform::detection::detect_system_info();
    
    match system_info.platform {
        Platform::MacOS => {
            if system_info.is_high_end {
                "macos-high-quality".to_string()
            } else {
                "macos-balanced".to_string()
            }
        },
        Platform::Windows => {
            match system_info.gpu_vendor {
                GpuVendor::Nvidia => {
                    if system_info.is_high_end {
                        "windows-nvidia-high-quality".to_string()
                    } else {
                        "windows-nvidia-balanced".to_string()
                    }
                },
                GpuVendor::AMD => {
                    if system_info.is_high_end {
                        "windows-amd-high-quality".to_string()
                    } else {
                        "windows-amd-balanced".to_string()
                    }
                },
                GpuVendor::Intel => {
                    if system_info.is_high_end {
                        "windows-intel-balanced".to_string()
                    } else {
                        "windows-intel-fast".to_string()
                    }
                },
                _ => "windows-nvidia-balanced".to_string(),
            }
        },
        Platform::Linux => {
            if system_info.is_high_end {
                "linux-high-quality".to_string()
            } else {
                "linux-balanced".to_string()
            }
        },
    }
}

// Preset creation functions

// macOS Presets
fn create_macos_balanced_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("vo".to_string(), "gpu-next".to_string());
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-context".to_string(), "macvk".to_string());
    config_options.insert("hwdec".to_string(), "videotoolbox".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Performance optimizations
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // Color management
    config_options.insert("target-colorspace-hint".to_string(), "yes".to_string());
    config_options.insert("icc-profile-auto".to_string(), "yes".to_string());
    
    // Scaling and quality (balanced)
    config_options.insert("scale".to_string(), "spline36".to_string());
    config_options.insert("dscale".to_string(), "mitchell".to_string());
    config_options.insert("cscale".to_string(), "spline36".to_string());
    
    // Audio settings
    config_options.insert("ao".to_string(), "coreaudio".to_string());
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "macos-balanced".to_string(),
        description: "Balanced preset for macOS with Apple Silicon".to_string(),
        platform: Some(Platform::MacOS),
        performance_level: PerformanceLevel::Balanced,
        config_options,
    }
}

fn create_macos_high_quality_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("vo".to_string(), "gpu-next".to_string());
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-context".to_string(), "macvk".to_string());
    config_options.insert("hwdec".to_string(), "videotoolbox".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Performance optimizations
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // Color management and HDR
    config_options.insert("target-colorspace-hint".to_string(), "yes".to_string());
    config_options.insert("icc-profile-auto".to_string(), "yes".to_string());
    config_options.insert("target-prim".to_string(), "apple".to_string());
    config_options.insert("target-trc".to_string(), "gamma2.2".to_string());
    config_options.insert("hdr-compute-peak".to_string(), "auto".to_string());
    
    // Higher quality scaling options
    config_options.insert("scale".to_string(), "ewa_lanczossharp".to_string());
    config_options.insert("dscale".to_string(), "ewa_lanczos".to_string());
    config_options.insert("cscale".to_string(), "ewa_lanczos".to_string());
    
    // Advanced rendering options
    config_options.insert("gpu-dumb-mode".to_string(), "no".to_string());
    config_options.insert("deband".to_string(), "yes".to_string());
    config_options.insert("deband-iterations".to_string(), "2".to_string());
    config_options.insert("deband-threshold".to_string(), "35".to_string());
    
    // Audio settings
    config_options.insert("ao".to_string(), "coreaudio".to_string());
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "macos-high-quality".to_string(),
        description: "High quality preset for macOS with Apple Silicon".to_string(),
        platform: Some(Platform::MacOS),
        performance_level: PerformanceLevel::HighQuality,
        config_options,
    }
}

fn create_macos_fast_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings (optimized for speed)
    config_options.insert("vo".to_string(), "gpu".to_string()); // Use standard GPU renderer
    config_options.insert("hwdec".to_string(), "videotoolbox".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Performance optimizations
    config_options.insert("video-sync".to_string(), "audio".to_string()); // Less demanding sync method
    config_options.insert("interpolation".to_string(), "no".to_string()); // Disable interpolation
    
    // Fast scaling options
    config_options.insert("scale".to_string(), "bilinear".to_string());
    config_options.insert("dscale".to_string(), "bilinear".to_string());
    config_options.insert("cscale".to_string(), "bilinear".to_string());
    
    // Disable demanding features
    config_options.insert("deband".to_string(), "no".to_string());
    
    // Audio settings
    config_options.insert("ao".to_string(), "coreaudio".to_string());
    config_options.insert("audio-channels".to_string(), "stereo".to_string()); // Force stereo for performance
    
    PresetConfig {
        name: "macos-fast".to_string(),
        description: "Fast preset for macOS with lower-end hardware".to_string(),
        platform: Some(Platform::MacOS),
        performance_level: PerformanceLevel::Fast,
        config_options,
    }
}

// Windows Presets
fn create_windows_nvidia_balanced_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-api".to_string(), "d3d11".to_string());
    config_options.insert("hwdec".to_string(), "auto-copy".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // NVIDIA-specific settings
    config_options.insert("d3d11-adapter".to_string(), "auto".to_string());
    config_options.insert("d3d11-exclusive-fs".to_string(), "yes".to_string());
    config_options.insert("d3d11-flip".to_string(), "yes".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // Scaling options (balanced quality/performance)
    config_options.insert("scale".to_string(), "spline36".to_string());
    config_options.insert("dscale".to_string(), "mitchell".to_string());
    config_options.insert("cscale".to_string(), "spline36".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "windows-nvidia-balanced".to_string(),
        description: "Balanced preset for Windows with NVIDIA GPUs".to_string(),
        platform: Some(Platform::Windows),
        performance_level: PerformanceLevel::Balanced,
        config_options,
    }
}

fn create_windows_amd_balanced_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-api".to_string(), "d3d11".to_string());
    config_options.insert("hwdec".to_string(), "auto-copy".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // AMD-specific settings
    config_options.insert("d3d11-adapter".to_string(), "auto".to_string());
    config_options.insert("d3d11-exclusive-fs".to_string(), "yes".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // Scaling options (balanced quality/performance)
    config_options.insert("scale".to_string(), "spline36".to_string());
    config_options.insert("dscale".to_string(), "mitchell".to_string());
    config_options.insert("cscale".to_string(), "spline36".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "windows-amd-balanced".to_string(),
        description: "Balanced preset for Windows with AMD GPUs".to_string(),
        platform: Some(Platform::Windows),
        performance_level: PerformanceLevel::Balanced,
        config_options,
    }
}

fn create_windows_intel_balanced_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-api".to_string(), "d3d11".to_string());
    config_options.insert("hwdec".to_string(), "auto-copy".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Intel-specific settings
    config_options.insert("d3d11-adapter".to_string(), "auto".to_string());
    
    // Performance settings (more conservative for Intel)
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "no".to_string());
    
    // Scaling options (balanced for Intel)
    config_options.insert("scale".to_string(), "spline36".to_string());
    config_options.insert("dscale".to_string(), "mitchell".to_string());
    config_options.insert("cscale".to_string(), "spline36".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "windows-intel-balanced".to_string(),
        description: "Balanced preset for Windows with Intel GPUs".to_string(),
        platform: Some(Platform::Windows),
        performance_level: PerformanceLevel::Balanced,
        config_options,
    }
}

fn create_windows_nvidia_high_quality_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-api".to_string(), "d3d11".to_string());
    config_options.insert("hwdec".to_string(), "auto-copy".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // NVIDIA-specific settings
    config_options.insert("d3d11-adapter".to_string(), "auto".to_string());
    config_options.insert("d3d11-exclusive-fs".to_string(), "yes".to_string());
    config_options.insert("d3d11-flip".to_string(), "yes".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // High quality scaling options
    config_options.insert("scale".to_string(), "ewa_lanczossharp".to_string());
    config_options.insert("dscale".to_string(), "ewa_lanczos".to_string());
    config_options.insert("cscale".to_string(), "ewa_lanczossoft".to_string());
    
    // Advanced rendering options
    config_options.insert("deband".to_string(), "yes".to_string());
    config_options.insert("deband-iterations".to_string(), "2".to_string());
    config_options.insert("deband-threshold".to_string(), "35".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "windows-nvidia-high-quality".to_string(),
        description: "High quality preset for Windows with NVIDIA GPUs".to_string(),
        platform: Some(Platform::Windows),
        performance_level: PerformanceLevel::HighQuality,
        config_options,
    }
}

fn create_windows_amd_high_quality_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("gpu-api".to_string(), "d3d11".to_string());
    config_options.insert("hwdec".to_string(), "auto-copy".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // AMD-specific settings
    config_options.insert("d3d11-adapter".to_string(), "auto".to_string());
    config_options.insert("d3d11-exclusive-fs".to_string(), "yes".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // High quality scaling options
    config_options.insert("scale".to_string(), "ewa_lanczossharp".to_string());
    config_options.insert("dscale".to_string(), "ewa_lanczos".to_string());
    config_options.insert("cscale".to_string(), "ewa_lanczossoft".to_string());
    
    // Advanced rendering options
    config_options.insert("deband".to_string(), "yes".to_string());
    config_options.insert("deband-iterations".to_string(), "2".to_string());
    config_options.insert("deband-threshold".to_string(), "35".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "windows-amd-high-quality".to_string(),
        description: "High quality preset for Windows with AMD GPUs".to_string(),
        platform: Some(Platform::Windows),
        performance_level: PerformanceLevel::HighQuality,
        config_options,
    }
}

fn create_windows_intel_fast_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("gpu-api".to_string(), "d3d11".to_string());
    config_options.insert("hwdec".to_string(), "auto-copy".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Intel-specific settings
    config_options.insert("d3d11-adapter".to_string(), "auto".to_string());
    
    // Performance settings (optimized for speed)
    config_options.insert("video-sync".to_string(), "audio".to_string());
    config_options.insert("interpolation".to_string(), "no".to_string());
    
    // Fast scaling options
    config_options.insert("scale".to_string(), "bilinear".to_string());
    config_options.insert("dscale".to_string(), "bilinear".to_string());
    config_options.insert("cscale".to_string(), "bilinear".to_string());
    
    // Disable demanding features
    config_options.insert("deband".to_string(), "no".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "stereo".to_string());
    
    PresetConfig {
        name: "windows-intel-fast".to_string(),
        description: "Fast preset for Windows with Intel GPUs".to_string(),
        platform: Some(Platform::Windows),
        performance_level: PerformanceLevel::Fast,
        config_options,
    }
}

// Linux Presets
fn create_linux_balanced_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("vo".to_string(), "gpu".to_string());
    config_options.insert("hwdec".to_string(), "auto-safe".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // Scaling options (balanced quality/performance)
    config_options.insert("scale".to_string(), "spline36".to_string());
    config_options.insert("dscale".to_string(), "mitchell".to_string());
    config_options.insert("cscale".to_string(), "spline36".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "linux-balanced".to_string(),
        description: "Balanced preset for Linux".to_string(),
        platform: Some(Platform::Linux),
        performance_level: PerformanceLevel::Balanced,
        config_options,
    }
}

fn create_linux_high_quality_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings
    config_options.insert("profile".to_string(), "gpu-hq".to_string());
    config_options.insert("vo".to_string(), "gpu".to_string());
    config_options.insert("hwdec".to_string(), "auto-safe".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "display-resample".to_string());
    config_options.insert("interpolation".to_string(), "yes".to_string());
    
    // High quality scaling options
    config_options.insert("scale".to_string(), "ewa_lanczossharp".to_string());
    config_options.insert("dscale".to_string(), "ewa_lanczos".to_string());
    config_options.insert("cscale".to_string(), "ewa_lanczossoft".to_string());
    
    // Advanced rendering options
    config_options.insert("deband".to_string(), "yes".to_string());
    config_options.insert("deband-iterations".to_string(), "2".to_string());
    config_options.insert("deband-threshold".to_string(), "35".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "auto-safe".to_string());
    
    PresetConfig {
        name: "linux-high-quality".to_string(),
        description: "High quality preset for Linux".to_string(),
        platform: Some(Platform::Linux),
        performance_level: PerformanceLevel::HighQuality,
        config_options,
    }
}

fn create_linux_fast_preset() -> PresetConfig {
    let mut config_options = HashMap::new();
    
    // Core video settings (optimized for speed)
    config_options.insert("vo".to_string(), "gpu".to_string());
    config_options.insert("hwdec".to_string(), "auto-safe".to_string());
    config_options.insert("hwdec-codecs".to_string(), "all".to_string());
    
    // Performance settings
    config_options.insert("video-sync".to_string(), "audio".to_string());
    config_options.insert("interpolation".to_string(), "no".to_string());
    
    // Fast scaling options
    config_options.insert("scale".to_string(), "bilinear".to_string());
    config_options.insert("dscale".to_string(), "bilinear".to_string());
    config_options.insert("cscale".to_string(), "bilinear".to_string());
    
    // Disable demanding features
    config_options.insert("deband".to_string(), "no".to_string());
    
    // Audio settings
    config_options.insert("audio-channels".to_string(), "stereo".to_string());
    
    PresetConfig {
        name: "linux-fast".to_string(),
        description: "Fast preset for Linux".to_string(),
        platform: Some(Platform::Linux),
        performance_level: PerformanceLevel::Fast,
        config_options,
    }
} 
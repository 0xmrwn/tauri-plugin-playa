use crate::core::presets::config::{Platform, GpuVendor, SystemInfo};
use std::process::Command;

pub fn detect_system_info() -> SystemInfo {
    let platform = detect_platform();
    let gpu_vendor = detect_gpu_vendor();
    let is_high_end = detect_high_end_system();
    
    SystemInfo {
        platform,
        gpu_vendor,
        is_high_end,
    }
}

fn detect_platform() -> Platform {
    #[cfg(target_os = "macos")]
    return Platform::MacOS;
    
    #[cfg(target_os = "windows")]
    return Platform::Windows;
    
    #[cfg(target_os = "linux")]
    return Platform::Linux;
    
    // Fallback (shouldn't happen with proper cfg attributes)
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return Platform::Linux;
}

fn detect_gpu_vendor() -> GpuVendor {
    #[cfg(target_os = "macos")]
    {
        // On macOS, we can assume Apple Silicon or Intel
        // Check if we're on Apple Silicon
        if let Ok(output) = Command::new("sysctl").args(&["-n", "machdep.cpu.brand_string"]).output() {
            let cpu_info = String::from_utf8_lossy(&output.stdout);
            if cpu_info.contains("Apple") {
                return GpuVendor::Apple;
            }
        }
        // Otherwise assume Intel
        return GpuVendor::Intel;
    }
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, try to detect using WMIC
        if let Ok(output) = Command::new("wmic").args(&["path", "win32_VideoController", "get", "name"]).output() {
            let gpu_info = String::from_utf8_lossy(&output.stdout).to_lowercase();
            
            if gpu_info.contains("nvidia") {
                return GpuVendor::Nvidia;
            } else if gpu_info.contains("amd") || gpu_info.contains("radeon") || gpu_info.contains("ati") {
                return GpuVendor::AMD;
            } else if gpu_info.contains("intel") {
                return GpuVendor::Intel;
            }
        }
        
        return GpuVendor::Unknown;
    }
    
    #[cfg(target_os = "linux")]
    {
        // On Linux, try to detect using lspci
        if let Ok(output) = Command::new("lspci").args(&["-v"]).output() {
            let gpu_info = String::from_utf8_lossy(&output.stdout).to_lowercase();
            
            if gpu_info.contains("nvidia") {
                return GpuVendor::Nvidia;
            } else if gpu_info.contains("amd") || gpu_info.contains("radeon") || gpu_info.contains("ati") {
                return GpuVendor::AMD;
            } else if gpu_info.contains("intel") {
                return GpuVendor::Intel;
            }
        }
        
        return GpuVendor::Unknown;
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return GpuVendor::Unknown;
}

fn detect_high_end_system() -> bool {
    // This is a simplified implementation
    // In a real-world scenario, we would check more system parameters
    
    #[cfg(target_os = "macos")]
    {
        // Check if we're on Apple Silicon M1 Pro/Max or M2/M3
        if let Ok(output) = Command::new("sysctl").args(&["-n", "machdep.cpu.brand_string"]).output() {
            let cpu_info = String::from_utf8_lossy(&output.stdout);
            if cpu_info.contains("M1 Pro") || cpu_info.contains("M1 Max") || 
               cpu_info.contains("M1 Ultra") || cpu_info.contains("M2") || 
               cpu_info.contains("M3") {
                return true;
            }
        }
        
        return false;
    }
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, check for high-end GPUs
        if let Ok(output) = Command::new("wmic").args(&["path", "win32_VideoController", "get", "name"]).output() {
            let gpu_info = String::from_utf8_lossy(&output.stdout).to_lowercase();
            
            // Check for high-end NVIDIA GPUs
            if gpu_info.contains("rtx") || gpu_info.contains("gtx 1080") || gpu_info.contains("gtx 1070") {
                return true;
            }
            
            // Check for high-end AMD GPUs
            if gpu_info.contains("rx 6") || gpu_info.contains("rx 5") || gpu_info.contains("vega") {
                return true;
            }
        }
        
        return false;
    }
    
    #[cfg(target_os = "linux")]
    {
        // On Linux, check for high-end GPUs using lspci
        if let Ok(output) = Command::new("lspci").args(&["-v"]).output() {
            let gpu_info = String::from_utf8_lossy(&output.stdout).to_lowercase();
            
            // Check for high-end NVIDIA GPUs
            if gpu_info.contains("rtx") || gpu_info.contains("gtx 1080") || gpu_info.contains("gtx 1070") {
                return true;
            }
            
            // Check for high-end AMD GPUs
            if gpu_info.contains("rx 6") || gpu_info.contains("rx 5") || gpu_info.contains("vega") {
                return true;
            }
        }
        
        return false;
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return false;
} 
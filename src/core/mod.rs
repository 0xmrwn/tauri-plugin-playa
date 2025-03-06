pub mod plugin;
pub mod player;
pub mod config;
pub mod presets;

use std::path::PathBuf;

/// Get the path to the mpv_config directory
pub fn get_assets_path() -> PathBuf {
    // In development, use a relative path from the crate root
    let current_dir = std::env::current_dir().unwrap_or_default();
    
    // First try the compiled-in path from build.rs
    let out_dir = std::env::var("OUT_DIR").ok();
    if let Some(dir) = out_dir {
        let path = PathBuf::from(dir).join("mpv_config");
        if path.exists() {
            return path;
        }
    }
    
    // Fall back to looking in the current directory structure
    let mut path = current_dir.join("mpv_config");
    if path.exists() {
        return path;
    }
    
    // Try parent directory
    path = current_dir.join("../mpv_config");
    if path.exists() {
        return path;
    }
    
    // Last resort, return a default path
    current_dir.join("mpv_config")
} 
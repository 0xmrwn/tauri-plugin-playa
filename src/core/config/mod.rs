use crate::Result;
use log::{debug, info};
use std::fs;
use std::path::PathBuf;

pub mod ipc;

/// Ensures the configuration directory exists
pub fn ensure_config_dir() -> Result<PathBuf> {
    let config_dir = get_mpv_config_path();
    
    if !config_dir.exists() {
        debug!("Creating config directory: {}", config_dir.display());
        fs::create_dir_all(&config_dir)?;
        info!("Created config directory: {}", config_dir.display());
    }
    
    Ok(config_dir)
}

/// Returns the path to the mpv configuration directory
pub fn get_mpv_config_path() -> PathBuf {
    crate::core::get_assets_path()
}

/// Initializes the default configuration
pub fn initialize_default_config() -> Result<()> {
    // Ensure the config directory exists
    ensure_config_dir()?;
    
    // Initialize IPC configuration
    ipc::cleanup_old_ipc_sockets()?;
    
    Ok(())
} 
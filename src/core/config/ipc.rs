use crate::Result;
use log::debug;
use std::path::PathBuf;
use std::fs;

/// Default timeout for IPC connections in milliseconds
pub const DEFAULT_IPC_TIMEOUT_MS: u64 = 5000;

/// Default polling interval for IPC events in milliseconds
pub const DEFAULT_IPC_POLL_INTERVAL_MS: u64 = 1000;

/// Default maximum number of reconnection attempts
pub const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Default reconnection delay in milliseconds
pub const DEFAULT_RECONNECT_DELAY_MS: u64 = 500;

/// IPC configuration options
#[derive(Debug, Clone)]
pub struct IpcConfig {
    /// Timeout for IPC connections in milliseconds
    pub timeout_ms: u64,
    
    /// Polling interval for IPC events in milliseconds
    pub poll_interval_ms: u64,
    
    /// Whether to automatically reconnect on connection loss
    pub auto_reconnect: bool,
    
    /// Maximum number of reconnection attempts
    pub max_reconnect_attempts: u32,
    
    /// Delay between reconnection attempts in milliseconds
    pub reconnect_delay_ms: u64,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_IPC_TIMEOUT_MS,
            poll_interval_ms: DEFAULT_IPC_POLL_INTERVAL_MS,
            auto_reconnect: true,
            max_reconnect_attempts: DEFAULT_MAX_RECONNECT_ATTEMPTS,
            reconnect_delay_ms: DEFAULT_RECONNECT_DELAY_MS,
        }
    }
}

impl IpcConfig {
    /// Creates a new IPC configuration with custom values
    pub fn new(
        timeout_ms: u64,
        poll_interval_ms: u64,
        auto_reconnect: bool,
        max_reconnect_attempts: u32,
        reconnect_delay_ms: u64,
    ) -> Self {
        Self {
            timeout_ms,
            poll_interval_ms,
            auto_reconnect,
            max_reconnect_attempts,
            reconnect_delay_ms,
        }
    }
    
    /// Creates a new IPC configuration with reconnection disabled
    pub fn without_reconnect() -> Self {
        Self {
            timeout_ms: DEFAULT_IPC_TIMEOUT_MS,
            poll_interval_ms: DEFAULT_IPC_POLL_INTERVAL_MS,
            auto_reconnect: false,
            max_reconnect_attempts: 0,
            reconnect_delay_ms: DEFAULT_RECONNECT_DELAY_MS,
        }
    }
    
    /// Creates a new IPC configuration with more aggressive reconnection settings
    pub fn with_aggressive_reconnect() -> Self {
        Self {
            timeout_ms: DEFAULT_IPC_TIMEOUT_MS,
            poll_interval_ms: DEFAULT_IPC_POLL_INTERVAL_MS,
            auto_reconnect: true,
            max_reconnect_attempts: 10,
            reconnect_delay_ms: 250,
        }
    }
}

/// Ensures the IPC socket directory exists
pub fn ensure_ipc_socket_dir() -> Result<PathBuf> {
    let socket_dir = if cfg!(target_family = "unix") {
        // On Unix, use /tmp directory
        PathBuf::from("/tmp")
    } else {
        // On Windows, use the temporary directory
        let temp_dir = std::env::temp_dir();
        temp_dir
    };
    
    if !socket_dir.exists() {
        debug!("Creating IPC socket directory: {}", socket_dir.display());
        fs::create_dir_all(&socket_dir)?;
    }
    
    Ok(socket_dir)
}

/// Cleans up old IPC sockets
pub fn cleanup_old_ipc_sockets() -> Result<()> {
    let socket_dir = ensure_ipc_socket_dir()?;
    
    if cfg!(target_family = "unix") {
        // On Unix, look for socket files with the format "mpv-socket-*"
        let entries = fs::read_dir(socket_dir)?;
        
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                
                if path.is_file() {
                    if let Some(filename) = path.file_name() {
                        if let Some(filename_str) = filename.to_str() {
                            if filename_str.starts_with("mpv-socket-") {
                                // Try to delete the socket file
                                if let Err(e) = fs::remove_file(&path) {
                                    debug!("Failed to remove old socket file {}: {}", path.display(), e);
                                } else {
                                    debug!("Removed old socket file: {}", path.display());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // On Windows, named pipes are automatically cleaned up by the OS
    
    Ok(())
} 
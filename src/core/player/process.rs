use crate::{Error, Result};
use log::{debug, error, info, warn};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use uuid::Uuid;
use crate::core::player::events::MpvEventListener;
use crate::core::plugin::WindowOptions;

/// Options for spawning mpv
#[derive(Debug, Clone, Default)]
pub struct SpawnOptions {
    /// Preset to use (default, high-quality, low-latency, etc.)
    pub preset: Option<String>,
    /// Additional mpv arguments
    pub extra_args: Vec<String>,
    /// Window configuration options
    pub window: Option<WindowOptions>,
}

impl From<&crate::core::plugin::PlaybackOptions> for SpawnOptions {
    fn from(options: &crate::core::plugin::PlaybackOptions) -> Self {
        let mut extra_args = options.extra_args.clone();
        
        // Convert start_time to argument
        if let Some(start_time) = options.start_time {
            extra_args.push(format!("--start={}", start_time));
        }
        
        // Convert title to argument
        if let Some(title) = &options.title {
            extra_args.push(format!("--title={}", title));
        }
        
        Self {
            preset: options.preset.clone(),
            extra_args,
            window: options.window.clone(),
        }
    }
}

/// Validates configuration files to ensure they don't have common issues
/// like trailing spaces after boolean values
fn validate_config_files() -> Result<()> {
    let script_opts_dir = {
        let mut path = crate::core::get_assets_path();
        path.push("script-opts");
        path
    };
    
    if !script_opts_dir.exists() {
        warn!("Script options directory not found at: {}", script_opts_dir.display());
        return Ok(());
    }
    
    let config_files = vec![
        "uosc.conf",
        "mpv.conf",
        "input.conf"
    ];
    
    for file_name in config_files {
        let file_path = script_opts_dir.join(file_name);
        if !file_path.exists() {
            debug!("Config file not found, skipping: {}", file_path.display());
            continue;
        }
        
        debug!("Validating config file: {}", file_path.display());
        validate_config_file(&file_path)?;
    }
    
    Ok(())
}

/// Validates a single configuration file for common issues
fn validate_config_file(file_path: &PathBuf) -> Result<()> {
    let file = fs::File::open(file_path)
        .map_err(|e| Error::ConfigError(format!("Failed to open config file {}: {}", file_path.display(), e)))?;
    
    let reader = BufReader::new(file);
    let mut fixed_lines = Vec::new();
    let mut needs_fixing = false;
    
    for line in reader.lines() {
        let line = line.map_err(|e| Error::ConfigError(format!("Failed to read line from {}: {}", file_path.display(), e)))?;
        
        // Check for boolean values with trailing spaces
        if line.contains("=yes ") || line.contains("=no ") {
            let fixed_line = line.replace("=yes ", "=yes").replace("=no ", "=no");
            fixed_lines.push(fixed_line);
            needs_fixing = true;
            warn!("Fixed trailing space in boolean value in {}: '{}'", file_path.display(), line);
        } else {
            fixed_lines.push(line);
        }
    }
    
    // Write back the fixed file if needed
    if needs_fixing {
        fs::write(file_path, fixed_lines.join("\n"))
            .map_err(|e| Error::ConfigError(format!("Failed to write fixed config file {}: {}", file_path.display(), e)))?;
        info!("Fixed configuration file: {}", file_path.display());
    }
    
    Ok(())
}

/// Generates a unique socket path for IPC communication.
pub fn generate_socket_path() -> String {
    #[cfg(target_family = "unix")]
    {
        format!("/tmp/mpv-socket-{}", Uuid::new_v4())
    }
    
    #[cfg(target_family = "windows")]
    {
        format!("\\\\.\\pipe\\mpv-socket-{}", Uuid::new_v4())
    }
}

/// Applies window options to mpv command arguments
fn apply_window_options(args: &mut Vec<String>, window: &WindowOptions) {
    // Apply borderless window mode
    if window.borderless {
        args.push("--border=no".to_string());
        args.push("--no-window-decorations".to_string());
    }
    
    // Build geometry string
    let mut geometry = String::new();
    
    // Add size if specified
    if let Some((width, height)) = window.size {
        geometry.push_str(&format!("{}x{}", width, height));
    }
    
    // Add position if specified
    if let Some((x, y)) = window.position {
        geometry.push_str(&format!("+{}+{}", x, y));
    }
    
    // Apply geometry if not empty
    if !geometry.is_empty() {
        args.push(format!("--geometry={}", geometry));
    }
    
    // Apply always on top
    if window.always_on_top {
        args.push("--ontop".to_string());
    }
    
    // Apply window opacity
    if let Some(opacity) = window.opacity {
        // Clamp opacity between 0.0 and 1.0
        let opacity = opacity.max(0.0).min(1.0);
        args.push(format!("--alpha={}", opacity));
    }
    
    // Apply hidden start
    if window.start_hidden {
        args.push("--force-window=yes".to_string());
        args.push("--start-hidden".to_string());
    }
    
    // Platform-specific options
    #[cfg(target_os = "windows")]
    {
        // On Windows, add extra options for proper borderless windows if needed
        if window.borderless {
            args.push("--no-border".to_string());
        }
        
        // Handle DPI awareness
        args.push("--hidpi-window-scale=no".to_string());
    }
    
    #[cfg(target_os = "linux")]
    {
        // On Linux, add X11-specific options if needed
        if window.borderless {
            args.push("--x11-name=mpv-borderless".to_string());
        }
    }
}

/// Spawns mpv with the specified media file or URL and options.
/// Returns the process handle and socket path for IPC communication.
pub fn spawn_mpv(
    file_or_url: &str, 
    options: &SpawnOptions
) -> Result<(Child, String)> {
    info!("Launching mpv for media: {}", file_or_url);
    
    // Validate configuration files before launching mpv
    if let Err(e) = validate_config_files() {
        warn!("Error validating config files: {}. Continuing anyway...", e);
    }

    // Generate a unique socket path for IPC
    let socket_path = generate_socket_path();
    debug!("Generated IPC socket path: {}", socket_path);

    // Build args using mpv's --option=value format
    let mut args = Vec::<String>::new();
    
    // Add verbose flag to see script loading errors
    args.push("--msg-level=all=v".to_string());
    
    // Add configuration directory
    let config_dir_path = get_mpv_config_path();
    args.push(format!("--config-dir={}", config_dir_path.to_str().unwrap()));
    
    // Ensure uosc is used instead of the standard OSC
    args.push("--osc=no".to_string());
    args.push("--osd-bar=no".to_string());
    
    // Enable the JSON IPC server
    args.push(format!("--input-ipc-server={}", socket_path));
    
    // Apply preset from configuration
    if let Some(preset_name) = &options.preset {
        log::debug!("Applying preset: {}", preset_name);
        match crate::core::presets::apply_preset(preset_name) {
            Ok(preset_args) => {
                args.extend(preset_args.into_iter().map(|s| s.to_string()));
            }
            Err(e) => {
                log::warn!("Failed to apply preset '{}': {}", preset_name, e);
            }
        }
    }
    
    // Apply window options if provided
    if let Some(window) = &options.window {
        apply_window_options(&mut args, window);
    } else {
        // Default behavior - add border=no
        args.push("--border=no".to_string());
    }
    
    // Add any extra arguments (these will override preset settings)
    args.extend(options.extra_args.iter().cloned());
    
    // Add the file or URL as the last argument
    args.push(file_or_url.to_string());

    debug!("MPV arguments: {:?}", args);

    // Spawn mpv asynchronously
    match Command::new("mpv").args(&args).spawn() {
        Ok(child) => {
            debug!("MPV process spawned with PID: {:?}", child.id());
            if !Path::new(&socket_path).exists() {
                error!("Socket file not created: {}", socket_path);
                return Err(Error::MpvError("Failed to create socket file".to_string()));
            }
            
            Ok((child, socket_path))
        }
        Err(e) => {
            error!("Failed to start mpv process");
            Err(Error::Io(e.to_string()))
        }
    }
}

/// Spawns mpv with the specified media file or URL.
/// Additional command-line arguments can override default configurations.
/// Returns the process handle and socket path for IPC communication.
pub fn spawn_mpv_legacy(file_or_url: &str, extra_args: &[&str]) -> Result<(Child, String)> {
    let options = SpawnOptions {
        extra_args: extra_args.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    
    spawn_mpv(file_or_url, &options)
}

/// Spawns mpv with the specified media file or URL and a preset.
/// The preset will override default configurations, and extra_args can override preset settings.
/// Returns the process handle and socket path for IPC communication.
pub fn spawn_mpv_with_preset_legacy(file_or_url: &str, preset_name: Option<&str>, extra_args: &[&str]) -> Result<(Child, String)> {
    let options = SpawnOptions {
        preset: preset_name.map(|s| s.to_string()),
        extra_args: extra_args.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    
    spawn_mpv(file_or_url, &options)
}

/// Returns the path to the dedicated mpv configuration directory.
fn get_mpv_config_path() -> PathBuf {
    crate::core::get_assets_path()
}

/// Monitors an mpv process and handles its exit
#[allow(dead_code)]
pub fn monitor_process(process: &mut Child, event_listener: &mut MpvEventListener) -> Result<i32> {
    debug!("Starting to monitor mpv process");
    
    // Wait for the process to exit
    let status = process.wait()?;
    let exit_code = status.code().unwrap_or(-1);
    
    debug!("MPV process exited with status: {}, exit code: {}", status, exit_code);
    
    // Handle the process exit in the event listener
    if let Err(e) = event_listener.handle_process_exit() {
        error!("Error handling process exit: {}", e);
    }
    
    Ok(exit_code)
} 
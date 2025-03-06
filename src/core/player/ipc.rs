use crate::{Error, Result};
use log::{debug, error};
use serde_json::{Value, json};
use std::io::{Write, BufRead, BufReader};
use std::time::{Duration, Instant};
use crate::core::config::ipc::IpcConfig;

#[cfg(target_family = "unix")]
use std::os::unix::net::UnixStream;

#[cfg(target_family = "windows")]
use std::fs::OpenOptions;
#[cfg(target_family = "windows")]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(target_family = "windows")]
use std::os::windows::io::{FromRawHandle, IntoRawHandle};
#[cfg(target_family = "windows")]
use winapi::um::fileapi::CreateFileA;
#[cfg(target_family = "windows")]
use winapi::um::winbase::{FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX};
#[cfg(target_family = "windows")]
use winapi::um::winnt::{GENERIC_READ, GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE};
#[cfg(target_family = "windows")]
use std::ffi::CString;
#[cfg(target_family = "windows")]
use std::ptr;
#[cfg(target_family = "windows")]
use std::io;

/// Client for communicating with mpv via JSON IPC.
pub struct MpvIpcClient {
    #[cfg(target_family = "unix")]
    socket: UnixStream,
    
    #[cfg(target_family = "windows")]
    socket: std::fs::File,
    
    request_id: u64,
    connected: bool,
    socket_path: String,
    config: IpcConfig,
    reconnect_attempts: u32,
    last_reconnect_time: Option<Instant>,
    intentionally_closed: bool,
}

impl MpvIpcClient {
    /// Connects to the mpv JSON IPC socket.
    pub fn connect(socket_path: &str) -> Result<Self> {
        Self::connect_with_config(socket_path, IpcConfig::default())
    }
    
    /// Connects to the mpv JSON IPC socket with custom IPC configuration.
    pub fn connect_with_config(socket_path: &str, config: IpcConfig) -> Result<Self> {
        debug!("Connecting to mpv IPC socket: {}", socket_path);
        
        let mut attempts = 0;
        let max_attempts = config.max_reconnect_attempts;
        let mut delay_ms = config.reconnect_delay_ms;
        
        // Retry loop for initial connection
        loop {
            // Check if socket file exists before attempting to connect (Unix only)
            #[cfg(target_family = "unix")]
            {
                let socket_path_exists = std::path::Path::new(socket_path).exists();
                if !socket_path_exists && attempts > 0 {
                    debug!("Socket path does not exist yet, waiting for mpv to create it. Attempt {}/{}", 
                           attempts + 1, max_attempts);
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    attempts += 1;
                    delay_ms = std::cmp::min(delay_ms * 2, 1000); // Exponential backoff, capped at 1 second
                    
                    if attempts >= max_attempts {
                        return Err(Error::MpvError(format!("Socket path not found after {} attempts", max_attempts)));
                    }
                    continue;
                }
            }
            
            #[cfg(target_family = "unix")]
            {
                match UnixStream::connect(socket_path) {
                    Ok(socket) => {
                        debug!("Successfully connected to mpv IPC socket");
                        return Ok(Self { 
                            socket, 
                            request_id: 1, 
                            connected: true,
                            socket_path: socket_path.to_string(),
                            config,
                            reconnect_attempts: 0,
                            last_reconnect_time: None,
                            intentionally_closed: false,
                        });
                    },
                    Err(e) => {
                        if attempts >= max_attempts {
                            error!("Failed to connect to mpv IPC socket after {} attempts: {}", max_attempts, e);
                            return Err(Error::Io(e.to_string()));
                        }
                        
                        debug!("Failed to connect to mpv IPC socket, retrying ({}/{}): {}", 
                               attempts + 1, max_attempts, e);
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                }
            }
            
            #[cfg(target_family = "windows")]
            {
                match std::fs::OpenOptions::new().read(true).write(true).open(socket_path) {
                    Ok(socket) => {
                        debug!("Successfully connected to mpv IPC socket");
                        return Ok(Self { 
                            socket, 
                            request_id: 1, 
                            connected: true,
                            socket_path: socket_path.to_string(),
                            config,
                            reconnect_attempts: 0,
                            last_reconnect_time: None,
                            intentionally_closed: false,
                        });
                    },
                    Err(e) => {
                        if attempts >= max_attempts {
                            error!("Failed to connect to mpv IPC socket after {} attempts: {}", max_attempts, e);
                            return Err(Error::Io(e.to_string()));
                        }
                        
                        debug!("Failed to connect to mpv IPC socket, retrying ({}/{}): {}", 
                               attempts + 1, max_attempts, e);
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                }
            }
        }
    }
    
    /// Attempts to reconnect to the mpv socket if disconnected
    fn reconnect(&mut self) -> Result<()> {
        // Always check intentionally_closed first before any other logic
        if self.intentionally_closed {
            debug!("Not reconnecting because client was intentionally closed");
            return Err(Error::MpvError("Client was intentionally closed".to_string()));
        }

        // If already connected, nothing to do
        if self.connected {
            return Ok(());
        }
        
        // Log the reconnection attempt and current state
        debug!("Attempting to reconnect to mpv IPC socket. Attempt: {}/{}", 
               self.reconnect_attempts + 1, 
               self.config.max_reconnect_attempts);
        
        // Check if we've reached the maximum number of reconnection attempts
        if self.reconnect_attempts >= self.config.max_reconnect_attempts {
            return Err(Error::MpvError(format!(
                "Max reconnection attempts ({}) reached", 
                self.config.max_reconnect_attempts
            )));
        }
        
        // Increment reconnection attempts
        self.reconnect_attempts += 1;
        
        let now = Instant::now();
        
        // If we recently tried to reconnect, wait a bit to avoid hammering the socket
        if let Some(last_time) = self.last_reconnect_time {
            let elapsed = now.duration_since(last_time);
            if elapsed < Duration::from_millis(self.config.reconnect_delay_ms) {
                std::thread::sleep(Duration::from_millis(self.config.reconnect_delay_ms) - elapsed);
            }
        }
        
        // Update last reconnection time
        self.last_reconnect_time = Some(now);
        
        // Check if the socket file even exists before trying to connect (Unix only)
        #[cfg(target_family = "unix")]
        {
            let socket_path = std::path::Path::new(&self.socket_path);
            if !socket_path.exists() {
                debug!("Socket path does not exist, mpv process has likely terminated");
                // Mark as intentionally closed since mpv is gone
                self.intentionally_closed = true;
                return Err(Error::MpvError("Socket file does not exist, mpv process has likely terminated".to_string()));
            }
        }
        
        // Attempt to reconnect
        #[cfg(target_family = "unix")]
        {
            match UnixStream::connect(&self.socket_path) {
                Ok(socket) => {
                    self.socket = socket;
                    self.connected = true;
                    self.reset_reconnect_attempts();
                    debug!("Successfully reconnected to mpv IPC socket");
                    return Ok(());
                },
                Err(e) => {
                    error!("Failed to reconnect to mpv IPC socket: {}", e);
                    
                    // If connection refused, mpv has likely terminated
                    if let Some(os_err) = e.raw_os_error() {
                        // ECONNREFUSED
                        if os_err == 61 || os_err == 111 {
                            debug!("Connection refused, mpv process has likely terminated");
                            // Mark as intentionally closed since mpv is gone
                            self.intentionally_closed = true;
                        }
                    }
                    
                    return Err(Error::Io(e.to_string()));
                }
            }
        }
        
        #[cfg(target_family = "windows")]
        {
            match std::fs::OpenOptions::new().read(true).write(true).open(&self.socket_path) {
                Ok(socket) => {
                    self.socket = socket;
                    self.connected = true;
                    self.reset_reconnect_attempts();
                    debug!("Successfully reconnected to mpv IPC socket");
                    return Ok(());
                },
                Err(e) => {
                    error!("Failed to reconnect to mpv IPC socket: {}", e);
                    
                    // Check for specific errors that indicate the pipe is gone
                    if e.kind() == std::io::ErrorKind::NotFound || e.kind() == std::io::ErrorKind::ConnectionRefused {
                        debug!("Named pipe not found or connection refused, mpv process has likely terminated");
                        // Mark as intentionally closed since mpv is gone
                        self.intentionally_closed = true;
                    }
                    
                    return Err(Error::Io(e.to_string()));
                }
            }
        }
    }
    
    /// Resets the reconnection attempts counter after a successful operation
    fn reset_reconnect_attempts(&mut self) {
        if self.reconnect_attempts > 0 {
            debug!("Resetting reconnection attempts counter");
            self.reconnect_attempts = 0;
        }
    }
    
    /// Sends a command to mpv with automatic reconnection if configured.
    pub fn command(&mut self, command: &str, args: &[Value]) -> Result<Value> {
        let result = self.command_internal(command, args);
        
        if let Err(ref e) = result {
            // Check if it's an IO error and auto-reconnect is enabled
            if self.should_reconnect(e) {
                debug!("Command failed, attempting to reconnect and retry");
                match self.reconnect() {
                    Ok(_) => {
                        // Retry the command after successful reconnection
                        return self.command_internal(command, args);
                    },
                    Err(reconnect_err) => {
                        error!("Failed to reconnect: {}", reconnect_err);
                        return Err(reconnect_err);
                    }
                }
            }
        } else {
            // Reset reconnection attempts after successful command
            self.reset_reconnect_attempts();
        }
        
        result
    }
    
    /// Internal implementation of command without reconnection logic
    fn command_internal(&mut self, command: &str, args: &[Value]) -> Result<Value> {
        let id = self.request_id;
        self.request_id += 1;
        
        let mut command_args = vec![Value::String(command.to_string())];
        command_args.extend_from_slice(args);
        
        let request = json!({
            "command": command_args,
            "request_id": id
        });
        
        self.send_request(&request)?;
        self.receive_response(id)
    }
    
    /// Gets a property from mpv with automatic reconnection if configured.
    pub fn get_property(&mut self, property: &str) -> Result<Value> {
        let result = self.get_property_internal(property);
        
        if let Err(ref e) = result {
            if self.should_reconnect(e) {
                debug!("Get property failed, attempting to reconnect and retry");
                match self.reconnect() {
                    Ok(_) => {
                        return self.get_property_internal(property);
                    },
                    Err(reconnect_err) => {
                        error!("Failed to reconnect: {}", reconnect_err);
                        return Err(reconnect_err);
                    }
                }
            }
        } else {
            self.reset_reconnect_attempts();
        }
        
        result
    }
    
    /// Internal implementation of get_property without reconnection logic
    fn get_property_internal(&mut self, property: &str) -> Result<Value> {
        let id = self.request_id;
        self.request_id += 1;
        
        let request = json!({
            "command": ["get_property", property],
            "request_id": id
        });
        
        self.send_request(&request)?;
        self.receive_response(id)
    }
    
    /// Sets a property in mpv with automatic reconnection if configured.
    pub fn set_property(&mut self, property: &str, value: Value) -> Result<Value> {
        let result = self.set_property_internal(property, value.clone());
        
        if let Err(ref e) = result {
            if self.should_reconnect(e) {
                debug!("Set property failed, attempting to reconnect and retry");
                match self.reconnect() {
                    Ok(_) => {
                        return self.set_property_internal(property, value);
                    },
                    Err(reconnect_err) => {
                        error!("Failed to reconnect: {}", reconnect_err);
                        return Err(reconnect_err);
                    }
                }
            }
        } else {
            self.reset_reconnect_attempts();
        }
        
        result
    }
    
    /// Internal implementation of set_property without reconnection logic
    fn set_property_internal(&mut self, property: &str, value: Value) -> Result<Value> {
        let id = self.request_id;
        self.request_id += 1;
        
        let request = json!({
            "command": ["set_property", property, value],
            "request_id": id
        });
        
        self.send_request(&request)?;
        self.receive_response(id)
    }
    
    /// Observes a property in mpv with automatic reconnection if configured.
    pub fn observe_property(&mut self, property: &str) -> Result<u64> {
        let result = self.observe_property_internal(property);
        
        if let Err(ref e) = result {
            if self.should_reconnect(e) {
                debug!("Observe property failed, attempting to reconnect and retry");
                match self.reconnect() {
                    Ok(_) => {
                        return self.observe_property_internal(property);
                    },
                    Err(reconnect_err) => {
                        error!("Failed to reconnect: {}", reconnect_err);
                        return Err(reconnect_err);
                    }
                }
            }
        } else {
            self.reset_reconnect_attempts();
        }
        
        result
    }
    
    /// Internal implementation of observe_property without reconnection logic
    fn observe_property_internal(&mut self, property: &str) -> Result<u64> {
        let id = self.request_id;
        self.request_id += 1;
        
        let request = json!({
            "command": ["observe_property", id, property],
            "request_id": id
        });
        
        self.send_request(&request)?;
        if let Value::Object(response) = self.receive_response(id)? {
            if let Some(Value::String(error)) = response.get("error") {
                if error != "success" {
                    return Err(Error::MpvError(error.clone()));
                }
            }
            
            return Ok(id);
        }
        
        Err(Error::MpvError("Invalid response format".to_string()))
    }
    
    /// Unobserves a property in mpv with automatic reconnection if configured.
    pub fn unobserve_property(&mut self, observe_id: u64) -> Result<Value> {
        let result = self.unobserve_property_internal(observe_id);
        
        if let Err(ref e) = result {
            if self.should_reconnect(e) {
                debug!("Unobserve property failed, attempting to reconnect and retry");
                match self.reconnect() {
                    Ok(_) => {
                        return self.unobserve_property_internal(observe_id);
                    },
                    Err(reconnect_err) => {
                        error!("Failed to reconnect: {}", reconnect_err);
                        return Err(reconnect_err);
                    }
                }
            }
        } else {
            self.reset_reconnect_attempts();
        }
        
        result
    }
    
    /// Internal implementation of unobserve_property without reconnection logic
    fn unobserve_property_internal(&mut self, observe_id: u64) -> Result<Value> {
        let id = self.request_id;
        self.request_id += 1;
        
        let request = json!({
            "command": ["unobserve_property", observe_id],
            "request_id": id
        });
        
        self.send_request(&request)?;
        self.receive_response(id)
    }
    
    /// Checks if we should attempt to reconnect based on the error
    fn should_reconnect(&mut self, error: &Error) -> bool {
        // Don't reconnect if auto-reconnect is disabled
        if !self.config.auto_reconnect {
            return false;
        }
        
        // Don't reconnect if we've exhausted our reconnection attempts
        if self.reconnect_attempts >= self.config.max_reconnect_attempts {
            debug!("Maximum reconnection attempts ({}) reached", self.config.max_reconnect_attempts);
            return false;
        }
        
        // Don't reconnect if we've intentionally closed the connection
        if self.intentionally_closed {
            return false;
        }
        
        match error {
            // Only attempt reconnection for specific I/O errors
            Error::Io(err_str) => {
                // Check for broken pipe, connection refused/reset
                // We can't use ErrorKind directly since we're storing the error as a string
                // So check for these specific error messages instead
                if err_str.contains("broken pipe") || 
                   err_str.contains("pipe is being closed") {
                    debug!("Broken pipe detected, will attempt reconnection");
                    return true;
                } else if err_str.contains("connection refused") {
                    debug!("Connection refused detected, will attempt reconnection");
                    return true;
                } else if err_str.contains("connection reset") {
                    debug!("Connection reset detected, will attempt reconnection");
                    return true;
                }
                
                // For other IO errors, don't attempt reconnection
                false
            },
            // For other error types, don't attempt reconnection
            _ => false,
        }
    }
    
    /// Sends a request to mpv with improved error handling
    fn send_request(&mut self, request: &Value) -> Result<()> {
        if !self.connected {
            return Err(Error::MpvError("Not connected to mpv".to_string()));
        }
        
        let request_str = request.to_string();
        
        // Add a newline to the request
        let request_bytes = format!("{}\n", request_str).into_bytes();
        
        #[cfg(target_family = "unix")] 
        {
            match self.socket.write_all(&request_bytes) {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Failed to send request: {}", e);
                    self.connected = false;
                    Err(Error::Io(e.to_string()))
                }
            }
        }
        
        #[cfg(target_family = "windows")]
        {
            match self.socket.write_all(&request_bytes) {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Failed to send request: {}", e);
                    self.connected = false;
                    Err(Error::Io(e.to_string()))
                }
            }
        }
    }
    
    /// Receives a response from mpv with improved error handling and timeout
    fn receive_response(&mut self, request_id: u64) -> Result<Value> {
        if !self.connected {
            if self.config.auto_reconnect {
                self.reconnect()?;
            } else {
                return Err(Error::MpvError("Not connected to mpv".to_string()));
            }
        }
        
        let timeout = Duration::from_millis(self.config.timeout_ms);
        let _start_time = Instant::now();
        
        #[cfg(target_family = "unix")]
        let mut reader = BufReader::new(&self.socket);
        
        #[cfg(target_family = "windows")]
        let mut reader = BufReader::new(&self.socket);
        
        // Set read timeout if available
        #[cfg(target_family = "unix")]
        {
            self.socket.set_read_timeout(Some(timeout))
                .map_err(|e| Error::Io(e.to_string()))?;
        }
        
        // Read the response
        let mut response_str = String::new();
        match reader.read_line(&mut response_str) {
            Ok(_) => {
                debug!("Received response: {}", response_str);
            },
            Err(e) => {
                error!("Failed to read response: {}", e);
                self.connected = false;
                return Err(Error::Io(e.to_string()));
            }
        }
        
        // If we reach here, we've exhausted the reader without finding a matching response
        Err(Error::MpvError(format!("No response found for request ID {}", request_id)))
    }
    
    /// Returns whether mpv is still running
    pub fn is_running(&mut self) -> bool {
        // If the client was intentionally closed, assume mpv is not running
        if self.intentionally_closed {
            debug!("is_running: client was intentionally closed, assuming mpv is not running");
            return false;
        }
        
        // If not connected, try to reconnect if enabled
        if !self.connected {
            if self.config.auto_reconnect {
                debug!("is_running: not connected, attempting to reconnect");
                if let Err(e) = self.reconnect() {
                    debug!("Failed to reconnect while checking if mpv is running: {}", e);
                    return false;
                }
            } else {
                debug!("is_running: not connected and auto-reconnect disabled");
                return false;
            }
        }
        
        // Try multiple properties to determine if mpv is running
        let pid_check = self.get_property("pid").is_ok();
        let path_check = self.get_property("path").is_ok();
        
        // Check if playback is idle (which can indicate player is about to exit)
        let idle_active = match self.get_property("idle-active") {
            Ok(value) => value.as_bool().unwrap_or(false),
            Err(_) => false,
        };
        
        // Check if EOF has been reached
        let eof_reached = match self.get_property("eof-reached") {
            Ok(value) => value.as_bool().unwrap_or(false),
            Err(_) => false,
        };
        
        // Check process state directly with a simple command
        let cmd_check = self.command("get_version", &[]).is_ok();
        
        debug!("is_running checks: pid={}, path={}, cmd={}, idle={}, eof={}", 
               pid_check, path_check, cmd_check, idle_active, eof_reached);
        
        // If basic checks pass but idle or EOF indicate termination, assume process is ending
        if (pid_check || path_check || cmd_check) && (idle_active || eof_reached) {
            debug!("Process detected as ending (idle={}, eof={})", idle_active, eof_reached);
            // Give the process a chance to exit cleanly, but mark as intentionally closed
            self.mark_as_intentionally_closed();
            return false;
        }
        
        // If any check passes, mpv is probably running
        if pid_check || path_check || cmd_check {
            true
        } else {
            // Mark as intentionally closed to prevent further reconnection attempts
            debug!("All running checks failed, marking client as intentionally closed");
            self.mark_as_intentionally_closed();
            false
        }
    }
    
    /// Returns whether the client is currently connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }
    
    /// Closes the connection to mpv.
    pub fn close(&mut self) {
        debug!("Explicitly closing IPC client connection");
        // Set the intentionally_closed flag first before any other operations
        self.intentionally_closed = true;
        
        #[cfg(target_family = "unix")]
        {
            // First try to properly close the socket
            let _ = self.socket.shutdown(std::net::Shutdown::Both);
            
            // Additionally, invalidate the connection by dropping and recreating it
            // This ensures any pending operations will fail immediately
            if let Ok(mut socket) = UnixStream::connect("/dev/null") {
                std::mem::swap(&mut self.socket, &mut socket);
                // Original socket is dropped here
            }
        }
        
        #[cfg(target_family = "windows")]
        {
            // On Windows, we can't easily invalidate the connection,
            // but we can at least set the connected flag
        }
        
        // Reset any reconnection state
        self.reconnect_attempts = 0;
        self.last_reconnect_time = None;
        
        // Update connected status
        self.connected = false;
        
        debug!("IPC client connection closed and marked as intentionally closed");
    }
    
    /// Gets the current playback time in seconds
    pub fn get_time_pos(&mut self) -> Result<f64> {
        match self.get_property("time-pos")? {
            Value::Number(n) => {
                if let Some(pos) = n.as_f64() {
                    Ok(pos)
                } else {
                    Err(Error::MpvError("Invalid time-pos format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid time-pos type".to_string()))
        }
    }
    
    /// Gets the duration of the current media in seconds
    pub fn get_duration(&mut self) -> Result<f64> {
        match self.get_property("duration")? {
            Value::Number(n) => {
                if let Some(duration) = n.as_f64() {
                    Ok(duration)
                } else {
                    Err(Error::MpvError("Invalid duration format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid duration type".to_string()))
        }
    }
    
    /// Gets the current playback position as a percentage (0-100)
    pub fn get_percent_pos(&mut self) -> Result<f64> {
        match self.get_property("percent-pos")? {
            Value::Number(n) => {
                if let Some(percent) = n.as_f64() {
                    Ok(percent)
                } else {
                    Err(Error::MpvError("Invalid percent-pos format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid percent-pos type".to_string()))
        }
    }
    
    /// Gets the current playback speed (1.0 is normal speed)
    pub fn get_speed(&mut self) -> Result<f64> {
        match self.get_property("speed")? {
            Value::Number(n) => {
                if let Some(speed) = n.as_f64() {
                    Ok(speed)
                } else {
                    Err(Error::MpvError("Invalid speed format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid speed type".to_string()))
        }
    }
    
    /// Sets the playback speed (1.0 is normal speed)
    pub fn set_speed(&mut self, speed: f64) -> Result<Value> {
        self.set_property("speed", json!(speed))
    }
    
    /// Gets the current volume level (0-100)
    pub fn get_volume(&mut self) -> Result<f64> {
        match self.get_property("volume")? {
            Value::Number(n) => {
                if let Some(volume) = n.as_f64() {
                    Ok(volume)
                } else {
                    Err(Error::MpvError("Invalid volume format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid volume type".to_string()))
        }
    }
    
    /// Sets the volume level (0-100)
    pub fn set_volume(&mut self, volume: f64) -> Result<Value> {
        self.set_property("volume", json!(volume))
    }
    
    /// Gets the current mute state
    pub fn get_mute(&mut self) -> Result<bool> {
        match self.get_property("mute")? {
            Value::Bool(mute) => Ok(mute),
            _ => Err(Error::MpvError("Invalid mute type".to_string()))
        }
    }
    
    /// Sets the mute state
    pub fn set_mute(&mut self, mute: bool) -> Result<Value> {
        self.set_property("mute", json!(mute))
    }
    
    /// Toggles mute state
    pub fn toggle_mute(&mut self) -> Result<Value> {
        let mute = self.get_mute()?;
        self.set_mute(!mute)
    }
    
    /// Gets the current pause state
    pub fn get_pause(&mut self) -> Result<bool> {
        match self.get_property("pause")? {
            Value::Bool(pause) => Ok(pause),
            _ => Err(Error::MpvError("Invalid pause type".to_string()))
        }
    }
    
    /// Sets the pause state
    pub fn set_pause(&mut self, pause: bool) -> Result<Value> {
        self.set_property("pause", json!(pause))
    }
    
    /// Toggles pause state
    pub fn toggle_pause(&mut self) -> Result<Value> {
        let pause = self.get_pause()?;
        self.set_pause(!pause)
    }
    
    /// Gets the current fullscreen state
    pub fn get_fullscreen(&mut self) -> Result<bool> {
        match self.get_property("fullscreen")? {
            Value::Bool(fullscreen) => Ok(fullscreen),
            _ => Err(Error::MpvError("Invalid fullscreen type".to_string()))
        }
    }
    
    /// Sets the fullscreen state
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<Value> {
        self.set_property("fullscreen", json!(fullscreen))
    }
    
    /// Toggles fullscreen state
    pub fn toggle_fullscreen(&mut self) -> Result<Value> {
        let fullscreen = self.get_fullscreen()?;
        self.set_fullscreen(!fullscreen)
    }
    
    /// Seeks to a specific position in seconds
    pub fn seek(&mut self, position: f64) -> Result<Value> {
        self.command("seek", &[json!(position), json!("absolute")])
    }
    
    /// Seeks to a specific percentage position (0-100)
    pub fn seek_percent(&mut self, percent: f64) -> Result<Value> {
        self.command("seek", &[json!(percent), json!("absolute-percent")])
    }
    
    /// Seeks relative to the current position (positive or negative seconds)
    pub fn seek_relative(&mut self, offset: f64) -> Result<Value> {
        self.command("seek", &[json!(offset), json!("relative")])
    }
    
    /// Gets the chapter list
    pub fn get_chapter_list(&mut self) -> Result<Vec<Value>> {
        match self.get_property("chapter-list")? {
            Value::Array(chapters) => Ok(chapters),
            _ => Err(Error::MpvError("Invalid chapter-list type".to_string()))
        }
    }
    
    /// Gets the current chapter index
    pub fn get_chapter(&mut self) -> Result<i64> {
        match self.get_property("chapter")? {
            Value::Number(n) => {
                if let Some(chapter) = n.as_i64() {
                    Ok(chapter)
                } else {
                    Err(Error::MpvError("Invalid chapter format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid chapter type".to_string()))
        }
    }
    
    /// Sets the current chapter index
    pub fn set_chapter(&mut self, chapter: i64) -> Result<Value> {
        self.set_property("chapter", json!(chapter))
    }
    
    /// Goes to the next chapter
    pub fn next_chapter(&mut self) -> Result<Value> {
        self.command("add", &[json!("chapter"), json!(1)])
    }
    
    /// Goes to the previous chapter
    pub fn prev_chapter(&mut self) -> Result<Value> {
        self.command("add", &[json!("chapter"), json!(-1)])
    }
    
    /// Gets information about the current media
    pub fn get_media_info(&mut self) -> Result<Value> {
        self.get_property("media-title")
    }
    
    /// Gets the current playlist
    pub fn get_playlist(&mut self) -> Result<Vec<Value>> {
        match self.get_property("playlist")? {
            Value::Array(playlist) => Ok(playlist),
            _ => Err(Error::MpvError("Invalid playlist type".to_string()))
        }
    }
    
    /// Gets the current playlist position
    pub fn get_playlist_pos(&mut self) -> Result<i64> {
        match self.get_property("playlist-pos")? {
            Value::Number(n) => {
                if let Some(pos) = n.as_i64() {
                    Ok(pos)
                } else {
                    Err(Error::MpvError("Invalid playlist-pos format".to_string()))
                }
            },
            _ => Err(Error::MpvError("Invalid playlist-pos type".to_string()))
        }
    }
    
    /// Sets the current playlist position
    pub fn set_playlist_pos(&mut self, pos: i64) -> Result<Value> {
        self.set_property("playlist-pos", json!(pos))
    }
    
    /// Goes to the next item in the playlist
    pub fn playlist_next(&mut self) -> Result<Value> {
        self.command("playlist-next", &[])
    }
    
    /// Goes to the previous item in the playlist
    pub fn playlist_prev(&mut self) -> Result<Value> {
        self.command("playlist-prev", &[])
    }
    
    /// Gets the number of audio tracks
    pub fn get_audio_tracks(&mut self) -> Result<Vec<Value>> {
        match self.get_property("track-list")? {
            Value::Array(tracks) => {
                let audio_tracks = tracks.into_iter()
                    .filter(|track| {
                        if let Some(Value::String(type_str)) = track.get("type") {
                            type_str == "audio"
                        } else {
                            false
                        }
                    })
                    .collect();
                Ok(audio_tracks)
            },
            _ => Err(Error::MpvError("Invalid track-list type".to_string()))
        }
    }
    
    /// Gets the number of subtitle tracks
    pub fn get_subtitle_tracks(&mut self) -> Result<Vec<Value>> {
        match self.get_property("track-list")? {
            Value::Array(tracks) => {
                let subtitle_tracks = tracks.into_iter()
                    .filter(|track| {
                        if let Some(Value::String(type_str)) = track.get("type") {
                            type_str == "sub"
                        } else {
                            false
                        }
                    })
                    .collect();
                Ok(subtitle_tracks)
            },
            _ => Err(Error::MpvError("Invalid track-list type".to_string()))
        }
    }
    
    /// Sets the current audio track
    pub fn set_audio_track(&mut self, id: i64) -> Result<Value> {
        self.set_property("aid", json!(id))
    }
    
    /// Sets the current subtitle track
    pub fn set_subtitle_track(&mut self, id: i64) -> Result<Value> {
        self.set_property("sid", json!(id))
    }
    
    /// Disables subtitles
    pub fn disable_subtitles(&mut self) -> Result<Value> {
        self.set_property("sid", json!("no"))
    }
    
    /// Takes a screenshot
    pub fn screenshot(&mut self, include_subtitles: bool) -> Result<Value> {
        let screenshot_type = if include_subtitles { "subtitles" } else { "video" };
        self.command("screenshot", &[json!(screenshot_type)])
    }
    
    /// Quits mpv
    pub fn quit(&mut self) -> Result<Value> {
        let result = self.command("quit", &[]);
        // Mark as intentionally closed after sending quit command
        self.mark_as_intentionally_closed();
        result
    }
    
    /// Gets the current playback status (playing, paused, idle)
    pub fn get_playback_status(&mut self) -> Result<String> {
        // First check if we're paused
        match self.get_pause()? {
            true => return Ok("paused".to_string()),
            false => {
                // Check if we're idle or playing
                match self.get_property("idle-active")? {
                    Value::Bool(true) => Ok("idle".to_string()),
                    Value::Bool(false) => Ok("playing".to_string()),
                    _ => Err(Error::MpvError("Invalid idle-active type".to_string()))
                }
            }
        }
    }
    
    /// Marks the client as intentionally closed, preventing reconnection attempts
    pub fn mark_as_intentionally_closed(&mut self) {
        debug!("Marking IPC client as intentionally closed");
        self.intentionally_closed = true;
        self.connected = false;
    }
    
    /// Returns the configured poll interval in milliseconds
    pub fn get_poll_interval(&self) -> u64 {
        self.config.poll_interval_ms
    }
    
    /// Returns whether the client has been intentionally closed
    pub fn is_intentionally_closed(&self) -> bool {
        self.intentionally_closed
    }
} 
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;
use tokio::task::JoinHandle as TokioJoinHandle;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use log::{debug, error};

use crate::core::player::process::{SpawnOptions, spawn_mpv};
use crate::core::player::ipc::MpvIpcClient;
use crate::core::player::events::MpvEventListener;
use crate::core::config::ipc::{IpcConfig, DEFAULT_IPC_POLL_INTERVAL_MS, DEFAULT_MAX_RECONNECT_ATTEMPTS, DEFAULT_RECONNECT_DELAY_MS};

use crate::{Result, Error};

use std::collections::HashSet;
use lazy_static::lazy_static;

// Global event deduplication cache
lazy_static! {
    static ref NOTIFIED_EVENTS: Mutex<HashMap<String, HashSet<String>>> = Mutex::new(HashMap::new());
}

/// A unique identifier for a video instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoId(Uuid);

impl VideoId {
    /// Creates a new random VideoId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    /// Converts the VideoId to a string
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
    
    /// Creates a VideoId from a string representation of a UUID
    pub fn from_string(s: &str) -> Result<Self> {
        match Uuid::parse_str(s) {
            Ok(uuid) => Ok(Self(uuid)),
            Err(e) => Err(Error::VideoIdError(format!("Invalid VideoId string: {}", e)))
        }
    }
}

/// Window configuration options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowOptions {
    /// Whether to use a borderless window
    pub borderless: bool,
    /// Window position (x, y) relative to screen
    pub position: Option<(i32, i32)>,
    /// Window size (width, height)
    pub size: Option<(u32, u32)>,
    /// Whether to make the window always on top
    pub always_on_top: bool,
    /// Alpha value for window transparency (0.0-1.0)
    pub opacity: Option<f32>,
    /// Whether to hide window on startup
    pub start_hidden: bool,
}

/// Options for video playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackOptions {
    /// Start time in seconds
    pub start_time: Option<f64>,
    /// Preset to use (default, high-quality, low-latency, etc.)
    pub preset: Option<String>,
    /// Additional mpv arguments
    pub extra_args: Vec<String>,
    /// Window title
    pub title: Option<String>,
    /// Whether to enable progress reporting
    pub report_progress: bool,
    /// Progress reporting interval in milliseconds
    pub progress_interval_ms: Option<u64>,
    /// Window configuration options
    pub window: Option<WindowOptions>,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: Option<u64>,
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            start_time: None,
            preset: None,
            extra_args: Vec::new(),
            title: None,
            report_progress: true,
            progress_interval_ms: Some(1000),
            window: None,
            connection_timeout_ms: None,
        }
    }
}

/// Events emitted by video instances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VideoEvent {
    /// Playback progress update
    Progress {
        id: VideoId,
        position: f64,
        duration: f64,
        percent: f64,
    },
    /// Video started playing
    Started { id: VideoId },
    /// Video paused
    Paused { id: VideoId },
    /// Video resumed playing
    Resumed { id: VideoId },
    /// Video playback ended
    Ended { id: VideoId },
    /// Video instance closed
    Closed { id: VideoId },
    /// Error occurred
    Error { id: VideoId, message: String },
}

/// A subscription to video events with async support
pub struct EventSubscription {
    receiver: mpsc::Receiver<VideoEvent>,
    _id: Uuid,
}

impl EventSubscription {
    /// Receives the next event, blocking until one is available
    pub async fn recv(&mut self) -> Option<VideoEvent> {
        self.receiver.recv().await
    }
}

/// Internal event subscriber
#[derive(Clone)]
struct EventSubscriber {
    id: Uuid,
    sender: mpsc::Sender<VideoEvent>,
}

/// Internal representation of a video instance
#[allow(dead_code)]
struct VideoInstance {
    id: VideoId,
    process: std::process::Child,
    ipc_client: Arc<Mutex<MpvIpcClient>>,
    event_listener: Option<MpvEventListener>,
    event_thread: Option<JoinHandle<()>>,
    socket_path: String,
}

impl Drop for VideoInstance {
    fn drop(&mut self) {
        debug!("Dropping VideoInstance with ID: {}", self.id.to_string());
        
        // First, stop the event listener to prevent any further IPC communication
        if let Some(mut event_listener) = self.event_listener.take() {
            debug!("Stopping event listener for video {}", self.id.to_string());
            let _ = event_listener.stop_listening();
            let _ = event_listener.handle_process_exit();
        }

        // Attempt to quit mpv gracefully and mark IPC as intentionally closed
        if let Some(mut client) = self.ipc_client.lock().ok() {
            debug!("Sending quit command to mpv for video {}", self.id.to_string());
            // quit() now marks the connection as intentionally closed
            let _ = client.quit();
            
            // For extra safety, explicitly close the connection
            client.close();
        }
        
        // Wait briefly for process to exit gracefully
        use std::thread::sleep;
        use std::time::Duration;
        sleep(Duration::from_millis(100));
        
        // Kill the process if it's still running
        let _ = self.process.kill();
        
        // Join the event thread if it exists
        if let Some(thread) = self.event_thread.take() {
            debug!("Joining event thread for video {}", self.id.to_string());
            let _ = thread.join();
        }
        
        debug!("VideoInstance with ID {} successfully dropped", self.id.to_string());
    }
}

/// Detailed video information
pub struct VideoInfo {
    /// Path or URL of the video
    pub path: String,
    /// Current playback position in seconds
    pub position: f64,
    /// Total duration of the video in seconds
    pub duration: f64,
    /// Current volume (0-100)
    pub volume: f64,
    /// Whether playback is currently paused
    pub is_paused: bool,
    /// Playback speed (1.0 = normal)
    pub speed: f64,
    /// Whether audio is muted
    pub is_muted: bool,
    /// Playback position as a percentage (0-100)
    pub percent: f64,
}

/// Represents the current playback progress of a video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackProgress {
    /// Current playback position in seconds
    pub position: f64,
    /// Total duration of the video in seconds
    pub duration: f64,
    /// Playback position as a percentage (0-100)
    pub percent: f64,
    /// Whether playback is currently paused
    pub is_paused: bool,
}

/// Manager for video instances with async support
pub struct VideoManager {
    instances: Arc<Mutex<HashMap<VideoId, VideoInstance>>>,
    event_subscribers: Arc<Mutex<Vec<EventSubscriber>>>,
    _event_task: Option<TokioJoinHandle<()>>,
}

impl VideoManager {
    /// Creates a new VideoManager
    pub fn new() -> Self {
        Self {
            instances: Arc::new(Mutex::new(HashMap::new())),
            event_subscribers: Arc::new(Mutex::new(Vec::new())),
            _event_task: None,
        }
    }
    
    /// Plays a video from a local file or URL
    pub async fn play(&self, source: String, options: PlaybackOptions) -> Result<VideoId> {
        let instances = Arc::clone(&self.instances);
        let event_subscribers = self.event_subscribers.clone();
        
        // Generate a new video ID
        let id = VideoId::new();
        
        // Spawn a blocking task to play the video
        tokio::task::spawn_blocking(move || {
            // Fix the spawn_options creation
            let spawn_options = SpawnOptions::from(&options);
            
            // Fix the mpv spawn
            let (mut process, socket_path) = spawn_mpv(&source, &spawn_options)?;
            
            // Fix the IpcConfig
            let ipc_config = if options.connection_timeout_ms.is_some() {
                IpcConfig::new(
                    options.connection_timeout_ms.unwrap(),
                    DEFAULT_IPC_POLL_INTERVAL_MS,  // Use default poll interval
                    true,  // auto_reconnect
                    DEFAULT_MAX_RECONNECT_ATTEMPTS,  // max_reconnect_attempts
                    DEFAULT_RECONNECT_DELAY_MS  // reconnect_delay_ms
                )
            } else {
                IpcConfig::default()
            };
            
            // Fix the MpvIpcClient connection
            let ipc_client = match MpvIpcClient::connect_with_config(&socket_path, ipc_config.clone()) {
                Ok(client) => client,
                Err(e) => {
                    // If we can't connect, make sure to clean up the process
                    debug!("Failed to connect to mpv IPC socket, killing process: {}", e);
                    let _ = process.kill();
                    return Err(e);
                }
            };
            
            let ipc_client = Arc::new(Mutex::new(ipc_client));
            
            // Fix the MpvIpcClient for events
            let event_ipc_client = match MpvIpcClient::connect_with_config(&socket_path, ipc_config) {
                Ok(client) => client,
                Err(e) => {
                    debug!("Failed to connect event listener to mpv IPC socket: {}", e);
                    // Still return success, but without event listening
                    let instance = VideoInstance {
                        id,
                        process,
                        ipc_client,
                        event_listener: None,
                        event_thread: None,
                        socket_path,
                    };
                    
                    let mut instances = instances.lock().unwrap();
                    instances.insert(id, instance);
                    
                    return Ok(id);
                }
            };
            
            // Fix the MpvEventListener creation
            let mut listener = MpvEventListener::new(event_ipc_client);
            
            // Start the listener
            if let Err(e) = listener.start_listening() {
                debug!("Failed to start event listener: {}", e);
                // Continue without event listening
                let instance = VideoInstance {
                    id,
                    process,
                    ipc_client,
                    event_listener: None,
                    event_thread: None,
                    socket_path,
                };
                
                let mut instances = instances.lock().unwrap();
                instances.insert(id, instance);
                
                return Ok(id);
            }
            
            // Set up event forwarding
            let video_id = id;
            let ipc_client_clone = Arc::clone(&ipc_client);
            let subscribers_clone = event_subscribers.clone();
            let interval = options.progress_interval_ms.unwrap_or(1000);
            
            // Start event thread
            let thread = thread::spawn(move || {
                Self::monitor_playback(video_id, ipc_client_clone, subscribers_clone, interval);
            });
            
            // Store the instance with listener and thread
            let instance = VideoInstance {
                id,
                process,
                ipc_client,
                event_listener: Some(listener),
                event_thread: Some(thread),
                socket_path,
            };
            
            let mut instances = instances.lock().unwrap();
            instances.insert(id, instance);
            
            Ok(id)
        }).await.unwrap()
    }
    
    /// Closes a specific video
    pub async fn close(&self, id: VideoId) -> Result<()> {
        let instances = self.instances.clone();
        let subscribers = self.event_subscribers.clone();
        
        // Spawn a blocking task to close the video
        match tokio::task::spawn_blocking(move || {
            debug!("Closing video with ID: {}", id.to_string());
            let mut instances = instances.lock().unwrap();
            
            if let Some(mut instance) = instances.remove(&id) {
                // First, mark the IPC client as intentionally closed
                // Do this before anything else to prevent reconnection attempts
                if let Some(mut client) = instance.ipc_client.lock().ok() {
                    debug!("Marking IPC client as intentionally closed for video {}", id.to_string());
                    client.mark_as_intentionally_closed();
                }
                
                // Then stop the event listener if it exists
                if let Some(mut listener) = instance.event_listener.take() {
                    debug!("Stopping event listener for video {}", id.to_string());
                    let _ = listener.stop_listening();
                    let _ = listener.handle_process_exit();
                }
                
                // Now try to send the quit command if still possible
                if let Some(mut client) = instance.ipc_client.lock().ok() {
                    if client.is_connected() {
                        debug!("Sending quit command to mpv for video {}", id.to_string());
                        let _ = client.quit();  // This also marks as intentionally closed
                    }
                    
                    // For extra safety, explicitly close the connection
                    client.close();
                }
                
                // Wait briefly for mpv to process the quit command
                use std::thread::sleep;
                use std::time::Duration;
                sleep(Duration::from_millis(100));
                
                // Kill the process if it's still running
                let _ = instance.process.kill();
                
                // Wait for any event thread to complete
                if let Some(thread) = instance.event_thread.take() {
                    debug!("Joining event thread for video {}", id.to_string());
                    let _ = thread.join();
                }
                
                // Notify subscribers that the video was closed
                Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                
                debug!("Video {} closed successfully", id.to_string());
            }
            
            Ok(())
        }).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to join task for closing video {}: {:?}", id.to_string(), e);
                Ok(()) // Convert JoinError to success since we just want to continue
            }
        }
    }
    
    /// Closes all videos
    pub async fn close_all(&self) -> Result<()> {
        let instances = self.instances.clone();
        let subscribers = self.event_subscribers.clone();
        
        // Spawn a blocking task to close all videos
        tokio::task::spawn_blocking(move || {
            let mut instances = instances.lock().unwrap();
            
            let ids: Vec<VideoId> = instances.keys().cloned().collect();
            for id in ids {
                if let Some(mut instance) = instances.remove(&id) {
                    debug!("Closing video with ID: {}", id.to_string());
                    
                    // First, mark the IPC client as intentionally closed
                    if let Some(mut client) = instance.ipc_client.lock().ok() {
                        debug!("Marking IPC client as intentionally closed for video {}", id.to_string());
                        client.mark_as_intentionally_closed();
                    }
                    
                    // Stop the event listener if it exists
                    if let Some(mut listener) = instance.event_listener.take() {
                        debug!("Stopping event listener for video {}", id.to_string());
                        let _ = listener.stop_listening();
                        let _ = listener.handle_process_exit();
                    }
                    
                    // Attempt to quit mpv gracefully
                    if let Ok(mut client) = instance.ipc_client.lock() {
                        debug!("Sending quit command to mpv for video {}", id.to_string());
                        let _ = client.quit();
                        
                        // For extra safety, explicitly close the connection
                        client.close();
                    }
                    
                    // Wait briefly for mpv to process the quit command
                    use std::thread::sleep;
                    use std::time::Duration;
                    sleep(Duration::from_millis(100));
                    
                    // Kill the process if it's still running
                    let _ = instance.process.kill();
                    
                    // Join the event thread if it exists
                    if let Some(thread) = instance.event_thread.take() {
                        debug!("Joining event thread for video {}", id.to_string());
                        let _ = thread.join();
                    }
                    
                    // Notify subscribers that the video was closed
                    Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                    
                    debug!("Video {} closed successfully", id.to_string());
                }
            }
            
            Ok(())
        }).await.unwrap()
    }
    
    /// Subscribes to video events
    pub async fn subscribe(&self) -> EventSubscription {
        let event_subscribers = self.event_subscribers.clone();
        let (sender, receiver) = mpsc::channel(100);
        let id = Uuid::new_v4();
        
        // Add the subscriber
        let subscriber = EventSubscriber {
            id,
            sender,
        };
        
        let mut subscribers = event_subscribers.lock().unwrap();
        subscribers.push(subscriber);
        
        EventSubscription {
            receiver,
            _id: id,
        }
    }
    
    /// Unsubscribes from video events
    pub async fn unsubscribe(&self, subscription_id: Uuid) {
        let event_subscribers = self.event_subscribers.clone();
        
        tokio::task::spawn_blocking(move || {
            let mut subscribers = event_subscribers.lock().unwrap();
            subscribers.retain(|s| s.id != subscription_id);
        }).await.unwrap();
    }
    
    /// Notifies subscribers of an event
    fn notify_subscribers(subscribers: &Arc<Mutex<Vec<EventSubscriber>>>, event: VideoEvent) {
        // Get event type and video ID based on the enum variant
        let (event_type, video_id) = match &event {
            VideoEvent::Progress { id, .. } => ("progress", id),
            VideoEvent::Started { id } => ("started", id),
            VideoEvent::Paused { id } => ("paused", id),
            VideoEvent::Resumed { id } => ("resumed", id),
            VideoEvent::Ended { id } => ("ended", id),
            VideoEvent::Closed { id } => ("closed", id),
            VideoEvent::Error { id, .. } => ("error", id),
        };

        // Check for "closed" or "ended" events to prevent duplicates using process-wide cache
        if event_type == "closed" || event_type == "ended" {
            let should_skip = {
                let mut events = NOTIFIED_EVENTS.lock().unwrap();
                let video_events = events.entry(video_id.0.to_string()).or_insert_with(HashSet::new);
                if video_events.contains(event_type) {
                    debug!("Skipping duplicate {} notification for video {:?} (process-wide deduplication)", 
                          event_type, video_id);
                    true
                } else {
                    video_events.insert(event_type.to_string());
                    debug!("Sending first {} notification for video {:?}", event_type, video_id);
                    false
                }
            };

            if should_skip {
                return;
            }
        }

        // Get the subscribers and notify them
        if let Ok(subscribers) = subscribers.lock() {
            // Notify all subscribers of the event
            for subscriber in subscribers.iter() {
                // Use try_send to avoid blocking
                if let Err(e) = subscriber.sender.try_send(event.clone()) {
                    debug!("Failed to notify subscriber: {}", e);
                }
            }
        }
    }
    
    /// Monitors playback and sends events to subscribers
    fn monitor_playback(
        id: VideoId,
        ipc_client: Arc<Mutex<MpvIpcClient>>,
        subscribers: Arc<Mutex<Vec<EventSubscriber>>>,
        interval_ms: u64,
    ) {
        use std::time::Duration;
        
        // Send started event
        Self::notify_subscribers(&subscribers, VideoEvent::Started { id });
        
        let interval = Duration::from_millis(interval_ms);
        let mut last_position = -1.0;
        let mut last_paused = false;
        let mut consecutive_errors = 0;
        let mut last_playback_status = String::new();  // Track previous playback status for changes
        let max_consecutive_errors = 3;  // Maximum number of consecutive errors before considering the player closed
        
        loop {
            // Sleep for the specified interval
            thread::sleep(interval);
            
            // First check if we are intentionally closed already
            let is_intentionally_closed = if let Ok(client) = ipc_client.lock() {
                client.is_intentionally_closed()
            } else {
                false
            };
            
            if is_intentionally_closed {
                debug!("IPC client for video {} is marked as intentionally closed, stopping monitoring", 
                       id.to_string());
                Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                break;
            }
            
            // Check if the ipc client is connected and socket exists
            // This is more reliable than just checking is_running
            let socket_exists = if let Ok(mut client) = ipc_client.lock() {
                match client.get_property("pid") {
                    Ok(_) => {
                        // Successfully communicated, reset error counter
                        consecutive_errors = 0;
                        true
                    },
                    Err(err) => {
                        debug!("Error checking mpv pid for video {}: {:?}", id.to_string(), err);
                        consecutive_errors += 1;
                        
                        // After multiple consecutive errors, assume the player is closed
                        if consecutive_errors >= max_consecutive_errors {
                            debug!("Reached max consecutive errors for video {}, assuming player closed", 
                                   id.to_string());
                            // Mark as intentionally closed to prevent further reconnection attempts
                            client.mark_as_intentionally_closed();
                            Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                            break;
                        }
                        
                        false
                    }
                }
            } else {
                false
            };
            
            if !socket_exists {
                consecutive_errors += 1;
                if consecutive_errors >= max_consecutive_errors {
                    debug!("Socket no longer exists for video {}, stopping monitoring", id.to_string());
                    if let Ok(mut client) = ipc_client.lock() {
                        client.mark_as_intentionally_closed();
                    }
                    Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                    break;
                }
                continue;
            }
            
            // Check current playback status - useful for detecting OSC-triggered actions
            let current_status = if let Ok(mut client) = ipc_client.lock() {
                match client.get_playback_status() {
                    Ok(status) => status,
                    Err(_) => String::new()
                }
            } else {
                String::new()
            };
            
            // If playback status changes to "idle", it might indicate
            // the user has closed the player via OSC
            if !current_status.is_empty() && current_status != last_playback_status {
                debug!("Playback status changed from '{}' to '{}' for video {}", 
                      last_playback_status, current_status, id.to_string());
                
                // Check for transitions that indicate OSC closure
                if current_status == "idle" {
                    debug!("Detected transition to idle state for video {}, likely OSC closure", id.to_string());
                    if let Ok(mut client) = ipc_client.lock() {
                        // Mark as intentionally closed to prevent reconnection attempts
                        client.mark_as_intentionally_closed();
                    }
                    Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                    break;
                }
                
                last_playback_status = current_status;
            }
            
            // Get current playback position
            let position = if let Ok(mut client) = ipc_client.lock() {
                if let Ok(value) = client.get_property("time-pos") {
                    value.as_f64()
                } else {
                    None
                }
            } else {
                None
            };
            
            let duration = if let Ok(mut client) = ipc_client.lock() {
                if let Ok(value) = client.get_property("duration") {
                    value.as_f64()
                } else {
                    None
                }
            } else {
                None
            };
            
            let paused = if let Ok(mut client) = ipc_client.lock() {
                if let Ok(value) = client.get_property("pause") {
                    value.as_bool().unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            };
            
            // Check if playback has ended
            let eof = if let Ok(mut client) = ipc_client.lock() {
                if let Ok(value) = client.get_property("eof-reached") {
                    value.as_bool().unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            };
            
            // Additionally check for idle-active which indicates mpv is waiting for commands
            let idle_active = if let Ok(mut client) = ipc_client.lock() {
                if let Ok(value) = client.get_property("idle-active") {
                    value.as_bool().unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            };
            
            // Send pause/resume events
            if paused != last_paused {
                if paused {
                    Self::notify_subscribers(&subscribers, VideoEvent::Paused { id });
                } else {
                    Self::notify_subscribers(&subscribers, VideoEvent::Resumed { id });
                }
                last_paused = paused;
            }
            
            // Send progress events
            if let (Some(position), Some(duration)) = (position, duration) {
                if position != last_position {
                    let percent = if duration > 0.0 {
                        (position / duration) * 100.0
                    } else {
                        0.0
                    };
                    
                    Self::notify_subscribers(&subscribers, VideoEvent::Progress {
                        id,
                        position,
                        duration,
                        percent,
                    });
                    
                    last_position = position;
                }
            }
            
            // Check if playback has ended
            if eof {
                debug!("EOF reached for video {}", id.to_string());
                if let Ok(mut client) = ipc_client.lock() {
                    // Mark as intentionally closed when EOF is reached
                    client.mark_as_intentionally_closed();
                }
                Self::notify_subscribers(&subscribers, VideoEvent::Ended { id });
                break;
            }
            
            // Check if the file has been closed
            if idle_active {
                debug!("Idle active detected for video {}", id.to_string());
                if let Ok(mut client) = ipc_client.lock() {
                    // Mark as intentionally closed when player becomes idle
                    client.mark_as_intentionally_closed();
                }
                Self::notify_subscribers(&subscribers, VideoEvent::Closed { id });
                break;
            }
        }
        
        debug!("Playback monitoring completed for video {}", id.to_string());
        
        // Ensure IPC client is marked as intentionally closed at the end
        if let Ok(mut client) = ipc_client.lock() {
            if !client.is_intentionally_closed() {
                debug!("Making sure IPC client is marked as intentionally closed at monitoring end");
                client.mark_as_intentionally_closed();
            }
        }
    }
    
    /// Updates window properties for a video instance
    pub async fn update_window(&self, id: VideoId, window: WindowOptions) -> Result<()> {
        let instances = self.instances.clone();
        
        tokio::task::spawn_blocking(move || {
            let instances = instances.lock().unwrap();
            
            if let Some(instance) = instances.get(&id) {
                let mut ipc_client = instance.ipc_client.lock().unwrap();
                
                // Apply window properties one by one
                if let Some((x, y)) = window.position {
                    let pos_value = serde_json::json!(format!("{}+{}", x, y));
                    ipc_client.set_property("window-pos", pos_value)?;
                }
                
                if let Some((width, height)) = window.size {
                    let size_value = serde_json::json!(format!("{}x{}", width, height));
                    ipc_client.set_property("geometry", size_value)?;
                }
                
                if window.always_on_top {
                    ipc_client.set_property("ontop", serde_json::json!(true))?;
                }
                
                if let Some(opacity) = window.opacity {
                    let opacity = opacity.max(0.0).min(1.0);
                    ipc_client.set_property("alpha", serde_json::json!(opacity))?;
                }
                
                if window.start_hidden {
                    ipc_client.set_property("window-minimized", serde_json::json!(true))?;
                }
                
                Ok(())
            } else {
                Err(crate::Error::MpvError(format!("Video instance not found: {}", id.to_string())))
            }
        }).await.unwrap()
    }
    
    /// Gets the current playback progress for a video
    pub async fn get_progress(&self, id: VideoId) -> Result<PlaybackProgress> {
        let instances = self.instances.lock().unwrap();
        
        if let Some(instance) = instances.get(&id) {
            let client = instance.ipc_client.clone();
            let mut client_guard = client.lock().unwrap();
            
            // Get position
            let position = match client_guard.get_property("playback-time") {
                Ok(pos) => pos.as_f64().unwrap_or(0.0),
                Err(_) => 0.0,
            };
            
            // Get duration
            let duration = match client_guard.get_property("duration") {
                Ok(dur) => dur.as_f64().unwrap_or(0.0),
                Err(_) => 0.0,
            };
            
            // Get paused state
            let is_paused = match client_guard.get_property("pause") {
                Ok(paused) => paused.as_bool().unwrap_or(false),
                Err(_) => false,
            };
            
            let percent = if duration > 0.0 {
                (position / duration) * 100.0
            } else {
                0.0
            };
            
            Ok(PlaybackProgress {
                position,
                duration,
                percent,
                is_paused,
            })
        } else {
            Err(Error::MpvError(format!("No video instance found with ID: {}", id.to_string())))
        }
    }
    
    /// Pauses video playback
    pub async fn pause(&self, id: VideoId) -> Result<()> {
        let instances = self.instances.lock().unwrap();
        
        if let Some(instance) = instances.get(&id) {
            let client = instance.ipc_client.clone();
            let mut client_guard = client.lock().unwrap();
            
            client_guard.set_pause(true)?;
            
            // Notify subscribers
            Self::notify_subscribers(
                &self.event_subscribers,
                VideoEvent::Paused { id }
            );
            
            Ok(())
        } else {
            Err(Error::MpvError(format!("Video instance not found: {}", id.to_string())))
        }
    }
    
    /// Resumes video playback
    pub async fn resume(&self, id: VideoId) -> Result<()> {
        let instances = self.instances.lock().unwrap();
        
        if let Some(instance) = instances.get(&id) {
            let client = instance.ipc_client.clone();
            let mut client_guard = client.lock().unwrap();
            
            client_guard.set_pause(false)?;
            
            // Notify subscribers
            Self::notify_subscribers(
                &self.event_subscribers,
                VideoEvent::Resumed { id }
            );
            
            Ok(())
        } else {
            Err(Error::MpvError(format!("Video instance not found: {}", id.to_string())))
        }
    }
    
    /// Seeks to a specific position in the video
    pub async fn seek(&self, id: VideoId, position: f64) -> Result<()> {
        let instances = self.instances.lock().unwrap();
        
        if let Some(instance) = instances.get(&id) {
            let client = instance.ipc_client.clone();
            let mut client_guard = client.lock().unwrap();
            
            client_guard.seek(position)?;
            
            Ok(())
        } else {
            Err(Error::MpvError(format!("Video instance not found: {}", id.to_string())))
        }
    }
    
    /// Sets the volume for a video
    pub async fn set_volume(&self, id: VideoId, volume: i32) -> Result<()> {
        let instances = self.instances.lock().unwrap();
        
        if let Some(instance) = instances.get(&id) {
            let client = instance.ipc_client.clone();
            let mut client_guard = client.lock().unwrap();
            
            client_guard.set_volume(volume as f64)?;
            
            Ok(())
        } else {
            Err(Error::MpvError(format!("Video instance not found: {}", id.to_string())))
        }
    }
    
    /// Gets detailed information about a video
    pub async fn get_video_info(&self, id: VideoId) -> Result<VideoInfo> {
        let instances = self.instances.lock().unwrap();
        
        if let Some(instance) = instances.get(&id) {
            let client = instance.ipc_client.clone();
            let mut client_guard = client.lock().unwrap();
            
            // Get position
            let position = match client_guard.get_property("playback-time") {
                Ok(pos) => pos.as_f64().unwrap_or(0.0),
                Err(_) => 0.0,
            };
            
            // Get duration
            let duration = match client_guard.get_property("duration") {
                Ok(dur) => dur.as_f64().unwrap_or(0.0),
                Err(_) => 0.0,
            };
            
            // Get volume
            let volume = match client_guard.get_property("volume") {
                Ok(vol) => vol.as_f64().unwrap_or(100.0),
                Err(_) => 100.0,
            };
            
            // Get paused state
            let is_paused = match client_guard.get_property("pause") {
                Ok(paused) => paused.as_bool().unwrap_or(false),
                Err(_) => false,
            };
            
            // Get speed
            let speed = match client_guard.get_property("speed") {
                Ok(spd) => spd.as_f64().unwrap_or(1.0),
                Err(_) => 1.0,
            };
            
            // Get muted state
            let is_muted = match client_guard.get_property("mute") {
                Ok(mute) => mute.as_bool().unwrap_or(false),
                Err(_) => false,
            };
            
            // Get path
            let path = match client_guard.get_property("path") {
                Ok(p) => p.as_str().unwrap_or("").to_string(),
                Err(_) => "".to_string(),
            };
            
            let percent = if duration > 0.0 {
                (position / duration) * 100.0
            } else {
                0.0
            };
            
            Ok(VideoInfo {
                path,
                position,
                duration,
                volume,
                is_paused,
                speed,
                is_muted,
                percent,
            })
        } else {
            Err(Error::MpvError(format!("Video instance not found: {}", id.to_string())))
        }
    }
}

impl Default for VideoManager {
    fn default() -> Self {
        Self::new()
    }
} 
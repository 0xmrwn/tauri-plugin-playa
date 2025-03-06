use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use serde_json::Value;
use log::{debug, error};

use crate::core::player::ipc::MpvIpcClient;
use crate::Error;
use crate::Result;

/// Types of events that can be emitted by mpv.
#[derive(Debug, Clone)]
pub enum MpvEvent {
    // Playback state events
    PlaybackStarted,
    PlaybackPaused,
    PlaybackResumed,
    PlaybackCompleted,
    
    // Progress events
    TimePositionChanged(f64),
    PercentPositionChanged(f64),
    
    // Player state events
    VolumeChanged(i32),
    MuteChanged(bool),
    
    // Error events
    PlaybackError(String),
    
    // Process events
    ProcessExited(i32),
    
    // Property change events
    PropertyChanged(String, Value),
    
    // Connection events
    ConnectionLost,
    ConnectionRestored,
}

/// Callback type for mpv events.
pub type EventCallback = Arc<dyn Fn(MpvEvent) + Send + Sync + 'static>;

/// Event listener for mpv events.
pub struct MpvEventListener {
    ipc_client: Arc<Mutex<MpvIpcClient>>,
    callbacks: Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    property_observers: Arc<Mutex<HashMap<String, u64>>>,
    running: Arc<Mutex<bool>>,
    poll_thread: Option<JoinHandle<()>>,
    connection_status: Arc<Mutex<bool>>,
    last_reconnect_attempt: Arc<Mutex<Option<Instant>>>,
}

impl MpvEventListener {
    /// Creates a new event listener.
    pub fn new(ipc_client: MpvIpcClient) -> Self {
        Self {
            ipc_client: Arc::new(Mutex::new(ipc_client)),
            callbacks: Arc::new(Mutex::new(HashMap::new())),
            property_observers: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
            poll_thread: None,
            connection_status: Arc::new(Mutex::new(true)), // Assume connected initially
            last_reconnect_attempt: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Subscribes to an event.
    pub fn subscribe<F>(&mut self, event_type: &str, callback: F) -> Result<()>
    where
        F: Fn(MpvEvent) + Send + Sync + 'static,
    {
        let event_callback = Arc::new(callback);
        
        if ["time-pos", "percent-pos", "pause", "mute", "volume", "eof-reached", "idle-active"]
            .contains(&event_type) {
            
            // Automatically observe the property if it's one of the standard properties
            self.observe_property(event_type)?;
        }
        
        let mut callbacks = self.callbacks.lock().unwrap();
        let event_callbacks = callbacks.entry(event_type.to_string()).or_insert_with(Vec::new);
        event_callbacks.push(event_callback);
        
        debug!("Subscribed to event: {}", event_type);
        Ok(())
    }
    
    /// Observes a property in mpv.
    fn observe_property(&mut self, property: &str) -> Result<()> {
        let mut property_observers = self.property_observers.lock().unwrap();
        
        // Check if we're already observing this property
        if property_observers.contains_key(property) {
            debug!("Already observing property: {}", property);
            return Ok(());
        }
        
        // Get a lock on the IPC client
        let mut ipc_client = self.ipc_client.lock().unwrap();
        
        // Register the property observer with mpv
        match ipc_client.observe_property(property) {
            Ok(observe_id) => {
                debug!("Started observing property: {} with ID: {}", property, observe_id);
                property_observers.insert(property.to_string(), observe_id);
                Ok(())
            },
            Err(e) => {
                error!("Failed to observe property {}: {}", property, e);
                Err(e)
            }
        }
    }
    
    /// Starts listening for events in a background thread.
    pub fn start_listening(&mut self) -> Result<()> {
        // Mark as running
        let mut running = self.running.lock().unwrap();
        if *running {
            debug!("Event listener is already running");
            return Ok(());
        }
        
        *running = true;
        drop(running);
        
        let ipc_client = Arc::clone(&self.ipc_client);
        let callbacks = Arc::clone(&self.callbacks);
        let property_observers = Arc::clone(&self.property_observers);
        let running = Arc::clone(&self.running);
        let connection_status = Arc::clone(&self.connection_status);
        let last_reconnect_attempt = Arc::clone(&self.last_reconnect_attempt);
        
        // Start a thread to poll for events
        let poll_thread = thread::spawn(move || {
            debug!("Starting event polling thread");
            
            while *running.lock().unwrap() {
                // Handle connection status
                let is_connected = {
                    let ipc_client = ipc_client.lock().unwrap();
                    ipc_client.is_connected()
                };
                
                {
                    let mut current_status = connection_status.lock().unwrap();
                    if *current_status != is_connected {
                        // Connection status changed
                        *current_status = is_connected;
                        
                        if is_connected {
                            // Connection restored, notify listeners
                            Self::notify_callbacks(&callbacks, "connection", &MpvEvent::ConnectionRestored);
                            debug!("Connection to mpv restored");
                            
                            // Re-observe all properties
                            Self::reobserve_properties(&ipc_client, &property_observers);
                        } else {
                            // Connection lost, notify listeners
                            Self::notify_callbacks(&callbacks, "connection", &MpvEvent::ConnectionLost);
                            debug!("Connection to mpv lost");
                        }
                    }
                }
                
                // Don't try to poll if not connected
                if !is_connected {
                    // Attempt reconnection
                    let should_attempt_reconnect = {
                        let mut last_attempt = last_reconnect_attempt.lock().unwrap();
                        if let Some(time) = *last_attempt {
                            if time.elapsed() > Duration::from_secs(5) {
                                *last_attempt = Some(Instant::now());
                                true
                            } else {
                                false
                            }
                        } else {
                            *last_attempt = Some(Instant::now());
                            true
                        }
                    };
                    
                    if should_attempt_reconnect {
                        debug!("Attempting to reconnect to mpv");
                        let mut ipc_client_lock = ipc_client.lock().unwrap();
                        // The reconnect will happen automatically on the next command if configured
                        let _ = ipc_client_lock.is_running();
                    }
                    
                    // Sleep a bit before trying again
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                
                // Poll for events if connected
                Self::poll_events(&ipc_client, &callbacks, &property_observers);
                
                // Use the configured poll interval instead of hardcoded value
                let poll_interval = {
                    let client = ipc_client.lock().unwrap();
                    client.get_poll_interval()
                };
                thread::sleep(Duration::from_millis(poll_interval));
            }
            
            debug!("Event polling thread stopped");
        });
        
        self.poll_thread = Some(poll_thread);
        debug!("Event listener started");
        
        Ok(())
    }
    
    /// Stops the event listener.
    pub fn stop_listening(&mut self) -> Result<()> {
        debug!("Stopping event listener");
        
        // Mark as not running first to stop the polling thread
        let mut running = self.running.lock().unwrap();
        if !*running {
            debug!("Event listener is not running");
            return Ok(());
        }
        
        *running = false;
        drop(running);
        
        // Mark the IPC client as intentionally closed first, before any unobserve attempts
        // This prevents the client from attempting to reconnect while we're shutting down
        {
            let mut ipc_client = match self.ipc_client.lock() {
                Ok(client) => client,
                Err(e) => {
                    error!("Failed to lock IPC client when stopping event listener: {:?}", e);
                    // Continue with cleanup even if we couldn't lock the client
                    return Ok(());
                }
            };
            
            // Mark as intentionally closed to prevent any reconnection attempts
            debug!("Marking IPC client as intentionally closed in stop_listening");
            ipc_client.mark_as_intentionally_closed();
        }
        
        // Wait for the poll thread to stop
        if let Some(thread) = self.poll_thread.take() {
            debug!("Waiting for event polling thread to stop");
            if let Err(e) = thread.join() {
                error!("Failed to join event polling thread: {:?}", e);
            }
        }
        
        // Now unobserve properties if the client is still connected
        // But only do this if we can acquire the locks without waiting
        if let Ok(mut ipc_client) = self.ipc_client.try_lock() {
            if let Ok(property_observers) = self.property_observers.try_lock() {
                for (property, observe_id) in property_observers.iter() {
                    debug!("Unobserving property: {} with ID: {}", property, observe_id);
                    
                    // Only attempt to unobserve if the client is still connected
                    if ipc_client.is_connected() {
                        if let Err(e) = ipc_client.unobserve_property(*observe_id) {
                            debug!("Failed to unobserve property {}: {} - likely already disconnected", property, e);
                        }
                    }
                }
            }
        }
        
        debug!("Event listener stopped");
        Ok(())
    }
    
    /// Re-observe all previously observed properties after reconnection
    fn reobserve_properties(
        ipc_client: &Arc<Mutex<MpvIpcClient>>,
        property_observers: &Arc<Mutex<HashMap<String, u64>>>,
    ) {
        let mut client = ipc_client.lock().unwrap();
        let mut observers = property_observers.lock().unwrap();
        
        // Create a list of properties to re-observe
        let properties: Vec<String> = observers.keys().cloned().collect();
        
        // Clear existing observers
        observers.clear();
        
        // Re-observe each property
        for property in properties {
            match client.observe_property(&property) {
                Ok(observe_id) => {
                    debug!("Re-observed property after reconnection: {} with ID: {}", property, observe_id);
                    observers.insert(property, observe_id);
                },
                Err(e) => {
                    error!("Failed to re-observe property {} after reconnection: {}", property, e);
                }
            }
        }
    }
    
    /// Polls for events from mpv.
    fn poll_events(
        ipc_client: &Arc<Mutex<MpvIpcClient>>,
        callbacks: &Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
        _property_observers: &Arc<Mutex<HashMap<String, u64>>>,
    ) {
        // Try to acquire the lock on the IPC client
        let mut ipc_client = match ipc_client.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                // Someone else is using the IPC client, skip this poll
                return;
            }
        };
        
        // If the client is intentionally closed, don't try to poll for events
        if ipc_client.is_intentionally_closed() {
            debug!("Skipping event polling for intentionally closed client");
            return;
        }
        
        // Track when the last position update was sent
        static mut LAST_POSITION_UPDATE: Option<Instant> = None;
        
        // Check if we need to update position (every 3 seconds)
        let should_update_position = unsafe {
            match LAST_POSITION_UPDATE {
                None => true,
                Some(last_time) => last_time.elapsed() >= Duration::from_secs(3)
            }
        };
        
        // Only update playback position occasionally
        if should_update_position {
            unsafe { LAST_POSITION_UPDATE = Some(Instant::now()) };
            Self::update_playback_properties(&mut ipc_client, callbacks);
        }
        
        // Always check for critical events
        Self::check_eof(&mut ipc_client, callbacks);
        Self::check_state_changes(&mut ipc_client, callbacks);
    }
    
    /// Updates playback properties like time-pos and percent-pos
    fn update_playback_properties(
        ipc_client: &mut MpvIpcClient,
        callbacks: &Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    ) {
        // Track the last reported positions to avoid sending too many updates
        static mut LAST_TIME_POS: Option<f64> = None;
        static mut LAST_PERCENT_POS: Option<f64> = None;
        
        // Get the current playback position
        if let Ok(time_pos) = ipc_client.get_time_pos() {
            // Only notify if position changed by at least 5 seconds
            let should_notify = unsafe {
                match LAST_TIME_POS {
                    None => true,
                    Some(last_pos) => (time_pos - last_pos).abs() >= 5.0
                }
            };
            
            if should_notify {
                unsafe { LAST_TIME_POS = Some(time_pos) };
                Self::notify_callbacks(callbacks, "time-pos", &MpvEvent::TimePositionChanged(time_pos));
            }
        }
        
        // Get the current percentage position
        if let Ok(percent_pos) = ipc_client.get_percent_pos() {
            // Only notify if position changed by at least 1%
            let should_notify = unsafe {
                match LAST_PERCENT_POS {
                    None => true,
                    Some(last_pos) => (percent_pos - last_pos).abs() >= 1.0
                }
            };
            
            if should_notify {
                unsafe { LAST_PERCENT_POS = Some(percent_pos) };
                Self::notify_callbacks(callbacks, "percent-pos", &MpvEvent::PercentPositionChanged(percent_pos));
            }
        }
    }
    
    /// Checks if playback has reached the end
    fn check_eof(
        ipc_client: &mut MpvIpcClient,
        callbacks: &Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    ) {
        // First, check if the ipc client is still connected and the process still running
        if !ipc_client.is_connected() {
            debug!("IPC client disconnected while checking EOF");
            
            // Check if it was an intentional close
            if ipc_client.is_intentionally_closed() {
                debug!("IPC client was intentionally closed, sending ProcessExited event");
                Self::notify_callbacks(callbacks, "process", &MpvEvent::ProcessExited(0));
            } else {
                debug!("IPC client disconnected unexpectedly, sending ConnectionLost event");
                Self::notify_callbacks(callbacks, "connection", &MpvEvent::ConnectionLost);
            }
            return;
        }
        
        // Check if we're at the end of playback via multiple signals
        
        // 1. Check direct EOF property
        let eof_reached = match ipc_client.get_property("eof-reached") {
            Ok(value) => {
                match value.as_bool() {
                    Some(true) => {
                        debug!("EOF reached directly reported by mpv property");
                        true
                    },
                    _ => false,
                }
            },
            Err(err) => {
                // If we get property unavailable error, mpv might be shutting down
                if let Error::MpvError(ref msg) = err {
                    if msg.contains("property unavailable") {
                        debug!("EOF property unavailable, mpv may be shutting down");
                        
                        // Mark as intentionally closed to avoid reconnection attempts
                        ipc_client.mark_as_intentionally_closed();
                        Self::notify_callbacks(callbacks, "process", &MpvEvent::ProcessExited(0));
                        return;
                    }
                }
                
                debug!("Error checking EOF: {:?}", err);
                false
            }
        };
        
        // 2. Check idle status - idle_active can indicate playback has ended
        let idle_active = match ipc_client.get_property("idle-active") {
            Ok(value) => value.as_bool().unwrap_or(false),
            Err(_) => false,
        };
        
        // 3. Check playback status - "idle" means no file is playing
        let playback_status = match ipc_client.get_playback_status() {
            Ok(status) => status,
            Err(_) => String::new(),
        };
        
        // If any of these indicators suggest EOF, notify about it
        if eof_reached || idle_active || playback_status == "idle" {
            debug!("EOF detected: eof_reached={}, idle_active={}, playback_status={}", 
                   eof_reached, idle_active, playback_status);
            Self::notify_callbacks(callbacks, "eof", &MpvEvent::PlaybackCompleted);
        }
    }
    
    /// Checks for state changes like pause, volume, etc.
    fn check_state_changes(
        ipc_client: &mut MpvIpcClient,
        callbacks: &Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    ) {
        // Check pause state
        if let Ok(paused) = ipc_client.get_pause() {
            static mut LAST_PAUSE_STATE: Option<bool> = None;
            
            let last_state = unsafe { LAST_PAUSE_STATE };
            
            if last_state != Some(paused) {
                if paused {
                    Self::notify_callbacks(callbacks, "pause", &MpvEvent::PlaybackPaused);
                } else if last_state.is_some() {
                    // Only notify resumed if we were previously paused
                    Self::notify_callbacks(callbacks, "pause", &MpvEvent::PlaybackResumed);
                } else {
                    // First check after starting, and we're not paused, so playback has started
                    Self::notify_callbacks(callbacks, "pause", &MpvEvent::PlaybackStarted);
                }
                
                unsafe { LAST_PAUSE_STATE = Some(paused); }
            }
        }
        
        // Volume and mute checks removed to reduce overhead
    }
    
    /// Notifies all registered callbacks for an event
    fn notify_callbacks(
        callbacks: &Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
        event_type: &str,
        event: &MpvEvent,
    ) {
        let callbacks_map = callbacks.lock().unwrap();
        
        // Call callbacks registered for this specific event type
        if let Some(event_callbacks) = callbacks_map.get(event_type) {
            for callback in event_callbacks {
                callback(event.clone());
            }
        }
        
        // Also call callbacks registered for all events
        if let Some(all_callbacks) = callbacks_map.get("all") {
            for callback in all_callbacks {
                callback(event.clone());
            }
        }
    }
    
    /// Checks if the event listener is running.
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
    
    /// Handles a process exit event.
    pub fn handle_process_exit(&mut self) -> Result<()> {
        debug!("Handling process exit in event listener");
        
        // Set running to false to stop event loop
        if let Ok(mut running) = self.running.lock() {
            *running = false;
        }
        
        // Mark the IPC client as intentionally closed to prevent reconnection attempts
        if let Ok(mut client) = self.ipc_client.lock() {
            debug!("Marking IPC client as intentionally closed due to process exit");
            client.mark_as_intentionally_closed();
            
            // Explicitly close the connection
            client.close();
        }
        
        // Clear all property observers to prevent further attempts to access them
        if let Ok(mut observers) = self.property_observers.lock() {
            observers.clear();
        }
        
        // Notify about process exit
        if let Ok(callbacks) = self.callbacks.lock() {
            Self::notify_callbacks(&Arc::new(Mutex::new(callbacks.clone())), "process", &MpvEvent::ProcessExited(0));
        }
        
        // Stop listening
        self.stop_listening()?;
        
        debug!("Process exit handling completed");
        Ok(())
    }
} 
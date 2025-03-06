use serde::{Deserialize, Serialize};

// Re-export relevant types from core
pub use crate::core::plugin::{VideoId, PlaybackOptions, VideoEvent, WindowOptions};

/// Request to play a video
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRequest {
    /// The path or URL of the video to play
    pub path: String,
    
    /// Optional playback options
    #[serde(default)]
    pub options: PlaybackOptions,
}

/// Response after successfully playing a video
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayResponse {
    /// The ID of the video being played
    pub video_id: String,
}

/// Request to control video playback
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlRequest {
    /// The ID of the video to control
    pub video_id: String,
    
    /// The command to execute (pause, resume, seek, etc.)
    pub command: String,
    
    /// Optional value for commands like seek that require additional data
    pub value: Option<f64>,
}

/// Response after controlling video playback
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlResponse {
    /// Whether the control command was successful
    pub success: bool,
    
    /// Current position in seconds
    pub position: Option<f64>,
    
    /// Current duration in seconds
    pub duration: Option<f64>,
    
    /// Current playback state (playing, paused, etc.)
    pub state: Option<String>,
}

/// Request to get video information
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoRequest {
    /// The ID of the video to get information for
    pub video_id: String,
}

/// Response with video information
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    /// The ID of the video
    pub video_id: String,
    
    /// The path or URL of the video
    pub path: String,
    
    /// Current position in seconds
    pub position: f64,
    
    /// Total duration in seconds
    pub duration: f64,
    
    /// Current volume (0-100)
    pub volume: i32,
    
    /// Whether the video is currently paused
    pub is_paused: bool,
    
    /// Current playback speed
    pub speed: f64,
    
    /// Whether the video is currently muted
    pub is_muted: bool,
}

/// Request to close a video
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseRequest {
    /// The ID of the video to close
    pub video_id: String,
}

/// Response after closing a video
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseResponse {
    /// Whether the video was successfully closed
    pub success: bool,
}

/// Request to list available presets
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPresetsRequest {}

/// Response with available presets
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPresetsResponse {
    /// List of available presets
    pub presets: Vec<String>,
    
    /// Recommended preset for current platform
    pub recommended: Option<String>,
}

use tauri::{plugin::PluginApi, AppHandle, Manager, Runtime};
use tauri::Emitter;
use tokio::sync::Mutex;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use log::{debug, error};

use crate::core::plugin::{VideoManager, VideoId};
use crate::models::{
    PlayRequest, PlayResponse, ControlRequest, ControlResponse, 
    InfoRequest, InfoResponse, CloseRequest, CloseResponse, 
    ListPresetsRequest, ListPresetsResponse
};
use crate::error::{Error, Result};

/// The state of the playa plugin
pub struct Playa<R: Runtime> {
    pub app_handle: AppHandle<R>,
    pub video_manager: Arc<Mutex<VideoManager>>,
    pub asset_path: PathBuf,
}

impl<R: Runtime> Playa<R> {
    /// Play a video
    pub async fn play(&self, request: PlayRequest) -> Result<PlayResponse> {
        let video_manager = self.video_manager.lock().await;
        
        let video_id = video_manager
            .play(request.path, request.options)
            .await
            .map_err(|e| Error::MpvError(format!("Failed to play video: {}", e)))?;
        
        Ok(PlayResponse {
            video_id: video_id.to_string(),
        })
    }
    
    /// Control video playback
    pub async fn control(&self, request: ControlRequest) -> Result<ControlResponse> {
        let video_manager = self.video_manager.clone();
        let video_id = VideoId::from_string(&request.video_id)
            .map_err(|e| Error::VideoIdError(format!("Invalid video ID: {}", e)))?;
        
        match request.command.as_str() {
            "pause" => {
                video_manager.lock().await.deref()
                    .pause(video_id)
                    .await
                    .map_err(|e| Error::MpvError(format!("Failed to pause video: {}", e)))?;
                
                Ok(ControlResponse {
                    success: true,
                    position: None,
                    duration: None,
                    state: Some("paused".to_string()),
                })
            }
            "resume" => {
                video_manager.lock().await.deref()
                    .resume(video_id)
                    .await
                    .map_err(|e| Error::MpvError(format!("Failed to resume video: {}", e)))?;
                
                Ok(ControlResponse {
                    success: true,
                    position: None,
                    duration: None,
                    state: Some("playing".to_string()),
                })
            }
            "seek" => {
                let position = request.value.ok_or_else(|| {
                    Error::PluginError("Seek command requires a position value".to_string())
                })?;
                
                video_manager.lock().await.deref()
                    .seek(video_id, position)
                    .await
                    .map_err(|e| Error::MpvError(format!("Failed to seek video: {}", e)))?;
                
                Ok(ControlResponse {
                    success: true,
                    position: Some(position),
                    duration: None,
                    state: None,
                })
            }
            "volume" => {
                let volume = request.value.ok_or_else(|| {
                    Error::PluginError("Volume command requires a volume value".to_string())
                })? as i32;
                
                video_manager.lock().await.deref()
                    .set_volume(video_id, volume)
                    .await
                    .map_err(|e| Error::MpvError(format!("Failed to set volume: {}", e)))?;
                
                Ok(ControlResponse {
                    success: true,
                    position: None,
                    duration: None,
                    state: None,
                })
            }
            _ => Err(Error::PluginError(format!(
                "Unsupported command: {}",
                request.command
            ))),
        }
    }
    
    /// Get video information
    pub async fn get_info(&self, request: InfoRequest) -> Result<InfoResponse> {
        let video_manager = self.video_manager.clone();
        let video_id = VideoId::from_string(&request.video_id)
            .map_err(|e| Error::VideoIdError(format!("Invalid video ID: {}", e)))?;
              
        let info = video_manager.lock().await.deref()
            .get_video_info(video_id)
            .await
            .map_err(|e| Error::MpvError(format!("Failed to get video info: {}", e)))?;
        
        Ok(InfoResponse {
            video_id: video_id.to_string(),
            path: info.path,
            position: info.position,
            duration: info.duration,
            volume: info.volume as i32,
            is_paused: info.is_paused,
            speed: info.speed,
            is_muted: info.is_muted,
        })
    }
    
    /// Close a video
    pub async fn close(&self, request: CloseRequest) -> Result<CloseResponse> {
        let video_manager = self.video_manager.lock().await;
        let video_id = VideoId::from_string(&request.video_id)
            .map_err(|e| Error::VideoIdError(format!("Invalid video ID: {}", e)))?;
        
        video_manager
            .close(video_id)
            .await
            .map_err(|e| Error::MpvError(format!("Failed to close video: {}", e)))?;
        
        Ok(CloseResponse { success: true })
    }
    
    /// List available presets
    pub fn list_presets(&self, _request: ListPresetsRequest) -> Result<ListPresetsResponse> {
        // Get the list of available presets
        let presets = crate::core::presets::list_available_presets();
        
        // Get the recommended preset
        let recommended = crate::core::presets::get_recommended_preset();
        
        Ok(ListPresetsResponse {
            presets,
            recommended: Some(recommended),
        })
    }
    
    /// Get the path to the mpv_config directory
    pub fn get_assets_path(&self) -> PathBuf {
        self.asset_path.clone()
    }
}

/// Initialize the playa plugin
pub fn init<R: Runtime>(app: &AppHandle<R>, _api: PluginApi<R, ()>) -> Result<Playa<R>> {
    // Set up logging
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .try_init()
        .ok();
    
    // Create a new tokio runtime for the plugin
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| Error::PluginError(format!("Failed to create Tokio runtime: {}", e)))?;
    
    // Create a video manager
    let video_manager = rt.block_on(async {
        VideoManager::new()
    });
    
    // Get the current plugin assets path
    let asset_path = app
        .path()
        .app_config_dir()
        .map_err(|e| Error::PluginError(format!("Failed to get app config directory: {}", e)))?
        .join("plugins")
        .join("playa")
        .join("mpv_config");
    
    let playa = Playa {
        app_handle: app.clone(),
        video_manager: Arc::new(Mutex::new(video_manager)),
        asset_path,
    };
    
    // Set up event subscription to forward events to the frontend
    setup_event_subscription(app, Arc::clone(&playa.video_manager))?;
    
    Ok(playa)
}

/// Set up event subscription to forward events to the frontend
fn setup_event_subscription<R: Runtime>(
    app: &AppHandle<R>,
    video_manager: Arc<Mutex<VideoManager>>,
) -> Result<()> {
    let app_handle = app.clone();
    
    // Spawn a task to handle video events
    tokio::spawn(async move {
        let mut subscription = video_manager.lock().await.deref().subscribe().await;
        
        while let Some(event) = subscription.recv().await {
            debug!("Received video event: {:?}", event);
            
            // Convert the VideoEvent to JSON and emit it to the frontend
            let event_name = "playa://event";
            
            // Use the Manager trait's emit method
            if let Err(e) = app_handle.emit(event_name, event) {
                error!("Failed to emit video event: {}", e);
            }
        }
    });
    
    Ok(())
} 
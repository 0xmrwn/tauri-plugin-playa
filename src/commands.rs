use tauri::{AppHandle, command, Runtime};

use crate::models::*;
use crate::Result;
use crate::PlayaExt;

/// Play a video file or URL
#[command]
pub(crate) async fn play<R: Runtime>(
    app: AppHandle<R>,
    request: PlayRequest,
) -> Result<PlayResponse> {
    app.playa().play(request).await
}

/// Control video playback (pause, resume, seek, etc.)
#[command]
pub(crate) async fn control<R: Runtime>(
    app: AppHandle<R>,
    request: ControlRequest,
) -> Result<ControlResponse> {
    app.playa().control(request).await
}

/// Get information about a playing video
#[command]
pub(crate) async fn get_info<R: Runtime>(
    app: AppHandle<R>,
    request: InfoRequest,
) -> Result<InfoResponse> {
    app.playa().get_info(request).await
}

/// Close a video
#[command]
pub(crate) async fn close<R: Runtime>(
    app: AppHandle<R>,
    request: CloseRequest,
) -> Result<CloseResponse> {
    app.playa().close(request).await
}

/// List available presets
#[command]
pub(crate) fn list_presets<R: Runtime>(
    app: AppHandle<R>,
    request: ListPresetsRequest,
) -> Result<ListPresetsResponse> {
    app.playa().list_presets(request)
}

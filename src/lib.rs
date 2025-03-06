use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime, RunEvent,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;
pub mod core;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Playa;
#[cfg(mobile)]
use mobile::Playa;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the playa APIs.
pub trait PlayaExt<R: Runtime> {
  fn playa(&self) -> &Playa<R>;
}

impl<R: Runtime, T: Manager<R>> crate::PlayaExt<R> for T {
  fn playa(&self) -> &Playa<R> {
    self.state::<Playa<R>>().inner()
  }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
  Builder::new("playa")
    .invoke_handler(tauri::generate_handler![
      commands::play,
      commands::control,
      commands::get_info,
      commands::close,
      commands::list_presets
    ])
    .setup(|app, api| {
      #[cfg(mobile)]
      let playa = mobile::init(app, api)?;
      #[cfg(desktop)]
      let playa = desktop::init(app, api)?;
      app.manage(playa);
      Ok(())
    })
    .on_event(|app, event| {
      match event {
        RunEvent::Exit => {
          // Clean up resources when the application exits
          if let Some(playa) = app.try_state::<Playa<R>>() {
            // Using try_state to avoid panicking if the state is not available
            log::info!("Cleaning up playa plugin resources");
            
            // We can perform any necessary cleanup here
            // For example, ensure all videos are closed
            let video_manager = playa.inner().video_manager.clone();
            
            // Use a blocking runtime to ensure cleanup completes before app exit
            if let Ok(rt) = tokio::runtime::Runtime::new() {
              rt.block_on(async {
                if let Ok(manager) = video_manager.try_lock() {
                  // Close all videos
                  if let Err(e) = manager.close_all().await {
                    log::error!("Error closing videos during shutdown: {}", e);
                  }
                }
              });
            }
          }
        }
        _ => {}
      }
    })
    .build()
}

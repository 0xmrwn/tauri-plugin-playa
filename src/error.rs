// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use std::io;

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    #[error("IO error: {0}")]
    Io(String),

    #[error("MPV error: {0}")]
    MpvError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("JSON error: {0}")]
    JsonError(String),
    
    #[error("Video ID error: {0}")]
    VideoIdError(String),
    
    #[error("Plugin error: {0}")]
    PluginError(String),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::JsonError(error.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        Self::PluginError(error.to_string())
    }
}

// Define a separate Error type for core::plugin that maps to our main Error
pub mod plugin_error {
    use super::Error;
    
    #[derive(Debug, thiserror::Error)]
    pub enum PluginError {
        #[error("IO error: {0}")]
        Io(std::io::Error),

        #[error("MPV error: {0}")]
        MpvError(String),

        #[error("Configuration error: {0}")]
        ConfigError(String),

        #[error("JSON error: {0}")]
        JsonError(String),

        #[error("Video ID error: {0}")]
        VideoIdError(String),
    }

    impl From<PluginError> for Error {
        fn from(error: PluginError) -> Self {
            match error {
                PluginError::Io(e) => Self::Io(e.to_string()),
                PluginError::MpvError(e) => Self::MpvError(e),
                PluginError::ConfigError(e) => Self::ConfigError(e),
                PluginError::JsonError(e) => Self::JsonError(e),
                PluginError::VideoIdError(e) => Self::VideoIdError(e),
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

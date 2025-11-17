use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = std::result::Result<T, AppError>;
pub type CommandResult<T> = std::result::Result<T, CommandError>;

#[derive(Debug, Error, Clone)]
pub enum AppError {
    #[error("Recorder is already running")]
    RecorderBusy,
    #[error("Recorder is not running")]
    RecorderNotRunning,
    #[error("Audio stream unavailable")]
    AudioStreamUnavailable,
    #[error("Audio input device error: {0}")]
    AudioDevice(String),
    #[error("Failed to initialize audio capture: {0}")]
    AudioInit(String),
    #[error("No audio samples captured")]
    AudioEmpty,
    #[error("Realtime service error: {0}")]
    Realtime(String),
    #[error("Missing OpenAI API key")]
    MissingApiKey,
    #[error("{0}")]
    Validation(String),
    #[error("Paste simulation failed: {0}")]
    Paste(String),
    #[error("History storage error: {0}")]
    History(String),
    #[error("Settings error: {0}")]
    Settings(String),
    #[error("Hotkey error: {0}")]
    Hotkey(String),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::RecorderBusy => "RECORDER_BUSY",
            AppError::RecorderNotRunning => "RECORDER_NOT_RUNNING",
            AppError::AudioStreamUnavailable => "AUDIO_STREAM_UNAVAILABLE",
            AppError::AudioDevice(_) => "AUDIO_DEVICE",
            AppError::AudioInit(_) => "AUDIO_INIT",
            AppError::AudioEmpty => "AUDIO_EMPTY",
            AppError::Realtime(_) => "REALTIME",
            AppError::MissingApiKey => "MISSING_API_KEY",
            AppError::Validation(_) => "VALIDATION",
            AppError::Paste(_) => "PASTE",
            AppError::History(_) => "HISTORY",
            AppError::Settings(_) => "SETTINGS",
            AppError::Hotkey(_) => "HOTKEY",
            AppError::Internal(_) => "INTERNAL",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<AppError> for CommandError {
    fn from(value: AppError) -> Self {
        Self::new(value.code(), value.to_string())
    }
}

impl From<tauri_plugin_global_shortcut::Error> for AppError {
    fn from(value: tauri_plugin_global_shortcut::Error) -> Self {
        AppError::Hotkey(value.to_string())
    }
}

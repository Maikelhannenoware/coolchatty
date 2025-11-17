use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::errors::{AppError, AppResult};

pub const DEFAULT_REALTIME_MODEL: &str = "gpt-realtime-mini";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub api_key: String,
    pub model: String,
    pub hotkey: String,
    pub auto_paste: bool,
    pub save_history: bool,
    pub sample_rate: u32,
    pub input_device: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: DEFAULT_REALTIME_MODEL.into(),
            hotkey: "Alt+Space".into(),
            auto_paste: true,
            save_history: true,
            sample_rate: 16_000,
            input_device: None,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    inner: RwLock<AppSettings>,
}

impl SettingsStore {
    pub fn load() -> AppResult<Self> {
        let path = settings_path()?;
        let data = if path.exists() {
            let raw =
                fs::read_to_string(&path).map_err(|err| AppError::Settings(err.to_string()))?;
            serde_json::from_str::<AppSettings>(&raw)
                .map_err(|err| AppError::Settings(err.to_string()))?
                .normalized()
        } else {
            let defaults = AppSettings::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| AppError::Settings(err.to_string()))?;
            }
            let body = serde_json::to_string_pretty(&defaults)
                .map_err(|err| AppError::Settings(err.to_string()))?;
            fs::write(&path, body).map_err(|err| AppError::Settings(err.to_string()))?;
            defaults
        };
        Ok(Self {
            path,
            inner: RwLock::new(data),
        })
    }

    pub async fn get(&self) -> AppSettings {
        self.inner.read().await.clone()
    }

    pub async fn update(&self, new_settings: AppSettings) -> AppResult<()> {
        let next = new_settings.normalized();
        {
            let mut guard = self.inner.write().await;
            *guard = next.clone();
        }
        let payload =
            serde_json::to_vec_pretty(&next).map_err(|err| AppError::Settings(err.to_string()))?;
        tokio::fs::write(&self.path, payload)
            .await
            .map_err(|err| AppError::Settings(err.to_string()))?;
        Ok(())
    }
}

fn settings_path() -> AppResult<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "coolchatty", "CoolChatty")
        .ok_or_else(|| AppError::Settings("unable to determine configuration directory".into()))?;
    let dir = proj_dirs.config_dir();
    Ok(dir.join("settings.json"))
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        if self.model.trim().is_empty() {
            self.model = DEFAULT_REALTIME_MODEL.into();
        }
        self
    }
}

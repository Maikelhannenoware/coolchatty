use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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
    pub fn load() -> anyhow::Result<Self> {
        let path = settings_path()?;
        let data = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str::<AppSettings>(&raw)?.normalized()
        } else {
            let defaults = AppSettings::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, serde_json::to_string_pretty(&defaults)?)?;
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

    pub async fn update(&self, new_settings: AppSettings) -> anyhow::Result<()> {
        let next = new_settings.normalized();
        {
            let mut guard = self.inner.write().await;
            *guard = next.clone();
        }
        tokio::fs::write(&self.path, serde_json::to_vec_pretty(&next)?).await?;
        Ok(())
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

fn settings_path() -> anyhow::Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "coolchatty", "CoolChatty")
        .ok_or_else(|| anyhow::anyhow!("unable to determine configuration directory"))?;
    let dir = proj_dirs.config_dir();
    Ok(dir.join("settings.json"))
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        if self.model.trim().is_empty() || self.model != DEFAULT_REALTIME_MODEL {
            self.model = DEFAULT_REALTIME_MODEL.into();
        }
        self
    }
}

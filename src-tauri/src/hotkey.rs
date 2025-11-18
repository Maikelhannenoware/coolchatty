use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing::{debug, warn};

use crate::errors::{AppError, AppResult};

const EVENT_NAME: &str = "trigger_record";

pub struct HotkeyManager {
    binding: Mutex<String>,
}

impl HotkeyManager {
    pub fn new(app: &AppHandle, default: &str) -> AppResult<Self> {
        let manager = Self {
            binding: Mutex::new(String::new()),
        };
        manager.register_binding(app, default)?;
        Ok(manager)
    }

    pub fn binding(&self) -> String {
        self.binding.lock().clone()
    }

    pub fn update(&self, app: &AppHandle, binding: String) -> AppResult<()> {
        self.register_binding(app, &binding)
    }

    pub fn emit_trigger(&self, app: &AppHandle) {
        let payload = self.binding();
        let _ = app.emit(EVENT_NAME, payload);
    }

    fn register_binding(&self, app: &AppHandle, binding: &str) -> AppResult<()> {
        let normalized = normalize_binding(binding.trim());
        let shortcut_manager = app.global_shortcut();
        shortcut_manager.unregister_all().map_err(AppError::from)?;

        if normalized.is_empty() {
            warn!("Hotkey cleared; no global shortcut registered");
            *self.binding.lock() = String::new();
            return Ok(());
        }

        let binding_owned = normalized;
        let payload_binding = binding_owned.clone();
        let event_name = EVENT_NAME.to_string();
        shortcut_manager
            .on_shortcut(normalized, move |app, _, event| {
                if matches!(event.state, ShortcutState::Pressed) {
                    let _ = app.emit(&event_name, payload_binding.clone());
                }
            })
            .map_err(AppError::from)?;

        *self.binding.lock() = binding_owned.clone();
        debug!(shortcut = %binding_owned, "registered global hotkey");
        Ok(())
    }
}

fn normalize_binding(binding: &str) -> String {
    if binding.is_empty() {
        return String::new();
    }
    #[cfg(target_os = "macos")]
    {
        let mut result = binding.to_string();
        if result.contains("Alt") {
            result = result.replace("Alt", "Option");
        }
        if result.contains("Cmd") {
            result = result.replace("Cmd", "Command");
        }
        result
    }
    #[cfg(not(target_os = "macos"))]
    {
        binding.to_string()
    }
}

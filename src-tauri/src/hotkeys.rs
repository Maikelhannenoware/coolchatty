use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

const EVENT_NAME: &str = "trigger_record";

pub struct HotkeyManager {
    binding: Mutex<String>,
}

impl HotkeyManager {
    pub fn new(default: &str) -> Self {
        Self {
            binding: Mutex::new(default.to_string()),
        }
    }

    pub fn binding(&self) -> String {
        self.binding.lock().clone()
    }

    pub fn update(&self, binding: String) {
        *self.binding.lock() = binding;
    }

    pub fn emit_trigger(&self, app: &AppHandle) {
        let payload = self.binding();
        let _ = app.emit(EVENT_NAME, payload);
    }

    pub const fn event_name() -> &'static str {
        EVENT_NAME
    }
}

use std::sync::Arc;

use crate::audio::RecorderService;
use crate::history::HistoryStore;
use crate::hotkeys::HotkeyManager;
use crate::paste::PasteManager;
use crate::settings::SettingsStore;

pub struct AppState {
    pub recorder: Arc<RecorderService>,
    pub history: Arc<HistoryStore>,
    pub paste: Arc<PasteManager>,
    pub hotkeys: Arc<HotkeyManager>,
    pub settings: Arc<SettingsStore>,
}

impl AppState {
    pub fn new(
        recorder: RecorderService,
        history: HistoryStore,
        paste: PasteManager,
        hotkeys: HotkeyManager,
        settings: SettingsStore,
    ) -> Self {
        Self {
            recorder: Arc::new(recorder),
            history: Arc::new(history),
            paste: Arc::new(paste),
            hotkeys: Arc::new(hotkeys),
            settings: Arc::new(settings),
        }
    }
}

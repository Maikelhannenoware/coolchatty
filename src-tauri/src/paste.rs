use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::errors::{AppError, AppResult};

#[derive(Clone, Copy, Debug)]
pub enum PasteOutcome {
    ClipboardOnly,
    SimulatedPaste,
}

#[derive(Default)]
pub struct PasteManager;

impl PasteManager {
    pub fn new() -> Self {
        Self
    }

    pub fn apply(&self, text: &str, auto_paste: bool) -> AppResult<PasteOutcome> {
        let mut clipboard = Clipboard::new()
            .map_err(|err| AppError::Paste(format!("clipboard unavailable: {err}")))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| AppError::Paste(err.to_string()))?;
        if auto_paste {
            simulate_paste()?;
            Ok(PasteOutcome::SimulatedPaste)
        } else {
            Ok(PasteOutcome::ClipboardOnly)
        }
    }
}

fn simulate_paste() -> AppResult<()> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|err| {
        AppError::Paste(format!("failed to initialize keyboard automation: {err}"))
    })?;
    #[cfg(target_os = "macos")]
    {
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|err| AppError::Paste(err.to_string()))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|err| AppError::Paste(err.to_string()))?;
        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|err| AppError::Paste(err.to_string()))?;
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|err| AppError::Paste(err.to_string()))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|err| AppError::Paste(err.to_string()))?;
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|err| AppError::Paste(err.to_string()))?;
    }
    Ok(())
}

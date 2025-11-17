use anyhow::{anyhow, Result};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

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

    pub fn apply(&self, text: &str, auto_paste: bool) -> Result<PasteOutcome> {
        let mut clipboard = Clipboard::new().map_err(|e| anyhow!("clipboard unavailable: {e}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| anyhow!("failed to update clipboard: {e}"))?;
        if auto_paste {
            simulate_paste()?;
            Ok(PasteOutcome::SimulatedPaste)
        } else {
            Ok(PasteOutcome::ClipboardOnly)
        }
    }
}

fn simulate_paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| anyhow!("failed to initialize keyboard automation: {e}"))?;
    #[cfg(target_os = "macos")]
    {
        enigo.key(Key::Meta, Direction::Press)?;
        enigo.key(Key::Unicode('v'), Direction::Click)?;
        enigo.key(Key::Meta, Direction::Release)?;
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        enigo.key(Key::Control, Direction::Press)?;
        enigo.key(Key::Unicode('v'), Direction::Click)?;
        enigo.key(Key::Control, Direction::Release)?;
    }
    Ok(())
}

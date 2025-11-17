use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::audio::RecorderRequest;
use crate::history::HistoryEntry;
use crate::paste::PasteOutcome;
use crate::settings::{AppSettings, DEFAULT_REALTIME_MODEL};
use crate::state::AppState;
use crate::websocket;

#[derive(Debug, Serialize)]
pub struct RecordingSummary {
    pub text: String,
    pub pasted: bool,
    pub duration_ms: i64,
}

#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.get().await;
    if settings.api_key.trim().is_empty() {
        return Err("Please provide an OpenAI API key in settings.".into());
    }

    state
        .recorder
        .start(RecorderRequest {
            sample_rate: settings.sample_rate,
            input_device: settings.input_device.clone(),
        })
        .map_err(|e| e.to_string())?;

    let audio_rx = state
        .recorder
        .take_receiver()
        .ok_or_else(|| "audio stream unavailable".to_string())?;

    let api_key = settings.api_key.clone();
    let model = settings.model.clone();
    let sample_rate = settings.sample_rate;
    let handle: tokio::task::JoinHandle<anyhow::Result<String>> = tokio::spawn(async move {
        websocket::stream_transcription(api_key, model, sample_rate, audio_rx).await
    });

    state
        .recorder
        .attach_session(handle)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<RecordingSummary, String> {
    let duration: Duration = state
        .recorder
        .stop()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "recorder is not running".to_string())?;

    let handle = state
        .recorder
        .take_session()
        .ok_or_else(|| "no active session".to_string())?;
    let mut settings = state.settings.get().await;
    let transcript = match handle.await.map_err(|e| e.to_string())? {
        Ok(text) => text,
        Err(err) => {
            let err_string = err.to_string();
            if is_model_error(&err_string) && settings.model != DEFAULT_REALTIME_MODEL {
                settings.model = DEFAULT_REALTIME_MODEL.into();
                state
                    .settings
                    .update(settings.clone())
                    .await
                    .map_err(|e| e.to_string())?;
                return Err(format!(
                    "{err_string}. Model reset to GPT Realtime mini. Please try again."
                ));
            }
            return Err(err_string);
        }
    };

    let pasted = if transcript.trim().is_empty() {
        false
    } else {
        matches!(
            state
                .paste
                .apply(&transcript, settings.auto_paste)
                .map_err(|e| e.to_string())?,
            PasteOutcome::SimulatedPaste
        )
    };

    if settings.save_history && !transcript.trim().is_empty() {
        state
            .history
            .add(&transcript)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(RecordingSummary {
        text: transcript,
        pasted,
        duration_ms: duration.as_millis() as i64,
    })
}

#[tauri::command]
pub async fn recorder_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.recorder.is_recording())
}

#[tauri::command]
pub async fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    state.history.all().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history.clear().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trigger_record_event(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.hotkeys.emit_trigger(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings.get().await)
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    state
        .settings
        .update(settings.clone())
        .await
        .map_err(|e| e.to_string())?;
    state.hotkeys.update(settings.hotkey.clone());
    Ok(())
}

fn is_model_error(message: &str) -> bool {
    message.contains("not supported") || message.contains("model")
}

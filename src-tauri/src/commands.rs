use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};
use tracing::info;

use crate::audio::RecorderRequest;
use crate::errors::{AppError, CommandError, CommandResult};
use crate::history::HistoryEntry;
use crate::paste::PasteOutcome;
use crate::realtime;
use crate::settings::{AppSettings, DEFAULT_REALTIME_MODEL};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct RecordingSummary {
    pub text: String,
    pub pasted: bool,
    pub duration_ms: i64,
}

#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>) -> CommandResult<()> {
    let settings = state.settings.get().await;
    if settings.api_key.trim().is_empty() {
        return Err(AppError::MissingApiKey.into());
    }

    let sample_rate = state
        .recorder
        .start(RecorderRequest {
            sample_rate: settings.sample_rate,
            input_device: settings.input_device.clone(),
        })
        .map_err(CommandError::from)?;

    let audio_rx = state
        .recorder
        .take_receiver()
        .ok_or(AppError::AudioStreamUnavailable)
        .map_err(CommandError::from)?;

    let api_key = settings.api_key.clone();
    let model = settings.model.clone();
    let handle = tokio::spawn(async move {
        realtime::stream_transcription(api_key, model, sample_rate, audio_rx).await
    });

    state
        .recorder
        .attach_session(handle)
        .map_err(CommandError::from)?;

    Ok(())
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> CommandResult<RecordingSummary> {
    let duration: Duration = state
        .recorder
        .stop()
        .await
        .map_err(CommandError::from)?
        .ok_or(AppError::RecorderNotRunning)
        .map_err(CommandError::from)?;

    let handle = state
        .recorder
        .take_session()
        .ok_or(AppError::RecorderNotRunning)
        .map_err(CommandError::from)?;
    let mut settings = state.settings.get().await;

    let transcript = match handle.await {
        Ok(Ok(text)) => text,
        Ok(Err(err)) => {
            let err_message = err.to_string();
            if is_model_error(&err_message) && settings.model != DEFAULT_REALTIME_MODEL {
                settings.model = DEFAULT_REALTIME_MODEL.into();
                state
                    .settings
                    .update(settings.clone())
                    .await
                    .map_err(CommandError::from)?;
                return Err(AppError::Validation(format!(
                    "{err_message}. Model reset to GPT Realtime mini. Please try again."
                ))
                .into());
            }
            return Err(err.into());
        }
        Err(err) => return Err(AppError::Internal(err.to_string()).into()),
    };

    let pasted = if transcript.trim().is_empty() {
        false
    } else {
        matches!(
            state
                .paste
                .apply(&transcript, settings.auto_paste)
                .map_err(CommandError::from)?,
            PasteOutcome::SimulatedPaste
        )
    };

    if settings.save_history && !transcript.trim().is_empty() {
        state
            .history
            .add(&transcript)
            .await
            .map_err(CommandError::from)?;
    }

    info!(
        "Recording finished (duration={} ms, pasted={})",
        duration.as_millis(),
        pasted
    );

    Ok(RecordingSummary {
        text: transcript,
        pasted,
        duration_ms: duration.as_millis() as i64,
    })
}

#[tauri::command]
pub async fn recorder_status(state: State<'_, AppState>) -> CommandResult<bool> {
    Ok(state.recorder.is_recording())
}

#[tauri::command]
pub async fn get_history(state: State<'_, AppState>) -> CommandResult<Vec<HistoryEntry>> {
    state.history.all().await.map_err(CommandError::from)
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> CommandResult<()> {
    state.history.clear().await.map_err(CommandError::from)
}

#[tauri::command]
pub async fn trigger_record_event(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    state.hotkeys.emit_trigger(&app);
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    Ok(state.settings.get().await)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResult<()> {
    state
        .settings
        .update(settings.clone())
        .await
        .map_err(CommandError::from)?;
    state
        .hotkeys
        .update(&app, settings.hotkey.clone())
        .map_err(CommandError::from)?;
    Ok(())
}

fn is_model_error(message: &str) -> bool {
    message.contains("not supported") || message.contains("model")
}

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tauri::http::{HeaderValue, Request};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use tracing::{debug, info, warn};

use crate::errors::{AppError, AppResult};

const MAX_CONNECT_ATTEMPTS: usize = 4;

pub async fn stream_transcription(
    api_key: String,
    model: String,
    sample_rate: u32,
    mut audio_rx: mpsc::Receiver<Vec<i16>>,
) -> AppResult<String> {
    let request = build_request(&api_key, &model)?;
    let mut backoff = Duration::from_millis(400);
    let mut attempt = 0usize;
    let ws = loop {
        attempt += 1;
        match connect_async(request.clone()).await {
            Ok((socket, _)) => break socket,
            Err(err) if attempt < MAX_CONNECT_ATTEMPTS => {
                warn!(attempt, error = %err, "websocket connect failed, retrying");
                sleep(backoff).await;
                backoff *= 2;
            }
            Err(err) => return Err(AppError::Realtime(err.to_string())),
        }
    };

    let (mut write, mut read) = ws.split();
    let mut total_samples: usize = 0;
    let mut chunk_counter = 0usize;

    while let Some(chunk) = audio_rx.recv().await {
        if chunk.is_empty() {
            continue;
        }
        total_samples += chunk.len();
        chunk_counter += 1;
        let payload = serde_json::json!({
            "type": "input_audio_buffer.append",
            "audio": encode_samples(&chunk),
        });
        write
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|err| AppError::Realtime(err.to_string()))?;
        let ms = (total_samples as f32 / sample_rate as f32) * 1000.0;
        debug!(
            chunk = chunk_counter,
            samples = chunk.len(),
            approx_ms = ms,
            "appended audio chunk"
        );
    }

    if total_samples == 0 {
        return Err(AppError::AudioEmpty);
    }

    let total_ms = (total_samples as f32 / sample_rate as f32) * 1000.0;
    if total_ms < 200.0 {
        return Err(AppError::Validation(format!(
            "Recording too short (only {total_ms:.1} ms). Please speak a bit longer."
        )));
    }

    write
        .send(Message::Text(
            serde_json::json!({"type": "input_audio_buffer.commit"})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|err| AppError::Realtime(err.to_string()))?;
    write
        .send(Message::Text(
            serde_json::json!({
                "type": "response.create",
                "response": {
                    "modalities": ["text"],
                    "instructions": "Transcribe the latest audio sample"
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|err| AppError::Realtime(err.to_string()))?;

    let mut transcript = String::new();
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(body)) => {
                let value: Value = serde_json::from_str(&body)
                    .map_err(|err| AppError::Realtime(err.to_string()))?;
                if let Some(event_type) = value.get("type").and_then(|v| v.as_str()) {
                    match event_type {
                        "response.output_text.delta" => {
                            if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                                transcript.push_str(delta);
                            }
                        }
                        "response.completed" => break,
                        "error" => {
                            let message = value
                                .get("error")
                                .and_then(|v| v.get("message"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown error");
                            return Err(AppError::Realtime(message.to_string()));
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Close(frame)) => {
                let reason = frame
                    .map(|f| f.reason.to_string())
                    .unwrap_or_else(|| "connection closed".into());
                return Err(AppError::Realtime(reason));
            }
            Ok(_) => continue,
            Err(err) => return Err(AppError::Realtime(err.to_string())),
        }
    }

    if transcript.trim().is_empty() {
        return Err(AppError::Realtime(
            "No transcript received from realtime endpoint".into(),
        ));
    }

    info!(length = transcript.len(), "transcription completed");
    Ok(transcript)
}

fn build_request(api_key: &str, model: &str) -> AppResult<Request<()>> {
    let url = format!("wss://api.openai.com/v1/realtime?model={model}");
    let mut request = url
        .into_client_request()
        .map_err(|err| AppError::Realtime(err.to_string()))?;
    let headers = request.headers_mut();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|err| AppError::Realtime(err.to_string()))?,
    );
    headers.insert("OpenAI-Beta", HeaderValue::from_static("realtime=v1"));
    headers.insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("openai-realtime-v1"),
    );
    Ok(request)
}

fn encode_samples(samples: &[i16]) -> String {
    let mut buf = BytesMut::with_capacity(samples.len() * 2);
    for sample in samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }
    BASE64.encode(&buf)
}

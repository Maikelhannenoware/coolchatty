use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tauri::http::HeaderValue;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};

pub async fn stream_transcription(
    api_key: String,
    model: String,
    sample_rate: u32,
    mut audio_rx: mpsc::Receiver<Vec<i16>>,
) -> Result<String> {
    let url = format!("wss://api.openai.com/v1/realtime?model={model}");
    let mut request = url
        .into_client_request()
        .map_err(|e| anyhow!("invalid websocket url: {e}"))?;
    {
        let headers = request.headers_mut();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|e| anyhow!("invalid auth header: {e}"))?,
        );
        headers.insert("OpenAI-Beta", HeaderValue::from_static("realtime=v1"));
        headers.insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static("realtime"),
        );
    }

    let (ws, _) = connect_async(request).await?;
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
            .await?;
        let ms = (total_samples as f32 / sample_rate as f32) * 1000.0;
        println!(
            "[ws] appended chunk #{chunk_counter} ({} samples) total ~{ms:.1} ms",
            chunk.len()
        );
    }

    if total_samples == 0 {
        return Err(anyhow!(
            "Keine Audioeingabe erkannt. Bitte erneut versuchen."
        ));
    }

    let total_ms = (total_samples as f32 / sample_rate as f32) * 1000.0;
    if total_ms < 100.0 {
        return Err(anyhow!(
            "Aufnahme zu kurz (nur {total_ms:.1} ms). Bitte etwas länger sprechen."
        ));
    }
    println!("[ws] stopping stream with {total_samples} samples (~{total_ms:.1} ms)");

    write
        .send(Message::Text(
            serde_json::json!({"type": "input_audio_buffer.commit"})
                .to_string()
                .into(),
        ))
        .await?;
    write
        .send(Message::Text(
            serde_json::json!({
                "type": "response.create",
                "response": {
                    "modalities": ["text"],
                    "instructions": "Transcribe the latest audio sample",
                }
            })
            .to_string()
            .into(),
        ))
        .await?;

    let mut transcript = String::new();
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(body) => {
                let value: Value = serde_json::from_str(&body)?;
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
                            return Err(anyhow!("OpenAI realtime error: {message}"));
                        }
                        _ => {}
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(transcript)
}

fn encode_samples(samples: &[i16]) -> String {
    let mut buf = BytesMut::with_capacity(samples.len() * 2);
    for sample in samples {
        buf.extend_from_slice(&sample.to_le_bytes());
    }
    BASE64.encode(&buf)
}

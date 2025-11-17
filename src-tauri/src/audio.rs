use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample, StreamConfig};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub struct RecorderRequest {
    pub sample_rate: u32,
    pub input_device: Option<String>,
}

struct ActiveRecorder {
    bridge: thread::JoinHandle<()>,
    stop: Arc<AtomicBool>,
    started_at: Instant,
    receiver: Option<mpsc::Receiver<Vec<i16>>>,
}

pub struct RecorderService {
    inner: Mutex<Option<ActiveRecorder>>,
    session: Mutex<Option<JoinHandle<anyhow::Result<String>>>>,
}

impl RecorderService {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            session: Mutex::new(None),
        }
    }

    pub fn start(&self, request: RecorderRequest) -> Result<()> {
        let mut guard = self.inner.lock();
        if guard.is_some() {
            return Err(anyhow!("recorder already running"));
        }

        let (chunk_tx, chunk_rx) = mpsc::channel(32);
        let stop = Arc::new(AtomicBool::new(false));
        let bridge_stop = stop.clone();
        let device_name = request.input_device.clone();
        let sample_rate = request.sample_rate;

        let bridge = thread::spawn(move || {
            if let Err(err) = capture_loop(device_name, sample_rate, chunk_tx, bridge_stop.clone())
            {
                eprintln!("audio capture failed: {err}");
                bridge_stop.store(true, Ordering::SeqCst);
            }
        });

        *guard = Some(ActiveRecorder {
            bridge,
            stop,
            started_at: Instant::now(),
            receiver: Some(chunk_rx),
        });
        Ok(())
    }

    pub fn take_receiver(&self) -> Option<mpsc::Receiver<Vec<i16>>> {
        self.inner
            .lock()
            .as_mut()
            .and_then(|active| active.receiver.take())
    }

    pub async fn stop(&self) -> Result<Option<Duration>> {
        let handle = {
            let mut guard = self.inner.lock();
            guard.take().map(|active| {
                active.stop.store(true, Ordering::SeqCst);
                (active.bridge, active.started_at)
            })
        };

        if let Some((handle, started_at)) = handle {
            tokio::task::spawn_blocking(move || {
                let _ = handle.join();
            })
            .await?;
            Ok(Some(started_at.elapsed()))
        } else {
            Ok(None)
        }
    }

    pub fn is_recording(&self) -> bool {
        self.inner.lock().is_some()
    }

    pub fn attach_session(&self, handle: JoinHandle<anyhow::Result<String>>) -> Result<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            return Err(anyhow!("session already running"));
        }
        *guard = Some(handle);
        Ok(())
    }

    pub fn take_session(&self) -> Option<JoinHandle<anyhow::Result<String>>> {
        self.session.lock().take()
    }
}

fn capture_loop(
    preferred: Option<String>,
    sample_rate: u32,
    tx: mpsc::Sender<Vec<i16>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let host = cpal::default_host();
    let device = select_input_device(&host, preferred)?;
    let supported = device.default_input_config()?;
    if let Ok(name) = device.name() {
        println!(
            "[audio] using device '{name}' (default {} Hz, {} channels, {:?})",
            supported.sample_rate().0,
            supported.channels(),
            supported.sample_format()
        );
    }
    let mut config: StreamConfig = supported.clone().into();
    config.sample_rate = cpal::SampleRate(sample_rate);

    let (frame_tx, frame_rx) = std_mpsc::channel::<Vec<i16>>();
    let err_fn = |err| eprintln!("audio stream error: {err}");
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, frame_tx.clone(), err_fn)?,
        SampleFormat::I16 => build_stream::<i16>(&device, &config, frame_tx.clone(), err_fn)?,
        SampleFormat::U16 => build_stream::<u16>(&device, &config, frame_tx.clone(), err_fn)?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play()?;

    let mut total_samples = 0usize;
    while !stop.load(Ordering::SeqCst) {
        match frame_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    continue;
                }
                total_samples += chunk.len();
                let chunk_ms = (chunk.len() as f32 / sample_rate as f32) * 1000.0;
                let total_ms = (total_samples as f32 / sample_rate as f32) * 1000.0;
                println!(
                    "[audio] received chunk with {} samples (~{chunk_ms:.1} ms), total ~{total_ms:.1} ms",
                    chunk.len()
                );
                if tx.blocking_send(chunk).is_err() {
                    break;
                }
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }

    if total_samples > 0 {
        let ms = (total_samples as f32 / sample_rate as f32) * 1000.0;
        println!("[audio] captured {total_samples} samples (~{ms:.1} ms) before stopping");
    } else {
        println!("[audio] no samples captured during recording");
    }

    drop(stream);
    Ok(())
}

fn select_input_device(host: &cpal::Host, preferred: Option<String>) -> Result<cpal::Device> {
    if let Some(name) = preferred.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        for device in host.input_devices()? {
            if device.name().map(|n| n == name).unwrap_or(false) {
                return Ok(device);
            }
        }
    }
    host.default_input_device()
        .ok_or_else(|| anyhow!("no audio input device available"))
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    tx: std_mpsc::Sender<Vec<i16>>,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream>
where
    T: Sample + SizedSample + Into<f32>,
{
    let channels = config.channels as usize;
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut chunk = Vec::with_capacity(data.len() / channels);
            for frame in data.chunks(channels) {
                let value: f32 = frame[0].into();
                let clamped = (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                chunk.push(clamped);
            }
            let _ = tx.send(chunk);
        },
        err_fn,
        None,
    )?)
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    Device, Sample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig,
};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::errors::{AppError, AppResult};

const READY_TIMEOUT: Duration = Duration::from_secs(3);
const CHUNK_CHANNEL_CAPACITY: usize = 64;

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
    session: Mutex<Option<JoinHandle<AppResult<String>>>>,
}

impl RecorderService {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            session: Mutex::new(None),
        }
    }

    pub fn start(&self, request: RecorderRequest) -> AppResult<u32> {
        let mut guard = self.inner.lock();
        if guard.is_some() {
            return Err(AppError::RecorderBusy);
        }

        let (chunk_tx, chunk_rx) = mpsc::channel(CHUNK_CHANNEL_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let bridge_stop = stop.clone();
        let device_name = request.input_device.clone();
        let desired_sample_rate = request.sample_rate;
        let (ready_tx, ready_rx) = std_mpsc::channel();

        let bridge = thread::Builder::new()
            .name("audio-bridge".into())
            .spawn(move || {
                if let Err(err) = capture_loop(
                    device_name,
                    desired_sample_rate,
                    chunk_tx,
                    bridge_stop.clone(),
                    ready_tx.clone(),
                ) {
                    error!(error = %err, "audio capture failed");
                    let _ = ready_tx.send(Err(err));
                    bridge_stop.store(true, Ordering::SeqCst);
                }
            })
            .map_err(|err| AppError::AudioInit(err.to_string()))?;

        let sample_rate = match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(rate)) => rate,
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                return Err(AppError::AudioInit(
                    "audio device initialization timed out".into(),
                ))
            }
        };

        *guard = Some(ActiveRecorder {
            bridge,
            stop,
            started_at: Instant::now(),
            receiver: Some(chunk_rx),
        });

        Ok(sample_rate)
    }

    pub fn take_receiver(&self) -> Option<mpsc::Receiver<Vec<i16>>> {
        self.inner
            .lock()
            .as_mut()
            .and_then(|active| active.receiver.take())
    }

    pub async fn stop(&self) -> AppResult<Option<Duration>> {
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
            .await
            .map_err(|err| AppError::AudioInit(err.to_string()))?;
            Ok(Some(started_at.elapsed()))
        } else {
            Ok(None)
        }
    }

    pub fn is_recording(&self) -> bool {
        self.inner.lock().is_some()
    }

    pub fn attach_session(&self, handle: JoinHandle<AppResult<String>>) -> AppResult<()> {
        let mut guard = self.session.lock();
        if guard.is_some() {
            return Err(AppError::RecorderBusy);
        }
        *guard = Some(handle);
        Ok(())
    }

    pub fn take_session(&self) -> Option<JoinHandle<AppResult<String>>> {
        self.session.lock().take()
    }
}

fn capture_loop(
    preferred: Option<String>,
    desired_sample_rate: u32,
    tx: mpsc::Sender<Vec<i16>>,
    stop: Arc<AtomicBool>,
    ready: std_mpsc::Sender<AppResult<u32>>,
) -> AppResult<()> {
    let host = cpal::default_host();
    let device = select_input_device(&host, preferred)?;
    let (supported, sample_rate) = resolve_stream_config(&device, desired_sample_rate)?;
    let config: StreamConfig = supported.clone().into();

    let (frame_tx, frame_rx) = std_mpsc::channel::<Vec<i16>>();
    let err_fn = |err| error!(%err, "audio stream error");
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, frame_tx.clone(), err_fn),
        SampleFormat::I16 => build_stream::<i16>(&device, &config, frame_tx.clone(), err_fn),
        SampleFormat::U16 => build_stream::<u16>(&device, &config, frame_tx.clone(), err_fn),
        other => Err(AppError::AudioInit(format!(
            "unsupported sample format: {other:?}"
        ))),
    };

    let stream = match stream {
        Ok(stream) => {
            if let Err(err) = stream.play() {
                let app_err = AppError::AudioInit(err.to_string());
                let _ = ready.send(Err(app_err.clone()));
                return Err(app_err);
            }
            if let Ok(name) = device.name() {
                info!(
                    device = %name,
                    channels = config.channels,
                    sample_rate,
                    "capturing audio input"
                );
            }
            let _ = ready.send(Ok(sample_rate));
            stream
        }
        Err(err) => {
            let _ = ready.send(Err(err.clone()));
            return Err(err);
        }
    };

    let mut total_samples = 0usize;
    while !stop.load(Ordering::SeqCst) {
        match frame_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) if !chunk.is_empty() => {
                total_samples += chunk.len();
                if let Err(err) = tx.try_send(chunk) {
                    match err {
                        TrySendError::Full(_) => {
                            warn!("audio channel full, dropping samples");
                        }
                        TrySendError::Closed(_) => {
                            break;
                        }
                    }
                }
            }
            Ok(_) => continue,
            Err(std_mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }

    debug!(total_samples, "audio capture loop stopping");
    drop(stream);
    Ok(())
}

fn select_input_device(host: &cpal::Host, preferred: Option<String>) -> AppResult<Device> {
    if let Some(name) = preferred.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        for device in host
            .input_devices()
            .map_err(|err| AppError::AudioDevice(err.to_string()))?
        {
            if device.name().map(|n| n == name).unwrap_or(false) {
                return Ok(device);
            }
        }
        warn!(device = %name, "preferred audio input device not found, falling back to default");
    }

    host.default_input_device()
        .ok_or_else(|| AppError::AudioDevice("no audio input device available".into()))
}

fn resolve_stream_config(
    device: &Device,
    desired_sample_rate: u32,
) -> AppResult<(SupportedStreamConfig, u32)> {
    let mut chosen: Option<SupportedStreamConfig> = None;
    if let Ok(configs) = device.supported_input_configs() {
        for config in configs {
            let min = config.min_sample_rate().0;
            let max = config.max_sample_rate().0;
            if desired_sample_rate >= min && desired_sample_rate <= max {
                let with_rate = config.with_sample_rate(cpal::SampleRate(desired_sample_rate));
                chosen = Some(with_rate);
                break;
            }
        }
    }

    if chosen.is_none() {
        let default = device
            .default_input_config()
            .map_err(|err| AppError::AudioDevice(err.to_string()))?;
        let fallback_rate = default.sample_rate().0;
        chosen = Some(default);
        warn!(
            desired_sample_rate,
            fallback = fallback_rate,
            "falling back to device sample rate"
        );
    }

    let supported = chosen.expect("sample rate resolution failed");
    Ok((supported.clone(), supported.sample_rate().0))
}

fn build_stream<T>(
    device: &Device,
    config: &StreamConfig,
    tx: std_mpsc::Sender<Vec<i16>>,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
) -> AppResult<Stream>
where
    T: Sample + SizedSample + Into<f32>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream(
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
        )
        .map_err(|err| AppError::AudioInit(err.to_string()))
}

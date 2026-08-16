use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use parking_lot::Mutex;

pub struct AudioRecorder {
    session: Mutex<Option<RecordingSession>>,
    initializing: AtomicBool,
    level_bits: Arc<AtomicU32>,
}

struct RecordingSession {
    _stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    started: Instant,
}

pub struct Recording {
    pub samples: Vec<f32>,
    pub duration: Duration,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            initializing: AtomicBool::new(false),
            level_bits: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn start(&self, maximum_duration: Duration) -> Result<()> {
        if self.initializing.swap(true, Ordering::AcqRel) {
            bail!("the microphone is still connecting");
        }
        let result = self.start_inner(maximum_duration);
        self.initializing.store(false, Ordering::Release);
        result
    }

    fn start_inner(&self, maximum_duration: Duration) -> Result<()> {
        if self.session.lock().is_some() {
            bail!("a recording is already active");
        }
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("no microphone input device is available")?;
        let supported = device
            .default_input_config()
            .context("the default microphone configuration is unavailable")?;
        let sample_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let config = supported.config();
        let samples = Arc::new(Mutex::new(Vec::with_capacity(sample_rate as usize * 30)));
        let limit = sample_rate as usize * maximum_duration.as_secs() as usize;
        let error_handler = |error| tracing::error!(%error, "microphone stream error");

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                &config,
                channels,
                samples.clone(),
                self.level_bits.clone(),
                limit,
                error_handler,
            )?,
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                &config,
                channels,
                samples.clone(),
                self.level_bits.clone(),
                limit,
                error_handler,
            )?,
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                &config,
                channels,
                samples.clone(),
                self.level_bits.clone(),
                limit,
                error_handler,
            )?,
            format => bail!("unsupported microphone sample format: {format:?}"),
        };
        stream.play()?;
        *self.session.lock() = Some(RecordingSession {
            _stream: stream,
            samples,
            sample_rate,
            started: Instant::now(),
        });
        Ok(())
    }

    pub fn stop(&self) -> Result<Recording> {
        let session = self
            .session
            .lock()
            .take()
            .context("no recording is active")?;
        self.level_bits.store(0, Ordering::Relaxed);
        let duration = session.started.elapsed();
        let source = session.samples.lock().clone();
        Ok(Recording {
            samples: resample_linear(&source, session.sample_rate, 16_000),
            duration,
        })
    }

    pub fn cancel(&self) {
        self.session.lock().take();
        self.level_bits.store(0, Ordering::Relaxed);
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level_bits.load(Ordering::Relaxed))
    }
}

#[cfg(target_os = "macos")]
pub fn ensure_microphone_access(app: &tauri::AppHandle) -> Result<()> {
    use std::sync::mpsc;

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    let (sender, receiver) = mpsc::sync_channel::<std::result::Result<(), String>>(1);
    app.run_on_main_thread(move || {
        let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
            let _ = sender.send(Err("the macOS audio media type is unavailable".into()));
            return;
        };
        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
        match status {
            AVAuthorizationStatus::Authorized => {
                let _ = sender.send(Ok(()));
            }
            AVAuthorizationStatus::Denied | AVAuthorizationStatus::Restricted => {
                let _ = sender.send(Err(
                    "microphone access is denied in macOS Privacy & Security settings".into(),
                ));
            }
            AVAuthorizationStatus::NotDetermined => {
                let completion = RcBlock::new(move |granted: Bool| {
                    let result = granted
                        .as_bool()
                        .then_some(())
                        .ok_or_else(|| "microphone access was not granted".to_owned());
                    let _ = sender.send(result);
                });
                unsafe {
                    AVCaptureDevice::requestAccessForMediaType_completionHandler(
                        media_type,
                        &completion,
                    );
                }
            }
            _ => {
                let _ = sender.send(Err(
                    "macOS returned an unknown microphone permission state".into()
                ));
            }
        }
    })
    .context("the microphone permission request could not reach the main thread")?;

    match receiver.recv_timeout(Duration::from_secs(60)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => bail!(message),
        Err(_) => bail!("microphone permission request timed out"),
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    samples: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    limit: usize,
    error_handler: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<Stream>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    let stream = device.build_input_stream(
        config.clone(),
        move |input: &[T], _| {
            let mut output = samples.lock();
            if output.len() >= limit {
                return;
            }
            let mut square_sum = 0.0f32;
            let mut frames = 0usize;
            for frame in input.chunks(channels) {
                let mono = frame
                    .iter()
                    .map(|sample| sample.to_sample::<f32>())
                    .sum::<f32>()
                    / channels as f32;
                output.push(mono);
                square_sum += mono * mono;
                frames += 1;
            }
            if frames > 0 {
                let rms = (square_sum / frames as f32).sqrt().clamp(0.0, 1.0);
                level.store(rms.to_bits(), Ordering::Relaxed);
            }
        },
        error_handler,
        None,
    )?;
    Ok(stream)
}

fn resample_linear(source: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if source.is_empty() || source_rate == target_rate {
        return source.to_vec();
    }
    let ratio = source_rate as f64 / target_rate as f64;
    let output_len = (source.len() as f64 / ratio).floor() as usize;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let position = index as f64 * ratio;
        let left = position.floor() as usize;
        let right = (left + 1).min(source.len() - 1);
        let fraction = (position - left as f64) as f32;
        output.push(source[left] * (1.0 - fraction) + source[right] * fraction);
    }
    output
}

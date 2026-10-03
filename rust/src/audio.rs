//! Audio capture: the microphone, or a WAV file for the benchmark, always
//! converted to 16 kHz mono 16-bit, the format that Deepgram gets.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

pub const SAMPLE_RATE: u32 = 16_000;

/// The benchmark sets this to a WAV file that replaces the microphone.
const FAKE_AUDIO_ENV: &str = "DICTATION_FAKE_AUDIO";
const FILE_BLOCK: usize = 320; // 20 ms, like a typical audio callback

/// 0.0 .. 1.0, written by the audio thread, read by the meter (f32 bits).
static LEVEL: AtomicU32 = AtomicU32::new(0);

pub fn level() -> f32 {
    f32::from_bits(LEVEL.load(Ordering::Relaxed))
}

struct Shared {
    samples: Mutex<Vec<i16>>,
    sink: Mutex<Option<Sender<Vec<i16>>>>,
}

enum Source {
    Mic(cpal::Stream),
    File { stop: Arc<AtomicBool>, thread: Option<JoinHandle<()>> },
}

/// A running recording. Call `stop()` to end it and get the samples.
pub struct Recording {
    shared: Arc<Shared>,
    source: Option<Source>,
}

impl Recording {
    /// Stop recording. Returns all samples, and closes the sink, which
    /// tells a streaming session that the audio is complete.
    pub fn stop(mut self) -> Vec<i16> {
        self.end();
        std::mem::take(&mut *self.shared.samples.lock().unwrap())
    }

    fn end(&mut self) {
        match self.source.take() {
            Some(Source::File { stop, thread }) => {
                stop.store(true, Ordering::Relaxed);
                if let Some(t) = thread {
                    let _ = t.join();
                }
            }
            Some(Source::Mic(stream)) => drop(stream),
            None => {}
        }
        self.shared.sink.lock().unwrap().take();
        LEVEL.store(0f32.to_bits(), Ordering::Relaxed);
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.end();
    }
}

/// True if there is a microphone (or the benchmark's WAV file replaces it).
pub fn has_input_device() -> bool {
    std::env::var_os(FAKE_AUDIO_ENV).is_some() || cpal::default_host().default_input_device().is_some()
}

/// Start recording. Each block of 16 kHz samples also goes to `sink`.
pub fn start(sink: Option<Sender<Vec<i16>>>) -> Result<Recording, String> {
    let shared = Arc::new(Shared { samples: Mutex::new(Vec::new()), sink: Mutex::new(sink) });
    let source = match std::env::var_os(FAKE_AUDIO_ENV) {
        Some(path) => start_file(path.into(), shared.clone())?,
        None => start_mic(shared.clone())?,
    };
    Ok(Recording { shared, source: Some(source) })
}

fn push(shared: &Shared, block: Vec<i16>) {
    update_level(&block);
    shared.samples.lock().unwrap().extend_from_slice(&block);
    if let Some(sink) = shared.sink.lock().unwrap().as_ref() {
        let _ = sink.send(block);
    }
}

fn update_level(block: &[i16]) {
    if block.is_empty() {
        return;
    }
    let sum: f64 = block.iter().map(|&s| (s as f64 / 32768.0).powi(2)).sum();
    let rms = (sum / block.len() as f64).sqrt() as f32 + 1e-9;
    // -60 dB reads as silence, -5 dB as full scale (same as dictate.py)
    let level = ((20.0 * rms.log10() + 60.0) / 55.0).clamp(0.0, 1.0);
    LEVEL.store(level.to_bits(), Ordering::Relaxed);
}

// -- microphone -------------------------------------------------------------

fn start_mic(shared: Arc<Shared>) -> Result<Source, String> {
    let device = cpal::default_host().default_input_device().ok_or("no input device")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let channels = config.channels as usize;
    let mut resampler = Resampler::new(config.sample_rate.0);

    // one closure per sample format, all ending in the same mono f32 path
    macro_rules! stream {
        ($t:ty, $to_f32:expr) => {
            device.build_input_stream(
                &config,
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let to_f32 = $to_f32;
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|frame| frame.iter().map(|&s| to_f32(s)).sum::<f32>() / channels as f32)
                        .collect();
                    push(&shared, resampler.process(&mono));
                },
                |e| eprintln!("audio: {e}"),
                None,
            )
        };
    }
    let stream = match format {
        SampleFormat::F32 => stream!(f32, |s: f32| s),
        SampleFormat::I16 => stream!(i16, |s: i16| s as f32 / 32768.0),
        SampleFormat::U16 => stream!(u16, |s: u16| (s as f32 - 32768.0) / 32768.0),
        other => return Err(format!("unsupported sample format {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Source::Mic(stream))
}

/// Converts mono f32 audio at the device rate to 16 kHz i16. When it
/// downsamples, it averages the input samples that each output sample
/// covers, which removes most of the aliasing.
struct Resampler {
    step: f64, // input samples per output sample
    pos: f64,
    buf: Vec<f32>,
}

impl Resampler {
    fn new(input_rate: u32) -> Self {
        Resampler { step: input_rate as f64 / SAMPLE_RATE as f64, pos: 0.0, buf: Vec::new() }
    }

    fn process(&mut self, input: &[f32]) -> Vec<i16> {
        self.buf.extend_from_slice(input);
        let width = self.step.round().max(1.0) as usize;
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 1);
        while (self.pos as usize) + width.max(2) <= self.buf.len() {
            let i = self.pos as usize;
            let s = if width > 1 {
                self.buf[i..i + width].iter().sum::<f32>() / width as f32
            } else {
                let f = (self.pos - i as f64) as f32;
                self.buf[i] * (1.0 - f) + self.buf[i + 1] * f
            };
            out.push((s.clamp(-1.0, 1.0) * 32767.0) as i16);
            self.pos += self.step;
        }
        let used = (self.pos as usize).min(self.buf.len());
        self.buf.drain(..used);
        self.pos -= used as f64;
        out
    }
}

// -- WAV file (benchmark) ---------------------------------------------------

/// Plays the file from the start in real time, then silence.
fn start_file(path: std::path::PathBuf, shared: Arc<Shared>) -> Result<Source, String> {
    let mut reader = hound::WavReader::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != SAMPLE_RATE || spec.channels != 1 || spec.bits_per_sample != 16 {
        return Err(format!("{}: must be 16 kHz, mono, 16-bit", path.display()));
    }
    let pcm: Vec<i16> = reader.samples::<i16>().filter_map(Result::ok).collect();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let thread = thread::spawn(move || {
        let start = Instant::now();
        let mut n = 0usize;
        while !stop_thread.load(Ordering::Relaxed) {
            let from = (n * FILE_BLOCK).min(pcm.len());
            let mut block = pcm[from..(from + FILE_BLOCK).min(pcm.len())].to_vec();
            block.resize(FILE_BLOCK, 0);
            push(&shared, block);
            n += 1;
            let due = start + Duration::from_secs_f64((n * FILE_BLOCK) as f64 / SAMPLE_RATE as f64);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                thread::sleep(wait);
            }
        }
    });
    Ok(Source::File { stop, thread: Some(thread) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_48k_to_16k_keeps_the_length() {
        let mut r = Resampler::new(48_000);
        let out: usize = (0..100).map(|_| r.process(&[0.5; 480]).len()).sum();
        assert!((out as i64 - 16_000).abs() <= 2, "got {out}");
    }

    #[test]
    fn resampler_keeps_a_constant_level() {
        let mut r = Resampler::new(44_100);
        let out = r.process(&[0.5; 4410]);
        assert!(out.iter().all(|&s| (s - 16383).abs() <= 1));
    }

    #[test]
    fn resampler_16k_passes_through() {
        let mut r = Resampler::new(16_000);
        let input: Vec<f32> = (0..160).map(|i| i as f32 / 1000.0).collect();
        let out = r.process(&input);
        assert_eq!(out.len(), 159); // the last sample waits for the next block
        assert_eq!(out[10], (0.010f32 * 32767.0) as i16);
    }
}

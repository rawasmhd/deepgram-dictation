//! Microphone capture. The prototype only needs the input level for the
//! meter; sending the audio to Deepgram comes in step 3.

use std::sync::atomic::{AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

/// 0.0 .. 1.0, written by the audio thread, read by the meter (f32 bits).
static LEVEL: AtomicU32 = AtomicU32::new(0);

pub fn level() -> f32 {
    f32::from_bits(LEVEL.load(Ordering::Relaxed))
}

/// An open input stream. Recording stops when this is dropped.
pub struct Mic {
    _stream: cpal::Stream,
}

impl Drop for Mic {
    fn drop(&mut self) {
        LEVEL.store(0f32.to_bits(), Ordering::Relaxed);
    }
}

pub fn start() -> Result<Mic, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("no input device")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();

    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            |data: &[f32], _: &cpal::InputCallbackInfo| update(data.iter().copied()),
            on_error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            |data: &[i16], _: &cpal::InputCallbackInfo| {
                update(data.iter().map(|&s| s as f32 / 32768.0))
            },
            on_error,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            |data: &[u16], _: &cpal::InputCallbackInfo| {
                update(data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0))
            },
            on_error,
            None,
        ),
        other => return Err(format!("unsupported sample format {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    Ok(Mic { _stream: stream })
}

fn on_error(e: cpal::StreamError) {
    eprintln!("audio: {e}");
}

fn update(samples: impl Iterator<Item = f32>) {
    let (mut sum, mut n) = (0.0f64, 0usize);
    for s in samples {
        sum += (s * s) as f64;
        n += 1;
    }
    if n == 0 {
        return;
    }
    let rms = (sum / n as f64).sqrt() as f32 + 1e-9;
    let db = 20.0 * rms.log10();
    // -60 dB reads as silence, -5 dB as full scale (same as dictate.py)
    let level = ((db + 60.0) / 55.0).clamp(0.0, 1.0);
    LEVEL.store(level.to_bits(), Ordering::Relaxed);
}

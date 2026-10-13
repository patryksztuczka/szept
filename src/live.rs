//! A live captioning session: capture audio with cpal, convert it to 16 kHz
//! mono PCM, and stream it to Soniox. Two threads do the work, so nothing
//! here blocks the UI; events come back on a channel.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use tokio::sync::mpsc::UnboundedSender;

use crate::settings::AudioSource;
use crate::soniox::{self, Event};

/// Audio is sent in 100 ms chunks.
const CHUNK_BYTES: usize = (soniox::SAMPLE_RATE as usize / 10) * 2;

/// Stops capturing when dropped. Soniox then sends its last tokens and the
/// event channel ends with [`Event::Finished`].
pub struct Session {
    _stop: mpsc::Sender<()>,
}

pub fn start(source: AudioSource, api_key: String) -> (Session, async_channel::Receiver<Event>) {
    let (events_tx, events_rx) = async_channel::unbounded();
    let (audio_tx, audio_rx) = tokio::sync::mpsc::unbounded_channel();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let events = events_tx.clone();
    thread::Builder::new()
        .name("soniox".into())
        .spawn(move || soniox::run(&api_key, audio_rx, &events))
        .expect("could not start the soniox thread");

    thread::Builder::new()
        .name("audio-capture".into())
        .spawn(
            move || match open_stream(source, audio_tx, events_tx.clone()) {
                Ok(stream) => {
                    // Blocks until the session is dropped; then the stream and
                    // its sender drop, which ends the Soniox session.
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(error) => {
                    tracing::warn!(%error, "could not start audio capture");
                    events_tx.send_blocking(Event::Failed(error)).ok();
                }
            },
        )
        .expect("could not start the audio thread");

    (Session { _stop: stop_tx }, events_rx)
}

/// Prints the peak level of each 100 ms chunk for `seconds`. Checks that
/// capture works, and that macOS granted access, without using Soniox.
pub fn meter(source: AudioSource, seconds: u64) -> Result<(), String> {
    let (audio_tx, _audio_rx) = tokio::sync::mpsc::unbounded_channel();
    let (events_tx, events_rx) = async_channel::unbounded();
    let _stream = open_stream(source, audio_tx, events_tx)?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        match events_rx.try_recv() {
            Ok(Event::Level(level)) => {
                let bar = "#".repeat((level.sqrt() * 50.0) as usize);
                println!("{level:7.4} {bar}");
            }
            Ok(Event::Notice(notice)) => eprintln!("{notice}"),
            Ok(_) => {}
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
    Ok(())
}

fn open_stream(
    source: AudioSource,
    audio: UnboundedSender<Vec<u8>>,
    events: async_channel::Sender<Event>,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let (device, config) = match source {
        // cpal records an output device through a Core Audio process tap,
        // which needs the "System Audio Recording" permission.
        AudioSource::Meeting => {
            let device = host
                .default_output_device()
                .ok_or("No audio output device found.")?;
            let config = device
                .default_output_config()
                .map_err(|e| format!("Could not read the output format: {e}"))?;
            (device, config)
        }
        AudioSource::Microphone => {
            let device = host.default_input_device().ok_or("No microphone found.")?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("Could not read the microphone format: {e}"))?;
            (device, config)
        }
    };
    tracing::info!(?source, ?config, "starting audio capture");

    let converter = Converter::new(config.sample_rate(), config.channels(), soniox::SAMPLE_RATE);
    let error_events = events.clone();
    let on_error = move |error: cpal::Error| {
        tracing::warn!(%error, "audio stream error");
        if !matches!(error.kind(), cpal::ErrorKind::Xrun) {
            error_events
                .try_send(Event::Notice(format!("Audio: {error}")))
                .ok();
        }
    };
    let stream_config = config.config();
    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, stream_config, converter, audio, events, on_error),
        SampleFormat::I16 => build::<i16>(&device, stream_config, converter, audio, events, on_error),
        SampleFormat::I32 => build::<i32>(&device, stream_config, converter, audio, events, on_error),
        SampleFormat::U16 => build::<u16>(&device, stream_config, converter, audio, events, on_error),
        format => return Err(format!("Unsupported audio format {format}.")),
    }
    .map_err(|error| match source {
        AudioSource::Meeting => format!(
            "Could not capture meeting audio: {error}. Allow {} under System Settings → Privacy & Security → Screen & System Audio Recording.",
            crate::APP_NAME
        ),
        AudioSource::Microphone => format!("Could not open the microphone: {error}"),
    })?;
    stream
        .play()
        .map_err(|e| format!("Could not start audio: {e}"))?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut converter: Converter,
    audio: UnboundedSender<Vec<u8>>,
    events: async_channel::Sender<Event>,
    on_error: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream::<T, _, _>(
        config,
        // Runs on the real-time audio thread: no locks, no blocking sends.
        move |data: &[T], _| {
            converter.push(data, |chunk, peak| {
                audio.send(chunk).ok();
                events.try_send(Event::Level(peak)).ok();
            });
        },
        on_error,
        None,
    )
}

/// Downmixes interleaved frames to mono, resamples by averaging each output
/// period (a box filter, enough for speech), and packs 16-bit little-endian
/// chunks.
struct Converter {
    channels: usize,
    /// Input samples per output sample.
    step: f64,
    /// Input samples left until the next output sample.
    until_next: f64,
    sum: f32,
    count: u32,
    last: f32,
    chunk: Vec<u8>,
    peak: f32,
}

impl Converter {
    fn new(input_rate: u32, channels: u16, output_rate: u32) -> Self {
        let step = f64::from(input_rate) / f64::from(output_rate);
        Self {
            channels: usize::from(channels.max(1)),
            step,
            until_next: step,
            sum: 0.0,
            count: 0,
            last: 0.0,
            chunk: Vec::with_capacity(CHUNK_BYTES),
            peak: 0.0,
        }
    }

    fn push<T>(&mut self, interleaved: &[T], mut on_chunk: impl FnMut(Vec<u8>, f32))
    where
        T: Sample,
        f32: FromSample<T>,
    {
        for frame in interleaved.chunks(self.channels) {
            let mono = frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / frame.len() as f32;
            self.sum += mono;
            self.count += 1;
            self.until_next -= 1.0;
            while self.until_next <= 0.0 {
                if self.count > 0 {
                    self.last = self.sum / self.count as f32;
                    self.sum = 0.0;
                    self.count = 0;
                }
                let sample = self.last.clamp(-1.0, 1.0);
                self.peak = self.peak.max(sample.abs());
                let value = (sample * f32::from(i16::MAX)) as i16;
                self.chunk.extend_from_slice(&value.to_le_bytes());
                if self.chunk.len() >= CHUNK_BYTES {
                    let chunk = std::mem::replace(&mut self.chunk, Vec::with_capacity(CHUNK_BYTES));
                    on_chunk(chunk, std::mem::take(&mut self.peak));
                }
                self.until_next += self.step;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(input_rate: u32, channels: u16, input: &[f32]) -> Vec<i16> {
        let mut converter = Converter::new(input_rate, channels, 16_000);
        let mut bytes = Vec::new();
        converter.push(input, |chunk, _| bytes.extend(chunk));
        bytes.extend(&converter.chunk);
        bytes
            .chunks(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }

    #[test]
    fn downsamples_48k_stereo_to_16k_mono() {
        // One second of stereo audio becomes one second at 16 kHz.
        let input = vec![0.5f32; 48_000 * 2];
        let output = convert(48_000, 2, &input);
        assert_eq!(output.len(), 16_000);
        assert!(output.iter().all(|&s| s == (0.5 * 32767.0) as i16));
    }

    #[test]
    fn handles_fractional_ratios() {
        let output = convert(44_100, 1, &vec![0.0; 44_100]);
        assert!((15_999..=16_000).contains(&output.len()));
    }

    #[test]
    fn averages_channels_and_clamps() {
        let output = convert(16_000, 2, &[1.0, -1.0, 2.0, 2.0]);
        assert_eq!(output, vec![0, i16::MAX]);
    }

    #[test]
    fn emits_100ms_chunks_with_peak() {
        let mut converter = Converter::new(16_000, 1, 16_000);
        let mut chunks = Vec::new();
        let mut input = vec![0.1f32; 3_200];
        input[10] = -0.8;
        converter.push(&input, |chunk, peak| chunks.push((chunk.len(), peak)));
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], (CHUNK_BYTES, 0.8));
        assert!((chunks[1].1 - 0.1).abs() < 1e-6);
    }
}

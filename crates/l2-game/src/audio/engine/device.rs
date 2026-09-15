#![allow(unused_imports)]
use super::*;
use super::engine::*;
use super::*;
use super::director::*;
use super::events::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mixer::Mixer;
use track::{BattleCycle, BattleKind, Music};
use wav::Sound;

pub(super) fn start_device(mixer: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("no default output config: {e}"))?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    if let Ok(mut m) = mixer.lock() {
        *m = Mixer::new(rate);
    }
    let config: cpal::StreamConfig = supported.config();
    let err = |e| eprintln!("sound: stream error: {e}");

    macro_rules! stream {
        ($sample:ty, $to:expr) => {{
            let mixer = Arc::clone(&mixer);
            let mut scratch: Vec<f32> = Vec::new();
            device
                .build_output_stream(
                    &config,
                    move |out: &mut [$sample], _| {
                        let frames = out.len() / channels.max(1);
                        scratch.resize(frames * 2, 0.0);
                        match mixer.lock() {
                            Ok(mut m) => m.fill(&mut scratch),
                            Err(_) => scratch.iter_mut().for_each(|s| *s = 0.0),
                        }
                        for (i, frame) in out.chunks_mut(channels.max(1)).enumerate() {
                            for (c, slot) in frame.iter_mut().enumerate() {
                                let v = if c < 2 { scratch[i * 2 + c] } else { 0.0 };
                                *slot = $to(v);
                            }
                        }
                    },
                    err,
                    None,
                )
                .map_err(|e| format!("{e}"))?
        }};
    }

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => stream!(f32, |v: f32| v),
        cpal::SampleFormat::I16 => stream!(i16, |v: f32| (v * 32767.0) as i16),
        cpal::SampleFormat::U16 => stream!(u16, |v: f32| ((v * 32767.0) as i32 + 32768) as u16),
        other => return Err(format!("unsupported sample format {other:?}")),
    };
    stream.play().map_err(|e| format!("{e}"))?;
    Ok(stream)
}



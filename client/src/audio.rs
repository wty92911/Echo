//! Audio Processor Module.
//!
//! Use [`cpal`] to capture and play audio.
//! Support volume control and audio mixing.

use core::f32;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Add;
use std::sync::{Arc, Mutex};

use crate::config::UserConfig;
use crate::utils::Buffer;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Stream, SupportedStreamConfig};
use ringbuffer::AllocRingBuffer;

pub const SAMPLE_RATE: u32 = 44100; // so huge
const MAX_PERCENT: u8 = 200;
const DEFAULT_SAMPLE_FORMAT: cpal::SampleFormat = cpal::SampleFormat::F32;

/// Mix audio data from different users according to volume config.
/// Simply add all audio data together.
pub fn mix<T>(data: &mut [T], audio: HashMap<String, Vec<T>>, config: &UserConfig, channels: usize)
where
    T: Debug + Into<f32> + From<f32> + Default + Clone + Copy + Add<Output = T>,
{
    let mut mix_audio = vec![T::default(); data.len() / channels];
    for (name, mut audio) in audio.into_iter() {
        // apply other volume configs
        if let Some(factor) = config.other_volume.get(&name) {
            multiply(&mut audio, *factor);
        }

        // resize(&mut audio, data.len());
        add(&mut mix_audio, &audio);
    }
    // apply own volume config
    multiply(&mut mix_audio, config.output_volume);
    // add mix audio data to data
    for i in 0..data.len() {
        data[i] = data[i] + mix_audio[i / channels];
    }

    // todo: normalize data
}

/// Multiply audio data by factor.
/// factor is in range [0, 200] percent.
///
/// For example, 100 means no change.
/// larger than 100 means volume up.
/// smaller than 100 means volume down.
/// 0 means mute.
pub fn multiply<T>(data: &mut [T], factor: u8)
where
    T: Copy + Into<f32> + From<f32>,
{
    let factor = if factor > MAX_PERCENT {
        MAX_PERCENT
    } else {
        factor
    };
    let factor = factor as f32 / 100.0;

    data.iter_mut()
        .for_each(|v| *v = T::from((*v).into() * factor));
}

/// Add audio data from `audio` to `mix_audio`.
///
/// `mix_audio` and `audio` may have different lengths. The operation will only be performed
/// up to the length of the shorter slice.
pub fn add<T>(mix_audio: &mut [T], audio: &[T])
where
    T: Copy + Add<Output = T>,
{
    let len = mix_audio.len().min(audio.len());

    for i in 0..len {
        mix_audio[i] = mix_audio[i] + audio[i];
    }
}

/// Resize audio data.
///
/// padding or down sample
pub fn resize<T>(audio: &mut Vec<T>, size: usize)
where
    T: Clone + Default + Copy,
{
    match audio.len().cmp(&size) {
        Ordering::Less => {
            // padding data for backlog
            audio.extend(vec![T::default(); size - audio.len()]);
        }
        Ordering::Equal => {}
        Ordering::Greater => {
            // down sample data for backlog
            for i in 0..size {
                let j = i * audio.len() / size;
                audio[i] = audio[j];
            }
            audio.resize(size, T::default());
        }
    }
}

/// Wrapper for playing audio data on some device.
/// Only supporting f32 sample format.
pub struct Speaker {
    device: Device,
    config: SupportedStreamConfig,
}

impl Default for Speaker {
    fn default() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");
        let mut supported_configs_range = device
            .supported_output_configs()
            .expect("error while querying configs");

        let config = supported_configs_range
            .find(|config| config.sample_format() == DEFAULT_SAMPLE_FORMAT)
            .map(|config| config.with_sample_rate(cpal::SampleRate(SAMPLE_RATE)))
            .unwrap();
        Self { device, config }
    }
}

impl Speaker {
    pub fn new(device: Device, config: SupportedStreamConfig) -> Self {
        Self { device, config }
    }

    /// Play audio data from `buf`.
    /// stop when returning [`Stream`] dropped.
    pub fn play(&mut self, buf: Arc<Mutex<Buffer<f32>>>, config: Arc<UserConfig>) -> Stream {
        let cnt = self.config.config().channels as usize;
        let stream = self
            .device
            .build_output_stream(
                &self.config.config(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // react to stream events and read or write stream data here.
                    let audio = buf.lock().unwrap().flush(data.len() / cnt);

                    mix(data, audio, &config, cnt);
                },
                move |_err| {
                    // react to errors here.
                },
                None, // None=blocking, Some(Duration)=timeout
            )
            .unwrap();
        stream
    }
}

/// Wrapper for recording audio data from some device.
/// Only supporting f32 sample format.
pub struct Microphone {
    device: Device,
    config: SupportedStreamConfig,
}

impl Default for Microphone {
    fn default() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("no input device available");
        let mut supported_configs_range = device
            .supported_input_configs()
            .expect("error while querying configs");

        let config = supported_configs_range
            .find(|config| config.sample_format() == DEFAULT_SAMPLE_FORMAT)
            .map(|config| config.with_sample_rate(cpal::SampleRate(SAMPLE_RATE)))
            .unwrap();
        assert!(config.channels() == 1); // for now we only support one channel mic.
        Self::new(device, config)
    }
}

impl Microphone {
    pub fn new(device: Device, config: SupportedStreamConfig) -> Self {
        Self { device, config }
    }

    /// Record audio data into `buffer`
    /// stop when returning [`Stream`] dropped.
    pub fn record(
        &mut self,
        buffer: Arc<Mutex<AllocRingBuffer<f32>>>,
        _config: Arc<UserConfig>,
    ) -> Stream {
        let stream = self
            .device
            .build_input_stream(
                &self.config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // apply user config
                    // multiply(&mut data, config.input_volume);
                    buffer.lock().unwrap().extend(data.to_vec());
                },
                move |_err| {
                    // react to errors here.
                },
                None, // None=blocking, Some(Duration)=timeout
            )
            .unwrap();
        stream
    }
}

#[cfg(test)]
mod test {
    use std::{thread, time};

    use crate::utils::RING_BUFFER_SIZE;

    use super::*;
    use cpal::traits::StreamTrait;
    use ringbuffer::RingBuffer;

    #[ignore = "only manual test"]
    #[test]
    fn test_audio() {
        let config = Arc::new(UserConfig::default());
        let buf = Arc::new(Mutex::new(AllocRingBuffer::new(RING_BUFFER_SIZE)));
        let mut mic = Microphone::default();
        dbg!(&mic.config);
        let stream = mic.record(buf.clone(), config.clone());
        stream.play().unwrap();

        let mut speaker = Speaker::default();
        dbg!(&speaker.config);
        let buffer = Arc::new(Mutex::new(Buffer::new()));

        let speaker_stream = speaker.play(buffer.clone(), config);
        speaker_stream.play().unwrap();
        thread::sleep(time::Duration::from_millis(3000));
        let mut buf = buf.lock().unwrap();
        if buf.len() > 0 {
            let data = buf.to_vec();
            println!("buf.len: {}", buf.len());
            buf.clear();
            buffer.lock().unwrap().extend("test_user".to_string(), data);
        }
        thread::sleep(time::Duration::from_millis(3000));
    }
}

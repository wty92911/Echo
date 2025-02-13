//! Audio Processor Module.
//!
//! Use [`cpal`] to capture and play audio.
//! Support volume control and audio mixing.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Add;
use std::sync::{Arc, Mutex};

use crate::config::UserConfig;
use crate::utils::{FromBytes, ToBytes};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Stream, SupportedStreamConfig};
use ringbuffer::{AllocRingBuffer, RingBuffer};

const SAMPLE_RATE: u32 = 44100;
const MAX_PERCENT: u8 = 200;
pub const RING_BUFFER_SIZE: usize = SAMPLE_RATE as usize;
const DEFAULT_SAMPLE_FORMAT: cpal::SampleFormat = cpal::SampleFormat::F32;

pub fn mix<T>(data: &mut [T], audio: HashMap<String, Vec<u8>>, config: &UserConfig)
where
    T: FromBytes + Into<f32> + From<f32> + Default + Clone + Copy + Add<Output = T>,
{
    let mut mix_audio = vec![T::default(); data.len()];
    for (name, audio) in audio.into_iter() {
        // transfer bytes data to T data
        let mut audio = <T>::from_bytes(audio);

        // apply other volume configs
        if let Some(factor) = config.other_volume.get(&name) {
            multiply(&mut audio, *factor);
        }

        resize(&mut audio, data.len());

        //
        add(&mut mix_audio, &audio);
    }
    // apply own volume config
    multiply(&mut mix_audio, config.output_volume);
    // add mix audio data to data
    add(data, &mix_audio);

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
    pub fn play(&mut self, buf: Arc<Mutex<Buffer>>, config: Arc<UserConfig>) -> Stream {
        let stream = self
            .device
            .build_output_stream(
                &self.config.config(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // react to stream events and read or write stream data here.
                    let audio = buf.lock().unwrap().flush(data.len());
                    mix(data, audio, &config);
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
        buffer: Arc<Mutex<AllocRingBuffer<u8>>>,
        config: Arc<UserConfig>,
    ) -> Stream {
        let stream = self
            .device
            .build_input_stream(
                &self.config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // apply user config
                    let mut data = data.to_vec();
                    multiply(&mut data, config.input_volume);
                    buffer.lock().unwrap().extend(data.to_bytes());
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

/// Buffer is holding audio data for each user.
/// inner mutable, use [`Arc`] and [`Mutex`] to share data between threads.
#[derive(Default)]
pub struct Buffer {
    data: HashMap<String, AllocRingBuffer<u8>>, // user - [audio data]
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Extend data for a user.
    pub fn extend(&mut self, user: String, data: Vec<u8>) {
        self.data
            .entry(user)
            .or_insert_with(|| AllocRingBuffer::new(RING_BUFFER_SIZE))
            .extend(data);
    }

    /// Flush all users' data
    pub fn flush(&mut self, length: usize) -> HashMap<String, Vec<u8>> {
        let mut flushed = HashMap::new();
        for (user, buffer) in self.data.iter_mut() {
            let drained: Vec<u8> = buffer.drain().collect();
            let remaining: Vec<u8> = drained.iter().skip(length).cloned().collect();
            let flushed_data = drained.iter().take(length).cloned().collect();
            flushed.insert(user.clone(), flushed_data);
            buffer.extend(remaining);
        }
        flushed
    }
}

mod test {}

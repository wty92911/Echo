use std::collections::HashMap;

use ringbuffer::{AllocRingBuffer, RingBuffer};

use crate::audio::SAMPLE_RATE;

/// Convert bytes to T.
///
/// It is assumed that the buffer contains data in little-endian format.
///
/// The extra bytes will be discarded.
pub trait FromBytes: Sized {
    fn from_bytes(bytes: Vec<u8>) -> Vec<Self>;
}

macro_rules! impl_from_ring_bytes {
    ($t:ty, $chunk_size:expr) => {
        impl FromBytes for $t {
            fn from_bytes(bytes: Vec<u8>) -> Vec<Self> {
                let mut values = Vec::new();
                let mut chunk = [0u8; $chunk_size];
                let mut i = 0;
                bytes.into_iter().for_each(|b| {
                    chunk[i] = b;
                    i += 1;
                    if i == $chunk_size {
                        values.push(<$t>::from_le_bytes(chunk));
                        i = 0;
                    }
                });
                values
            }
        }
    };
}

// Implement FromRingBytes for f32 (4 bytes)
impl_from_ring_bytes!(f32, 4);

// Implement FromRingBytes for u32 (4 bytes)
impl_from_ring_bytes!(u32, 4);

// Implement FromRingBytes for i32 (4 bytes)
impl_from_ring_bytes!(i32, 4);

// Implement FromRingBytes for i64 (8 bytes)
impl_from_ring_bytes!(i64, 8);

// Implement FromRingBytes for u64 (8 bytes)
impl_from_ring_bytes!(u64, 8);

/// Convert Vec<T> to bytes.
pub trait ToBytes {
    fn to_bytes(&self) -> Vec<u8>;
}

macro_rules! impl_to_bytes {
    ($t:ty) => {
        impl ToBytes for Vec<$t> {
            fn to_bytes(&self) -> Vec<u8> {
                let mut bytes = Vec::new();
                self.iter().for_each(|v| {
                    bytes.extend_from_slice(&v.to_le_bytes());
                });
                bytes
            }
        }
    };
}

// Implement ToBytes for Vec<f32> (4 bytes)
impl_to_bytes!(f32);

// Implement ToBytes for Vec<u32> (4 bytes)
impl_to_bytes!(u32);

// Implement ToBytes for Vec<i32> (4 bytes)
impl_to_bytes!(i32);

// Implement ToBytes for Vec<i64> (8 bytes)
impl_to_bytes!(i64);

// Implement ToBytes for Vec<u64> (8 bytes)
impl_to_bytes!(u64);

pub const RING_BUFFER_SIZE: usize = SAMPLE_RATE as usize * 10;

/// Buffer is holding audio data for each user.
/// inner mutable, use [`Arc`] and [`Mutex`] to share data between threads.
#[derive(Default)]
pub struct Buffer<T> {
    data: HashMap<String, AllocRingBuffer<T>>, // user - [audio data]
}

impl<T: Clone> Buffer<T> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Extend data for a user.
    pub fn extend(&mut self, user: String, data: impl IntoIterator<Item = T>) {
        self.data
            .entry(user)
            .or_insert_with(|| AllocRingBuffer::new(RING_BUFFER_SIZE))
            .extend(data);
    }

    /// Flush all users' data
    pub fn flush(&mut self, length: usize) -> HashMap<String, Vec<T>> {
        let mut flushed = HashMap::new();
        for (user, buffer) in self.data.iter_mut() {
            let drained: Vec<T> = buffer.drain().collect();
            let remaining: Vec<T> = drained.iter().skip(length).cloned().collect();
            let flushed_data: Vec<T> = drained.iter().take(length).cloned().collect();
            println!("{}, {}", flushed_data.len(), remaining.len());
            flushed.insert(user.clone(), flushed_data);
            buffer.extend(remaining);
        }
        flushed
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_from_bytes_f32() {
        let bytes = vec![0, 0, 128, 63]; // 1.0 in f32
        let expected = vec![1.0f32];
        let result = f32::from_bytes(bytes);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_from_bytes_u32() {
        let bytes = vec![1, 0, 0, 0]; // 1 in u32
        let expected = vec![1u32];
        let result = u32::from_bytes(bytes);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_from_bytes_i32() {
        let bytes = vec![255, 255, 255, 255]; // -1 in i32
        let expected = vec![-1i32];
        let result = i32::from_bytes(bytes);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_to_bytes_f32() {
        let data = vec![1.0f32];
        let expected = vec![0, 0, 128, 63]; // 1.0 in f32
        let result = data.to_bytes();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_to_bytes_u32() {
        let data = vec![1u32];
        let expected = vec![1, 0, 0, 0]; // 1 in u32
        let result = data.to_bytes();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_to_bytes_i32() {
        let data = vec![-1i32];
        let expected = vec![255, 255, 255, 255]; // -1 in i32
        let result = data.to_bytes();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_buffer_extend_and_flush() {
        let mut buffer = Buffer::new();
        let user = String::from("user1");

        // Extend buffer with some data
        buffer.extend(user.clone(), vec![1, 2, 3, 4, 5]);

        // Check internal state of data after extending
        assert_eq!(
            buffer.data.get(&user).unwrap().to_vec(),
            vec![1, 2, 3, 4, 5]
        );

        // Flush the buffer with a given length
        let flushed_data = buffer.flush(3);

        // Check the flushed data
        assert_eq!(flushed_data.get(&user).unwrap(), &vec![1, 2, 3]);

        // Check the remaining data in the buffer for the user
        assert_eq!(buffer.data.get(&user).unwrap().to_vec(), vec![4, 5]);
    }
}

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

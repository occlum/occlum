//! This file provides ability of serialize/deserialize.
use crate::prelude::*;

use std::collections::HashMap;
use std::convert::TryInto;
use std::mem::size_of;

pub const BA_SIZE: usize = size_of::<Lba>();
pub const USIZE_SIZE: usize = size_of::<usize>();
pub const U32_SIZE: usize = size_of::<u32>();
pub const BITMAP_UNIT: usize = 8;

/// Provide ability for encode/decode
/// from bytes/type to type/bytes.
///
/// Used for exchanging data between memory and device.
pub trait Serialize {
    /// Encode from struct to bytes.
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()>;

    /// Decode bytes to struct.
    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized;

    /// Encoded binary length of bytes.
    fn bytes_len(&self) -> Option<usize> {
        None
    }
}

pub trait Encoder {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()>;
}

impl Encoder for Vec<u8> {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

impl Encoder for [u8; BA_SIZE] {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        debug_assert!(self.len() == buf.len());
        self.copy_from_slice(buf);
        Ok(())
    }
}

impl Encoder for [u8] {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        debug_assert!(self.len() == buf.len());
        self.copy_from_slice(buf);
        Ok(())
    }
}

impl Serialize for Lba {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        encoder.write_bytes(&self.to_raw().to_le_bytes())
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let decode_err = EINVAL;
        Ok(Self::new(RawBid::from_le_bytes(
            buf.try_into().map_err(|_| decode_err)?,
        )))
    }

    fn bytes_len(&self) -> Option<usize> {
        Some(self.to_raw().to_le_bytes().len())
    }
}

impl Serialize for usize {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        encoder.write_bytes(&self.to_le_bytes())
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let decode_err = EINVAL;
        Ok(Self::from_le_bytes(buf.try_into().map_err(|_| decode_err)?))
    }

    fn bytes_len(&self) -> Option<usize> {
        Some(self.to_le_bytes().len())
    }
}

impl<K: Serialize + std::cmp::Eq + std::hash::Hash, V: Serialize> Serialize for HashMap<K, V> {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        let cap = self.len();
        encoder.write_bytes(&cap.to_le_bytes())?;
        for (idx, (k, v)) in self.iter().enumerate() {
            if idx == 0 {
                let k_len = k.bytes_len().unwrap();
                let v_len = v.bytes_len().unwrap();
                encoder.write_bytes(&k_len.to_le_bytes())?;
                encoder.write_bytes(&v_len.to_le_bytes())?;
            }
            k.encode(encoder)?;
            v.encode(encoder)?;
        }
        Ok(())
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let decode_err = EINVAL;
        let mut offset = 0;
        let cap = usize::from_le_bytes(
            buf[offset..offset + USIZE_SIZE]
                .try_into()
                .map_err(|_| decode_err)?,
        );
        offset += USIZE_SIZE;
        let mut map = HashMap::<K, V>::new();
        if cap == 0 {
            return Ok(map);
        }
        let k_size = usize::from_le_bytes(
            buf[offset..offset + USIZE_SIZE]
                .try_into()
                .map_err(|_| decode_err)?,
        );
        offset += USIZE_SIZE;
        let v_size = usize::from_le_bytes(
            buf[offset..offset + USIZE_SIZE]
                .try_into()
                .map_err(|_| decode_err)?,
        );
        offset += USIZE_SIZE;
        for _ in 0..cap {
            let (key, value) = {
                let k = K::decode(&buf[offset..offset + k_size])?;
                offset += k_size;
                let v = V::decode(&buf[offset..offset + v_size])?;
                offset += v_size;
                (k, v)
            };
            map.insert(key, value);
        }
        Ok(map)
    }
}

impl Serialize for BitMap {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        // TODO: Fix this limitation
        assert!(self.len() % BITMAP_UNIT == 0);
        let bitvec = self.clone().into_vec();
        encoder.write_bytes(&bitvec.len().to_le_bytes())?;
        encoder.write_bytes(&bitvec)?;
        Ok(())
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let decode_err = EINVAL;
        let len = usize::from_le_bytes(buf[..USIZE_SIZE].try_into().map_err(|_| decode_err)?);
        Ok(BitMap::from_vec(
            buf[USIZE_SIZE..USIZE_SIZE + len]
                .try_into()
                .map_err(|_| decode_err)?,
        ))
    }

    fn bytes_len(&self) -> Option<usize> {
        Some(USIZE_SIZE + self.clone().into_vec().len())
    }
}

/// Default `Serialize` implementation based on `std::mem::transmute`.
#[macro_export]
macro_rules! impl_default_serialize {
    ($target_struct:ident, $struct_size:expr) => {
        impl $crate::Serialize for $target_struct {
            fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
                unsafe {
                    encoder.write_bytes(&std::mem::transmute_copy::<
                        $target_struct,
                        [u8; $struct_size],
                    >(self))
                }
            }

            fn decode(buf: &[u8]) -> Result<Self>
            where
                Self: Sized,
            {
                debug_assert!(buf.len() == $struct_size);
                let decode_err = EINVAL;
                unsafe {
                    Ok(std::mem::transmute::<[u8; $struct_size], $target_struct>(
                        buf.try_into().map_err(|_| decode_err)?,
                    ))
                }
            }

            fn bytes_len(&self) -> Option<usize> {
                Some($struct_size)
            }
        }
    };
}

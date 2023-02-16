use crate::utils::AsBuf;

use block_device::{Bid, BlockDeviceAsFile, BLOCK_SIZE};
use errno::prelude::*;
use std::mem::MaybeUninit;
use std::sync::Arc;

pub struct Storage {
    device: Arc<dyn BlockDeviceAsFile>,
}

impl Storage {
    pub fn new(device: Arc<dyn BlockDeviceAsFile>) -> Self {
        Self { device }
    }

    /// Load struct `T` from given block and offset in the device
    pub async fn load_struct<T: Sync + Send + AsBuf>(&self, id: Bid, offset: usize) -> Result<T> {
        let mut s: T = unsafe { MaybeUninit::uninit().assume_init() };
        let s_mut_buf = s.as_buf_mut();
        assert!(offset + s_mut_buf.len() <= BLOCK_SIZE);
        let device_offset = id.to_offset() + offset;
        let len = self.device.read(device_offset, s_mut_buf).await?;
        assert!(len == s_mut_buf.len());
        Ok(s)
    }

    /// Store struct `T` to given block and offset in the device
    pub async fn store_struct<T: Sync + Send + AsBuf>(
        &self,
        id: Bid,
        offset: usize,
        s: &T,
    ) -> Result<()> {
        let s_buf = s.as_buf();
        assert!(offset + s_buf.len() <= BLOCK_SIZE);
        let device_offset = id.to_offset() + offset;
        let len = self.device.write(device_offset, s_buf).await?;
        assert!(len == s_buf.len());
        Ok(())
    }

    /// Read blocks starting from the offset of block into the given buffer.
    pub async fn read_at(&self, id: Bid, buf: &mut [u8], offset: usize) -> Result<usize> {
        let device_offset = id.to_offset() + offset;
        self.device.read(device_offset, buf).await
    }

    /// Write buffer at the blocks starting from the offset of block.
    pub async fn write_at(&self, id: Bid, buf: &[u8], offset: usize) -> Result<usize> {
        let device_offset = id.to_offset() + offset;
        self.device.write(device_offset, buf).await
    }

    /// Commit all the data in device to underlying storage for durability.
    pub async fn sync(&self) -> Result<()> {
        self.device.sync().await
    }

    /// Flush the specified blocks(if they are cached) to device.
    pub async fn flush_blocks(&self, blocks: &[Bid]) -> Result<usize> {
        self.device.flush_blocks(blocks).await
    }
}

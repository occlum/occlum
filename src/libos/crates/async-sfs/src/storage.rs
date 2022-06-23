use crate::utils::AsBuf;

use block_device::{BlockDevice, BlockDeviceExt, BlockId, BLOCK_SIZE};
use errno::prelude::*;
use std::mem::MaybeUninit;
use std::sync::Arc;

pub struct Storage {
    device: Arc<dyn BlockDevice>,
}

impl Storage {
    pub fn new(device: Arc<dyn BlockDevice>) -> Self {
        Self { device }
    }

    /// Load struct `T` from given block and offset in the storage
    pub async fn load_struct<T: Sync + Send + AsBuf>(
        &self,
        id: BlockId,
        offset: usize,
    ) -> Result<T> {
        let mut s: T = unsafe { MaybeUninit::uninit().assume_init() };
        let s_mut_buf = s.as_buf_mut();
        assert!(offset + s_mut_buf.len() <= BLOCK_SIZE);
        let device_offset = id * BLOCK_SIZE + offset;
        let len = self.device.read(device_offset, s_mut_buf).await?;
        assert!(len == s_mut_buf.len());
        Ok(s)
    }

    /// Store struct `T` to given block and offset in the storage
    pub async fn store_struct<T: Sync + Send + AsBuf>(
        &self,
        id: BlockId,
        offset: usize,
        s: &T,
    ) -> Result<()> {
        let s_buf = s.as_buf();
        assert!(offset + s_buf.len() <= BLOCK_SIZE);
        let device_offset = id * BLOCK_SIZE + offset;
        let len = self.device.write(device_offset, s_buf).await?;
        assert!(len == s_buf.len());
        Ok(())
    }

    /// Read blocks starting from the offset of block into the given buffer.
    pub async fn read_at(&self, id: BlockId, buf: &mut [u8], offset: usize) -> Result<usize> {
        let device_offset = id * BLOCK_SIZE + offset;
        self.device.read(device_offset, buf).await
    }

    /// Write buffer at the blocks starting from the offset of block.
    pub async fn write_at(&self, id: BlockId, buf: &[u8], offset: usize) -> Result<usize> {
        let device_offset = id * BLOCK_SIZE + offset;
        self.device.write(device_offset, buf).await
    }

    /// Flush the buffer.
    pub async fn flush(&self) -> Result<()> {
        self.device.flush().await
    }

    // pub fn as_page_cache(&self) -> Option<&CachedDisk<FSPageAlloc>> {
    //     match self {
    //         Self::PageCache(cache) => Some(cache),
    //         _ => None,
    //     }
    // }
}

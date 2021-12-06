/// An extension trait for block devices to support file-like I/O operations.
/// This trait is mostly for the testing purpose.
#[async_trait]
pub trait BlockDeviceExt {
    /// Read a specified number of bytes from a byte offset in the device.
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<()>;

    /// Write a specified number of bytes to a byte offset in the device.
    async fn write(&self, offset: usize, buf: &[u8]) -> Result<()>;

    /// Flush all cached data in the device to the storage medium for durability.
    async fn flush(&self) -> Result<()>;
}

impl<B: BlockDevice> BlockDeviceExt for B {
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        // Check offset and buf size
        let len = self.total_bytes();
        if offset >= len {
            return Ok(0);
        }
        let buf_len = buf.len().min(len - offset);
        let buf = &mut buf[..buf_len];
        if buf.len() == 0 {
            return Ok(0);
        }

        todo!()
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> Result<()> {
        todo!()
    }

    async fn flush(&self) -> Result<()> {
        todo!()
    }
}

use super::*;

#[derive(Debug)]
pub struct DevNull;

impl File for DevNull {
    fn write(&self, _buf: &[u8]) -> Result<usize> {
        Ok(_buf.len())
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize> {
        Ok(_buf.len())
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        Ok(bufs.iter().map(|buf| buf.len()).sum())
    }

    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EINVAL, "device not support reads")
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EINVAL, "device not support reads")
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        return_errno!(EINVAL, "device not support reads")
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_errno!(EINVAL, "device not support seeks")
    }

    fn metadata(&self) -> Result<Metadata> {
        unimplemented!()
    }

    fn set_len(&self, len: u64) -> Result<()> {
        return_errno!(EINVAL, "device not support resizing")
    }

    fn sync_all(&self) -> Result<()> {
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        Ok(())
    }

    fn read_entry(&self) -> Result<String> {
        return_errno!(ENOTDIR, "device is not a directory")
    }

    fn as_any(&self) -> &Any {
        self
    }
}

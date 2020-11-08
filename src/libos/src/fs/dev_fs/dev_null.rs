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

    fn poll_new(&self) -> IoEvents {
        IoEvents::IN
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

use super::*;

#[derive(Debug)]
pub struct DevZero;

impl File for DevZero {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        for b in _buf.iter_mut() {
            *b = 0;
        }
        Ok(_buf.len())
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        self.read(_buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut total_nbytes = 0;
        for buf in bufs {
            total_nbytes += self.read(buf)?;
        }
        Ok(total_nbytes)
    }

    fn poll_new(&self) -> IoEvents {
        IoEvents::IN
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

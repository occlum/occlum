use super::*;

#[derive(Debug)]
pub struct NullFile;

impl File for NullFile {
    fn read(&self, _buf: &mut [u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, Error> {
        Ok(0)
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        unimplemented!()
    }

    fn metadata(&self) -> Result<Metadata, Error> {
        unimplemented!()
    }

    fn set_len(&self, len: u64) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_all(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_data(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn read_entry(&self) -> Result<String, Error> {
        unimplemented!()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

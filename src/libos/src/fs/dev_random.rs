use super::*;

#[derive(Debug)]
pub struct DevRandom;

extern "C" {
    fn sgx_read_rand(rand_buf: *mut u8, buf_size: usize) -> sgx_status_t;
}

impl File for DevRandom {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        let buf = _buf.as_mut_ptr();
        let size = _buf.len();
        let status = unsafe { sgx_read_rand(buf, size) };
        if status != sgx_status_t::SGX_SUCCESS {
            return_errno!(EAGAIN, "failed to get random number from sgx");
        }
        Ok(size)
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        self.read(_buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut total_nbytes = 0;
        for buf in bufs {
            match self.read(buf) {
                Ok(this_nbytes) => {
                    total_nbytes += this_nbytes;
                }
                Err(e) => {
                    if total_nbytes > 0 {
                        break;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Ok(total_nbytes)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize> {
        return_errno!(EINVAL, "device not support writes")
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize> {
        return_errno!(EINVAL, "device not support writes")
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        return_errno!(EINVAL, "device not support writes")
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

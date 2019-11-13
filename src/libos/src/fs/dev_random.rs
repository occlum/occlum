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

    fn as_any(&self) -> &Any {
        self
    }
}

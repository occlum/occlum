use super::*;
use crate::net::PollEventFlags;

#[derive(Debug)]
pub struct DevRandom;

extern "C" {
    fn sgx_read_rand(rand_buf: *mut u8, buf_size: usize) -> sgx_status_t;
}

impl File for DevRandom {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        let (buf, size) = _buf.as_mut().as_mut_ptr_and_len();
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

    fn metadata(&self) -> Result<Metadata> {
        Ok(Metadata {
            dev: 0,
            inode: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: FileType::CharDevice,
            mode: (FileMode::S_IRUSR | FileMode::S_IRGRP | FileMode::S_IROTH).bits(),
            nlinks: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn poll(&self) -> Result<(PollEventFlags)> {
        Ok(PollEventFlags::POLLIN)
    }

    fn poll_new(&self) -> IoEvents {
        IoEvents::IN
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub trait AsDevRandom {
    fn as_dev_random(&self) -> Result<&DevRandom>;
}

impl AsDevRandom for FileRef {
    fn as_dev_random(&self) -> Result<&DevRandom> {
        self.as_any()
            .downcast_ref::<DevRandom>()
            .ok_or_else(|| errno!(EBADF, "not random device"))
    }
}

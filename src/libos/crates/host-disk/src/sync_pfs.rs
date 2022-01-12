use block_device::{BioReq, BioSubmission, BioType, BlockDevice};
use std::io::prelude::*;
use std::io::{IoSlice, IoSliceMut, SeekFrom};
use std::path::{Path, PathBuf};
use std::sgxfs::{OpenOptions, SgxFile};

use crate::prelude::*;

#[derive(Clone)]
pub struct LockedFile(Arc<Mutex<SgxFile>>);

unsafe impl Send for LockedFile {}
unsafe impl Sync for LockedFile {}

/// A type of host disk that implements a block device interface by performing
/// normal synchronous I/O to the underlying host file.
///
/// `SyncIoDisk` implements the interface of `BlockDevice`. Although the
/// interface is asynchronous, the implementation uses the normal synchronous
/// `read` and `write` system calls to perform I/O. Thus, the performance of
/// `SyncIoDisk` is not good. This is especially true for SGX where issuing
/// system calls from the enclave triggers enclave switching, which is costly.
///
/// It is recommended to use `IoUringDisk` for an optimal performance.
pub struct SyncSgxDisk {
    file: LockedFile,
    path: PathBuf,
    total_blocks: usize,
    can_read: bool,
    can_write: bool,
}

impl SyncSgxDisk {
    pub fn new(path: &Path, total_blocks: usize) -> Result<SyncSgxDisk> {
        let options = {
            let mut options = OpenOptions::new();
            options.write(true).update(true);
            options
        };
        let mut file = options.open(path)?;

        // The set_len() is unsupported for SgxFile, we have to
        // implement it in a slow way by padding null bytes.
        static ZEROS: [u8; 0x1000] = [0; 0x1000];
        let mut remaining_len = total_blocks * BLOCK_SIZE;
        while remaining_len != 0 {
            let l = remaining_len.min(0x1000);
            let len = file.write(&ZEROS[..l])?;
            remaining_len -= len;
        }
        file.flush()?;
        file.seek(SeekFrom::Start(0))?;

        let disk = Self {
            file: LockedFile(Arc::new(Mutex::new(file))),
            path: path.to_path_buf(),
            total_blocks,
            can_read: true,
            can_write: true,
        };
        Ok(disk)
    }

    fn do_read(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_read {
            return Err(errno!(EACCES, "read is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;

        let mut file = self.file.0.lock().unwrap();
        file.seek(SeekFrom::Start(offset as u64))?;
        let read_len = req.access_mut_bufs_with(|bufs| {
            let mut slices: Vec<IoSliceMut<'_>> = bufs
                .iter_mut()
                .map(|buf| IoSliceMut::new(buf.as_slice_mut()))
                .collect();

            file.read_vectored(&mut slices)
        })?;

        drop(file);

        debug_assert!(read_len / BLOCK_SIZE == req.num_blocks());
        Ok(())
    }

    fn do_write(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "write is not allowed"));
        }
        let (offset, _) = self.get_range_in_bytes(&req)?;

        let mut file = self.file.0.lock().unwrap();

        file.seek(SeekFrom::Start(offset as u64))?;
        const IOV_MAX_IN_LINUX: usize = 1024;

        let write_len = req.access_bufs_with(|bufs| {
            let writev_times = bufs.len() / IOV_MAX_IN_LINUX;
            let rem_len = bufs.len() % IOV_MAX_IN_LINUX;
            let mut total_write_len = 0;
            let mut idx = 0;

            while idx < writev_times {
                let slices: Vec<IoSlice<'_>> = bufs
                    [idx * IOV_MAX_IN_LINUX..(idx + 1) * IOV_MAX_IN_LINUX]
                    .iter()
                    .map(|buf| IoSlice::new(buf.as_slice()))
                    .collect();
                total_write_len += file.write_vectored(&slices).unwrap();
                idx += 1;
            }

            if rem_len > 0 {
                let slices: Vec<IoSlice<'_>> = bufs
                    [writev_times * IOV_MAX_IN_LINUX..writev_times * IOV_MAX_IN_LINUX + rem_len]
                    .iter()
                    .map(|buf| IoSlice::new(buf.as_slice()))
                    .collect();
                total_write_len += file.write_vectored(&slices).unwrap();
            }
            total_write_len
        });
        drop(file);

        debug_assert!(write_len / BLOCK_SIZE == req.num_blocks());
        Ok(())
    }

    fn do_flush(&self) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "flush is not allowed"));
        }

        let mut file = self.file.0.lock().unwrap();
        file.flush()?;
        //file.sync_all()?;
        drop(file);

        Ok(())
    }

    fn get_range_in_bytes(&self, req: &Arc<BioReq>) -> Result<(usize, usize)> {
        let begin_block = req.addr();
        let end_block = begin_block + req.num_blocks();
        if end_block > self.total_blocks {
            return Err(errno!(EINVAL, "invalid block range"));
        }
        let begin_offset = begin_block * BLOCK_SIZE;
        let end_offset = end_block * BLOCK_SIZE;
        Ok((begin_offset, end_offset))
    }
}

impl BlockDevice for SyncSgxDisk {
    fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        // Update the status of req to submittted
        let submission = BioSubmission::new(req);

        let req = submission.req();
        let type_ = req.type_();
        let res = match type_ {
            BioType::Read => self.do_read(req),
            BioType::Write => self.do_write(req),
            BioType::Flush => self.do_flush(),
        };

        // Update the status of req to completed and set the response
        let resp = res.map_err(|e| e.errno());
        unsafe {
            req.complete(resp);
        }

        submission
    }
}

impl Drop for SyncSgxDisk {
    fn drop(&mut self) {
        // Ensure all data are peristed before the disk is dropped
        let _ = self.do_flush();
    }
}

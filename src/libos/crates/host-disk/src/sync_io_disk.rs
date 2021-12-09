use block_device::{BioReq, BioSubmission, BioType, BlockDevice};
use std::fs::File;
use std::io::prelude::*;
use std::io::{IoSlice, IoSliceMut, SeekFrom};

use crate::prelude::*;
use crate::{HostDisk, OpenOptions};

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
pub struct SyncIoDisk {
    file: Mutex<File>,
    total_blocks: usize,
    can_read: bool,
    can_write: bool,
}

impl SyncIoDisk {
    fn read(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_read {
            return Err(errno!(EACCES, "read is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;

        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset as u64))?;
        let read_len = req.access_mut_bufs_with(|bufs| {
            let mut slices: Vec<IoSliceMut<'_>> = bufs
                .iter_mut()
                .map(|buf| IoSliceMut::new(buf.as_slice_mut()))
                .collect();
            file.read_vectored(&mut slices)
        })?;
        drop(file);

        assert!(read_len / BLOCK_SIZE == req.num_bufs());
        Ok(())
    }

    fn write(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "write is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;

        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset as u64))?;
        let write_len = req.access_bufs_with(|bufs| {
            let slices: Vec<IoSlice<'_>> = bufs
                .iter()
                .map(|buf| IoSlice::new(buf.as_slice()))
                .collect();
            file.write_vectored(&slices)
        })?;
        drop(file);

        assert!(write_len / BLOCK_SIZE == req.num_bufs());
        Ok(())
    }

    fn flush(&self, _req: &Arc<BioReq>) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "flush is not allowed"));
        }

        let mut file = self.file.lock().unwrap();
        file.flush()?;
        drop(file);

        Ok(())
    }

    fn get_range_in_bytes(&self, req: &Arc<BioReq>) -> Result<(usize, usize)> {
        let begin_block = req.addr();
        let end_block = begin_block + req.num_bufs();
        if end_block > self.total_blocks {
            return Err(errno!(EINVAL, "invalid block range"));
        }
        let begin_offset = begin_block * BLOCK_SIZE;
        let end_offset = end_block * BLOCK_SIZE;
        Ok((begin_offset, end_offset))
    }
}

impl BlockDevice for SyncIoDisk {
    fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        // Update the status of req to submittted
        let submission = BioSubmission::new(req);

        let req = submission.req();
        let type_ = req.type_();
        let res = match type_ {
            BioType::Read => self.read(req),
            BioType::Write => self.write(req),
            BioType::Flush => self.flush(req),
        };

        // Update the status of req to completed and set the response
        let resp = res.map_err(|e| e.errno());
        unsafe {
            req.complete(resp);
        }

        submission
    }
}

impl HostDisk for SyncIoDisk {
    fn from_options_and_file(options: &OpenOptions<Self>, file: File) -> Result<Self> {
        let total_blocks = options.total_blocks.unwrap_or_else(|| {
            let file_len = file.metadata().unwrap().len() as usize;
            assert!(file_len >= BLOCK_SIZE);
            file_len / BLOCK_SIZE
        });
        let can_read = options.read;
        let can_write = options.write;
        let new_self = Self {
            file: Mutex::new(file),
            total_blocks,
            can_read,
            can_write,
        };
        Ok(new_self)
    }
}

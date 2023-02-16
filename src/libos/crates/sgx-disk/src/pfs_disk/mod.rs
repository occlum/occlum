use std::fmt;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sgxfs::SgxFile as PfsFile;

use block_device::{BioReq, BioSubmission, BioType, BlockDevice};

pub use self::open_options::OpenOptions;
use crate::prelude::*;

mod open_options;

/// A virtual disk backed by a protected file of Intel SGX Protected File
/// System Library (SGX-PFS).
///
/// This type of disks is considered (relatively) secure.
pub struct PfsDisk {
    file: Mutex<PfsFile>,
    path: PathBuf,
    total_blocks: usize,
    can_read: bool,
    can_write: bool,
}

// Safety. PfsFile does not implement Send, but it is safe to do so.
unsafe impl Send for PfsDisk {}
// Safety. PfsFile does not implement Sync but it is safe to do so.
unsafe impl Sync for PfsDisk {}

// The first 3KB file data of PFS are stored in the metadata node. All remaining
// file data are stored in nodes of 4KB. We need to consider this internal
// offset so that our block I/O are aligned with the PFS internal node boundaries.
const PFS_INNER_OFFSET: usize = 3 * 1024;

impl PfsDisk {
    /// Open a disk backed by an existing PFS file on the host.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        OpenOptions::new().read(true).write(true).open(path)
    }

    /// Open a disk by opening or creating a PFS file on the give path.
    pub fn create<P: AsRef<Path>>(path: P, total_blocks: usize) -> Result<Self> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .total_blocks(total_blocks)
            .open(path)
    }

    /// Returns the PFS file on the host Linux.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn do_read(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_read {
            return Err(errno!(EACCES, "read is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;
        let offset = offset + PFS_INNER_OFFSET;

        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset as u64)).unwrap();
        req.access_mut_bufs_with(|bufs| {
            // We do not use read_vectored. This is because PfsFile does not give
            // a specialized implementation that offers a performance advantage.
            for buf in bufs {
                let read_len = file.read(buf.as_slice_mut()).unwrap();
                debug_assert!(read_len == buf.len());
            }
        });
        drop(file);

        Ok(())
    }

    fn do_write(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "write is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;
        let offset = offset + PFS_INNER_OFFSET;

        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset as u64)).unwrap();
        req.access_bufs_with(|bufs| {
            // We do not use read_vectored. This is because PfsFile does not give
            // a specialized implementation that offers a performance advantage.
            for buf in bufs {
                let write_len = file.write(buf.as_slice()).unwrap();
                debug_assert!(write_len == buf.len());
            }
        });
        drop(file);

        Ok(())
    }

    fn do_flush(&self) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "flush is not allowed"));
        }

        let mut file = self.file.lock().unwrap();
        file.flush().unwrap();
        // TODO: sync
        // file.sync_data()?;
        drop(file);

        Ok(())
    }

    fn get_range_in_bytes(&self, req: &Arc<BioReq>) -> Result<(usize, usize)> {
        let begin_block = req.addr().to_raw() as usize;
        let end_block = begin_block + req.num_blocks();
        if end_block > self.total_blocks {
            return Err(errno!(EINVAL, "invalid block range"));
        }
        let begin_offset = begin_block * BLOCK_SIZE;
        let end_offset = end_block * BLOCK_SIZE;
        Ok((begin_offset, end_offset))
    }
}

impl BlockDevice for PfsDisk {
    fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        // Update the status of req to submitted
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

impl Drop for PfsDisk {
    fn drop(&mut self) {
        let mut file = self.file.lock().unwrap();
        file.flush().unwrap();
        // TODO: sync
        // file.sync_all()?;
    }
}

impl fmt::Debug for PfsDisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PfsDisk")
            .field("path", &self.path)
            .field("total_blocks", &self.total_blocks)
            .finish()
    }
}

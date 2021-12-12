use crate::prelude::*;

/// An in-memory disk.
pub struct MemDisk {
    total_blocks: usize,
    disk: Mutex<Box<[u8]>>,
}

impl MemDisk {
    /// Create an in-memory disk with a fixed capability.
    pub fn new(total_blocks: usize) -> Result<Self> {
        let disk = {
            let total_bytes = total_blocks * crate::BLOCK_SIZE;
            let disk = Box::<[u8]>::new_zeroed_slice(total_bytes);
            unsafe { disk.assume_init() }
        };
        Ok(Self {
            total_blocks,
            disk: Mutex::new(disk),
        })
    }
}

impl MemDisk {
    fn read(&self, req: &Arc<BioReq>) -> Result<()> {
        let (begin_offset, _end_offset) = self.get_range_in_bytes(&req)?;

        let disk = self.disk.lock();
        let mut offset = begin_offset;
        req.access_mut_bufs_with(|bufs| {
            for buf in bufs.iter_mut() {
                let buf_len = buf.len();
                buf.as_slice_mut()
                    .copy_from_slice(&disk[offset..offset + buf_len]);
                offset += buf.len();
            }
        });
        drop(disk);

        Ok(())
    }

    fn write(&self, req: &Arc<BioReq>) -> Result<()> {
        let (begin_offset, _end_offset) = self.get_range_in_bytes(&req)?;

        let mut disk = self.disk.lock();
        let mut offset = begin_offset;
        req.access_bufs_with(|bufs| {
            for buf in bufs.iter() {
                disk[offset..offset + buf.len()].copy_from_slice(buf.as_slice());
                offset += buf.len();
            }
        });
        drop(disk);

        Ok(())
    }

    fn flush(&self, _req: &Arc<BioReq>) -> Result<()> {
        // Do nothing
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

impl BlockDevice for MemDisk {
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

#[cfg(test)]
mod test {
    use super::*;

    fn test_setup() -> MemDisk {
        let total_blocks = 16;
        MemDisk::new(total_blocks).unwrap()
    }

    fn test_teardown(disk: MemDisk) {
        drop(disk);
    }

    crate::gen_unit_tests!(test_setup, test_teardown);
}

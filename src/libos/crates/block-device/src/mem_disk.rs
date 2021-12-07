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
                buf.as_slice_mut()
                    .copy_from_slice(&disk[offset..offset + BLOCK_SIZE]);
                offset += BLOCK_SIZE;
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
                disk[offset..offset + BLOCK_SIZE].copy_from_slice(buf.as_slice());
                offset += BLOCK_SIZE;
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
        let end_block = begin_block + req.num_bufs();
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
    use self::helper::check_disk_filled_with_val;
    use super::*;

    #[test]
    fn check_zeroed() {
        async_rt::task::block_on(async move {
            // Create a small MemDisk
            let mem_disk = {
                let total_blocks = 16;
                MemDisk::new(total_blocks).unwrap()
            };

            // MemDisk should be filled with zeros
            check_disk_filled_with_val(&mem_disk, 0).await.unwrap();
        });
    }

    #[test]
    fn write_all() {
        async_rt::task::block_on(async move {
            // Create a small MemDisk
            let mem_disk = {
                let total_blocks = 16;
                MemDisk::new(total_blocks).unwrap()
            };
            let val = 39_u8;

            // Send a write that fills all blocks with a single byte
            let bufs = (0..mem_disk.total_blocks)
                .map(|addr| {
                    let mut boxed_slice =
                        unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                    for b in boxed_slice.iter_mut() {
                        *b = val;
                    }
                    let buf = BlockBuf::from_boxed(boxed_slice);
                    buf
                })
                .collect();
            let req = BioReq::new_write(0, bufs, None).unwrap();
            let submission = mem_disk.submit(Arc::new(req));
            submission.complete().await;

            // MemDisk should be filled with the value
            check_disk_filled_with_val(&mem_disk, val).await.unwrap();
        });
    }

    mod helper {
        use super::*;

        /// Check whether a disk is filled with a given byte value.
        pub async fn check_disk_filled_with_val(disk: &dyn BlockDevice, val: u8) -> Result<()> {
            // Initiate multiple reads, each of which reads just one block
            let reads: Vec<_> = (0..disk.total_blocks())
                .map(|addr| {
                    let bufs = {
                        let boxed_slice =
                            unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                        let buf = BlockBuf::from_boxed(boxed_slice);
                        vec![buf]
                    };
                    let callback: BioCompletionCallback = Box::new(|req, resp| {
                        assert!(resp == Ok(()));
                    }
                        as _);
                    let req = BioReq::new_read(addr, bufs, Some(callback)).unwrap();
                    disk.submit(Arc::new(req))
                })
                .collect();

            // Wait for reads to complete and check bytes
            for read in reads {
                let req = read.complete().await;

                let mut bufs = req.take_bufs();
                for buf in bufs.drain(..) {
                    // Check if any byte read does not equal to the value
                    if buf.as_slice().iter().any(|b| *b != val) {
                        return Err(errno!(EINVAL, "found unexpected byte"));
                    }

                    // Safety. It is safe to drop the memory of buffers here
                    drop(unsafe { BlockBuf::into_boxed(buf) });
                }
            }
            Ok(())
        }
    }
}

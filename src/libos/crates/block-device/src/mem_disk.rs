/// An in-memory disk.
pub struct MemDisk {
    total_blocks: usize,
    disk: Mutex<Box<[u8]>>,
}

impl MemDisk {
    pub fn new(total_blocks: usize) -> Result<Self> {
        let disk = {
            let total_bytes = total_blocks * BlockBuf::SIZE;
            let disk = Box::<[u32]>::new_zeroed_slice(total_bytes);
            unsafe {
                disk.assume_init()
            }
        };
        Self {
            total_blocks,
            disk: Mutex::new(disk),
        } 
    }
}

impl MemDisk {
    fn read(&self, req: Arc<BlockReq>) -> Submission {
        let res = self.do_read(&req);
        req.on_complete(res);
        Submission::new(req)
    }

    fn do_read(&self, req: &Arc<BlockReq>) -> Result<()> {
        let (begin_offset, end_offset) = self.get_range_in_bytes(&req)?;

        let disk = self.disk.lock();
        let data = &disk[begin_offset..end_offset];
        for (block_i, block) in req.blocks_mut().enumerate() {
            let offset = block_i * Block::SIZE; 
            block.as_slice_mut().copy_from_slice(&disk[offset..offset + Block::SIZE])
        }
        drop(disk);

        Ok(())
    }

    fn get_range_in_bytes(&self, req: &Arc<BlockReq>) -> Result<(usize, usize)> {
        let begin_block = req.block_id;
        let end_block = begin_block + req.num_blocks;
        if end_block > self.total_blocks {
            return Err(errno!(EINVAl, "invalid block range")); 
        }
        let begin_offset = begin_block * Block::SIZE;
        let end_offset = end_block * Block::SIZE;
        Ok((begin_offset, end_offset))
    }

    fn write(&self, req: Arc<BlockReq>) -> Submission {
        let res = self.do_write(&req);
        unsafe {
            req.complete(res);
        }
        Submission::new(req)
    }

    fn do_write(&self, req: &Arc<BlockReq>) -> Result<()> {
        let (begin_offset, end_offset) = self.get_range_in_bytes(&req)?;

        let mut disk = self.disk.lock();
        let data = &mut disk[begin_offset..end_offset];
        for (block_i, block) in req.blocks().enumerate() {
            let offset = block_i * Block::SIZE; 
            data[offset..offset + Block::SIZE].copy_from_slice(block.as_slice());
        }
        drop(disk);

        Ok(())
    }

    fn flush(&self, req: Arc<BlockReq>) -> Submission {
        req.on_complete(Ok(()));
        Submission::new(req) 
    }
}

impl BlockDevice for MemDisk {
    fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    fn submit(&self, req: Arc<BlockReq>) -> Result<Submission> {
        let type_ = req.type_();
        match type_ {
            BlockReqType::Read => {
                self.read(req)
            }
            BlockReqType::Write => {
                self.write(req)
            }
            BlockReqType::Flush => {
                self.flush(req)
            }
        } 
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::BlockDeviceExt;

    #[test]
    fn read_write() {
        async_rt::task::block_on({
            let total_blocks = 16;
            let mem_disk = MemDisk::new(total_blocks);
            let offset = 1234;
            let msg = "HelloWorld";

            mem_disk.write(offset, msg.as_bytes()).await;

            let mut buf = Box::<[u32]>::new_zeroed_slice(msg.as_bytes().len());
            mem_disk.read(offset, &mut buf).await;
            assert!(&buf == msg.as_bytes());
        });
    }

    #[test]
    fn parallel_write() {
        async_rt::task::block_on({
            let total_blocks = 16;
            let mem_disk = MemDisk::new(total_blocks);
            let submissions: Vec<Submission> = (0..10).map(|_| {
                let block_req = Arc::new(BlockReq::new_write(..., ..., ...));
                let submission = mem_disk.submit(block_req.clone());
                submission
            }).collect();
        });
    }
}

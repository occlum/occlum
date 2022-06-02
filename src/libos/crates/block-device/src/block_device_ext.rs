use async_trait::async_trait;
use core::ptr::NonNull;

use crate::prelude::*;
use crate::util::{align_down, align_up};

/// An extension trait for block devices to support file-like I/O operations.
/// This trait can be convenient when a block device should behave like a file.
#[async_trait]
pub trait BlockDeviceExt {
    /// Read a specified number of bytes at a byte offset on the device.
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;

    /// Write a specified number of bytes at a byte offset on the device.
    async fn write(&self, offset: usize, buf: &[u8]) -> Result<usize>;

    /// Flush all cached data in the device to the storage medium for durability.
    async fn flush(&self) -> Result<()>;
}

#[async_trait]
impl BlockDeviceExt for dyn BlockDevice {
    async fn read(&self, offset: usize, read_buf: &mut [u8]) -> Result<usize> {
        Impl::new(self).read(offset, read_buf).await
    }

    async fn write(&self, offset: usize, write_buf: &[u8]) -> Result<usize> {
        Impl::new(self).write(offset, write_buf).await
    }

    async fn flush(&self) -> Result<()> {
        Impl::new(self).flush().await
    }
}

#[async_trait]
impl<B: BlockDevice> BlockDeviceExt for B {
    async fn read(&self, offset: usize, read_buf: &mut [u8]) -> Result<usize> {
        Impl::new(self).read(offset, read_buf).await
    }

    async fn write(&self, offset: usize, write_buf: &[u8]) -> Result<usize> {
        Impl::new(self).write(offset, write_buf).await
    }

    async fn flush(&self) -> Result<()> {
        Impl::new(self).flush().await
    }
}

// TODO: The following implementation does not gurantee the atomicity of concurrent
// reads and writes when their offsets or lengths are not block aligned.
// Is this a problem? Should the interface promise such properties?
//
// The atomicity of block-aligned reads and writes are determined by the block
// device.

// We cannot add private methods to trait (e.g., BlockDeviceExt). So the Impl
// struct is introduced as a zero-cost means to workaround the limitation.
struct Impl<'a> {
    disk: &'a dyn BlockDevice,
}

impl<'a> Impl<'a> {
    pub fn new(disk: &'a dyn BlockDevice) -> Self {
        Self { disk }
    }

    pub async fn read(&self, offset: usize, read_buf: &mut [u8]) -> Result<usize> {
        if offset > isize::MAX as usize {
            return Err(errno!(EINVAL, "offset too large"));
        }

        // Block devices do not support "short reads". So we need to make sure
        // that all requested bytes are within the capability of the block device.
        let total_bytes = self.disk.total_blocks() * BLOCK_SIZE;
        let read_buf = if offset + read_buf.len() <= total_bytes {
            read_buf
        } else {
            &mut read_buf[..total_bytes - offset]
        };

        if read_buf.len() == 0 {
            return Ok(0);
        }

        if Self::cover_one_partial_block(offset, read_buf.len()) {
            self.do_read_one_partial_block(offset, read_buf).await
        } else {
            self.do_read_general(offset, read_buf).await
        }
    }

    async fn do_read_one_partial_block(&self, offset: usize, read_buf: &mut [u8]) -> Result<usize> {
        let read_buf_len = read_buf.len();
        debug_assert!(read_buf_len < BLOCK_SIZE);

        let bufs = {
            // Safety. It is ok for read buffers to have uninit data.
            let boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
            let buf = BlockBuf::from_boxed(boxed_slice);
            vec![buf]
        };
        let req = BioReqBuilder::new(BioType::Read)
            .addr(offset / BLOCK_SIZE)
            .bufs(bufs)
            .on_drop(|_req: &BioReq, mut bufs: Vec<BlockBuf>| {
                let buf = bufs.remove(0);
                // Safety. This block buffer must be created from a boxed slice
                drop(unsafe { BlockBuf::into_boxed(buf) });
            })
            .build();
        let submission = self.disk.submit(Arc::new(req));
        let req = submission.complete().await;
        let res = req.response().unwrap();

        if let Err(e) = res {
            return Err(errno!(e.errno(), "read on a block device failed"));
        }

        // Copy back the partial blocks
        req.access_bufs_with(|bufs| {
            let one_block = bufs[0].as_slice();
            let in_block_offset = offset % BLOCK_SIZE;
            let src_buf = &one_block[in_block_offset..in_block_offset + read_buf_len];
            read_buf.copy_from_slice(src_buf);
        });
        Ok(read_buf_len)
    }

    async fn do_read_general(&self, offset: usize, read_buf: &mut [u8]) -> Result<usize> {
        let read_buf_len = read_buf.len();

        // Block devices can only fullfil requests for whole blocks. We need
        // to consider whether the requested byte range covers partial blocks.
        let is_first_block_partial = offset % BLOCK_SIZE != 0;
        let first_partial_block_len = if is_first_block_partial {
            BLOCK_SIZE - (offset % BLOCK_SIZE)
        } else {
            0
        };
        let is_last_block_partial = (offset + read_buf_len) % BLOCK_SIZE != 0;
        let last_partial_block_len = if is_last_block_partial {
            (offset + read_buf_len) % BLOCK_SIZE
        } else {
            0
        };

        // Block buffers for read
        let bufs = {
            // Construct the buffers for the request in three steps
            let mut bufs = Vec::new();
            // Step 1
            if is_first_block_partial {
                // Safety. It is ok for read buffers to have uninit data.
                let boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                let buf = BlockBuf::from_boxed(boxed_slice);
                bufs.push(buf);
            }
            // Step 2
            let whole_block_buf =
                &mut read_buf[first_partial_block_len..read_buf_len - last_partial_block_len];
            if whole_block_buf.len() > 0 {
                let buf = unsafe {
                    // Safety. The pointer of a slice must not be null.
                    let ptr = NonNull::new_unchecked(whole_block_buf.as_mut_ptr());
                    let len = whole_block_buf.len();
                    // Safety. The memory refered to by the pair of pointer and
                    // length is valid during the entire life cyle of the request.
                    BlockBuf::from_raw_parts(ptr, len)
                };
                bufs.push(buf);
            }
            // Step 3
            if is_last_block_partial {
                // Safety. It is ok for read buffers to have uninit data.
                let boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                let buf = BlockBuf::from_boxed(boxed_slice);
                bufs.push(buf);
            }
            bufs
        };

        // We keep associate the read request with some extra info
        #[derive(Copy, Clone, Debug)]
        struct ExtraInfo {
            is_first_block_partial: bool,
            is_last_block_partial: bool,
        }
        let extra_info = ExtraInfo {
            is_first_block_partial,
            is_last_block_partial,
        };

        // Finally, we are ready to construct the read request
        let req = BioReqBuilder::new(BioType::Read)
            .addr(offset / BLOCK_SIZE)
            .bufs(bufs)
            .ext(extra_info)
            // When the request is to be dropped, we free the two boxed slices
            // that we may have allocated for the first and last partial blocks.
            .on_drop(|req: &BioReq, mut bufs: Vec<BlockBuf>| {
                let ExtraInfo {
                    is_first_block_partial,
                    is_last_block_partial,
                } = req.ext().get::<ExtraInfo>().unwrap().clone();

                if is_last_block_partial {
                    let buf = bufs.remove(bufs.len() - 1);
                    // Safety. This block buffer must be created from a boxed slice
                    drop(unsafe { BlockBuf::into_boxed(buf) });
                }
                if is_first_block_partial {
                    let buf = bufs.remove(0);
                    // Safety. This block buffer must be created from a boxed slice
                    drop(unsafe { BlockBuf::into_boxed(buf) });
                }
            })
            .build();
        let submission = self.disk.submit(Arc::new(req));
        let req = submission.complete().await;
        let res = req.response().unwrap();

        if let Err(e) = res {
            return Err(errno!(e.errno(), "read on a block device failed"));
        }

        // Copy back the partial blocks
        req.access_bufs_with(|bufs| {
            if is_first_block_partial {
                let dst_buf = &mut read_buf[..first_partial_block_len];
                let first_block = bufs[0].as_slice();
                let src_buf = &first_block[BLOCK_SIZE - first_partial_block_len..];
                dst_buf.copy_from_slice(src_buf);
            }
            if is_last_block_partial {
                let dst_buf = &mut read_buf[read_buf_len - last_partial_block_len..];
                let last_block = bufs[bufs.len() - 1].as_slice();
                let src_buf = &last_block[..dst_buf.len()];
                dst_buf.copy_from_slice(src_buf);
            }
        });
        Ok(read_buf_len)
    }

    pub async fn write(&self, offset: usize, write_buf: &[u8]) -> Result<usize> {
        if offset > isize::MAX as usize {
            return Err(errno!(EINVAL, "offset too large"));
        }

        // Block devices do not support "short writes". So we need to make sure
        // that all requested bytes are within the capability of the block device.
        let total_bytes = self.disk.total_blocks() * BLOCK_SIZE;
        let write_buf = if offset + write_buf.len() <= total_bytes {
            write_buf
        } else {
            &write_buf[..total_bytes - offset]
        };

        let write_buf_len = write_buf.len();
        if write_buf_len == 0 {
            return Ok(0);
        }

        if Self::cover_one_partial_block(offset, write_buf.len()) {
            self.do_write_one_partial_block(offset, write_buf).await
        } else {
            self.do_write_general(offset, write_buf).await
        }
    }

    async fn do_write_one_partial_block(&self, offset: usize, write_buf: &[u8]) -> Result<usize> {
        let write_buf_len = write_buf.len();
        debug_assert!(write_buf_len < BLOCK_SIZE);

        let bufs = {
            // Safety. It is ok to create an uninit slice as we will fill it
            // with valid data immediately after the creation.
            let mut boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
            self.read(align_down(offset, BLOCK_SIZE), &mut boxed_slice)
                .await?;

            // Update the part of the block given by the write buffer
            let in_block_offset = offset % BLOCK_SIZE;
            let dst_buf = &mut boxed_slice[in_block_offset..in_block_offset + write_buf_len];
            dst_buf.copy_from_slice(write_buf);

            let buf = BlockBuf::from_boxed(boxed_slice);
            vec![buf]
        };
        let req = BioReqBuilder::new(BioType::Write)
            .addr(offset / BLOCK_SIZE)
            .bufs(bufs)
            .on_drop(|_req: &BioReq, mut bufs: Vec<BlockBuf>| {
                let buf = bufs.remove(0);
                // Safety. This block buffer must be created from a boxed slice
                drop(unsafe { BlockBuf::into_boxed(buf) });
            })
            .build();
        let submission = self.disk.submit(Arc::new(req));
        let req = submission.complete().await;
        let res = req.response().unwrap();

        if let Err(e) = res {
            return Err(errno!(e.errno(), "write on a block device failed"));
        }
        Ok(write_buf_len)
    }

    async fn do_write_general(&self, offset: usize, write_buf: &[u8]) -> Result<usize> {
        let write_buf_len = write_buf.len();

        // Block devices can only fullfil requests for whole blocks. We need
        // to consider whether the requested byte range covers partial blocks.
        let is_first_block_partial = offset % BLOCK_SIZE != 0;
        let first_partial_block_len = if is_first_block_partial {
            BLOCK_SIZE - (offset % BLOCK_SIZE)
        } else {
            0
        };
        let is_last_block_partial = (offset + write_buf_len) % BLOCK_SIZE != 0;
        let last_partial_block_len = if is_last_block_partial {
            (offset + write_buf_len) % BLOCK_SIZE
        } else {
            0
        };

        // Block buffers for write
        let bufs = {
            // Construct the buffers for the request in three steps
            let mut bufs = Vec::new();
            // Step 1
            if is_first_block_partial {
                let boxed_slice = {
                    // Safety. It is ok to create an uninit slice as we will fill it
                    // with valid data immediately after the creation.
                    let mut boxed_slice =
                        unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                    let first_block_offset = align_down(offset, BLOCK_SIZE);
                    self.read(first_block_offset, &mut boxed_slice).await?;

                    let write_part = &mut boxed_slice[BLOCK_SIZE - first_partial_block_len..];
                    let write_part_src = &write_buf[..first_partial_block_len];
                    write_part.copy_from_slice(write_part_src);

                    boxed_slice
                };
                let buf = BlockBuf::from_boxed(boxed_slice);
                bufs.push(buf);
            }
            // Step 2
            let whole_block_buf =
                &write_buf[first_partial_block_len..write_buf_len - last_partial_block_len];
            if whole_block_buf.len() > 0 {
                let buf = unsafe {
                    // Safety. The pointer of a slice must not be null.
                    let ptr = NonNull::new_unchecked(whole_block_buf.as_ptr() as _);
                    let len = whole_block_buf.len();
                    // Safety. The memory refered to by the pair of pointer and
                    // length is valid during the entire life cyle of the request.
                    BlockBuf::from_raw_parts(ptr, len)
                };
                bufs.push(buf);
            }
            // Step 3
            if is_last_block_partial {
                let boxed_slice = {
                    // Safety. It is ok to create an uninit slice as we will fill it
                    // with valid data immediately after the creation.
                    let mut boxed_slice =
                        unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                    let res = self
                        .read(
                            align_down(offset + write_buf_len, BLOCK_SIZE),
                            &mut boxed_slice,
                        )
                        .await;
                    if res.is_err() {
                        // Don't forget to free the boxed slice for the first block
                        if is_first_block_partial {
                            let buf = bufs.remove(0);
                            // Safety. This block buffer must be created from a boxed slice
                            drop(unsafe { BlockBuf::into_boxed(buf) });
                        }
                        return res;
                    }

                    let write_part = &mut boxed_slice[..last_partial_block_len];
                    let write_part_src = &write_buf[write_buf_len - last_partial_block_len..];
                    write_part.copy_from_slice(write_part_src);

                    boxed_slice
                };
                let buf = BlockBuf::from_boxed(boxed_slice);
                bufs.push(buf);
            }
            bufs
        };

        // We keep associate the read request with some extra info
        #[derive(Copy, Clone, Debug)]
        struct ExtraInfo {
            is_first_block_partial: bool,
            is_last_block_partial: bool,
        }
        let extra_info = ExtraInfo {
            is_first_block_partial,
            is_last_block_partial,
        };

        // Finally, we are ready to construct the write request
        let req = BioReqBuilder::new(BioType::Write)
            .addr(offset / BLOCK_SIZE)
            .bufs(bufs)
            .ext(extra_info)
            // When the request is to be dropped, we free the two boxed slices
            // that we may have allocated for the first and last partial blocks.
            .on_drop(|req: &BioReq, mut bufs: Vec<BlockBuf>| {
                let ExtraInfo {
                    is_first_block_partial,
                    is_last_block_partial,
                } = req.ext().get::<ExtraInfo>().unwrap().clone();

                if is_last_block_partial {
                    let buf = bufs.remove(bufs.len() - 1);
                    // Safety. This block buffer must be created from a boxed slice
                    drop(unsafe { BlockBuf::into_boxed(buf) });
                }
                if is_first_block_partial {
                    let buf = bufs.remove(0);
                    // Safety. This block buffer must be created from a boxed slice
                    drop(unsafe { BlockBuf::into_boxed(buf) });
                }
            })
            .build();
        let submission = self.disk.submit(Arc::new(req));
        let req = submission.complete().await;
        let res = req.response().unwrap();

        if let Err(e) = res {
            return Err(errno!(e.errno(), "write on a block device failed"));
        }

        Ok(write_buf_len)
    }

    pub async fn flush(&self) -> Result<()> {
        let req = BioReqBuilder::new(BioType::Flush).build();
        let submission = self.disk.submit(Arc::new(req));
        let req = submission.complete().await;
        req.response()
            .unwrap()
            .map_err(|e| errno!(e.errno(), "flush on a block device failed"))
    }

    // Check if a read or writer covers only one partial block. In other words,
    // it cannot cover more than one partial blocks or cover a whole block.
    fn cover_one_partial_block(offset: usize, len: usize) -> bool {
        if len >= BLOCK_SIZE {
            return false;
        }

        let aligned_begin = align_down(offset, BLOCK_SIZE);
        let aligned_end = align_up(offset + len, BLOCK_SIZE);
        (aligned_end - aligned_begin) <= BLOCK_SIZE
    }
}

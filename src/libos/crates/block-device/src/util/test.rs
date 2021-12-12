/// Generate unit tests for a block device.
#[macro_export]
macro_rules! gen_unit_tests {
    // The setup function: fn() -> D, where D implements BlockDevice.
    // The teardown function: fn(d: D), where D implements BlockDevice.
    //
    // Note that the teardown function will not be called if the unit test failed
    // due to assert failures.
    ($setup:ident, $teardown:ident) => {
        use std::sync::Arc;
        use $crate::util::test::check_disk_filled_with_val;
        use $crate::{BioReq, BioReqBuilder, BioType, BlockBuf, BlockDevice, BLOCK_SIZE};

        // Check a new disk is initialized with zeros.
        #[test]
        fn check_zeroed() {
            async_rt::task::block_on(async move {
                let disk = $setup();

                // The disk should be filled with zeros
                check_disk_filled_with_val(&disk, 0).await.unwrap();

                $teardown(disk);
            });
        }

        // Write all blocks on a disk in a single request.
        #[test]
        fn write_all() {
            async_rt::task::block_on(async move {
                let disk = $setup();

                let val = b'@'; // a printable byte

                // Send a write that fills all blocks with a single byte
                let mut boxed_slice = unsafe {
                    Box::new_uninit_slice(disk.total_blocks() * BLOCK_SIZE).assume_init()
                };
                for b in boxed_slice.iter_mut() {
                    *b = val;
                }
                let buf = BlockBuf::from_boxed(boxed_slice);
                let bufs = vec![buf];
                let req = BioReqBuilder::new(BioType::Write)
                    .addr(0)
                    .bufs(bufs)
                    .on_drop(|req: &BioReq, mut bufs: Vec<BlockBuf>| {
                        // Free the boxed slice that we allocated before
                        bufs.drain(..).for_each(|buf| {
                            // Safety. BlockBuffer is created with from_boxed
                            let boxed_slice = unsafe {
                                BlockBuf::into_boxed(buf);
                            };
                            drop(boxed_slice);
                        });
                    })
                    .build();
                let submission = disk.submit(Arc::new(req));
                let req = submission.complete().await;
                assert!(req.response() == Some(Ok(())));

                // The disk should be filled with the value
                check_disk_filled_with_val(&disk, val).await.unwrap();

                $teardown(disk);
            });
        }
    };
}

use crate::prelude::*;
use crate::{BioReq, BioReqBuilder, BioType, BlockBuf, BlockDevice, BLOCK_SIZE};

/// Check whether a disk is filled with a given byte value.
pub async fn check_disk_filled_with_val(disk: &dyn BlockDevice, val: u8) -> Result<()> {
    // Send a big read that reads all blocks of the disk
    let boxed_slice = unsafe { Box::new_uninit_slice(disk.total_bytes()).assume_init() };
    let buf = BlockBuf::from_boxed(boxed_slice);
    let bufs = vec![buf];
    let req = BioReqBuilder::new(BioType::Read)
        .addr(0)
        .bufs(bufs)
        .on_drop(|req: &BioReq, mut bufs: Vec<BlockBuf>| {
            // Free the boxed slice that we allocated before
            bufs.drain(..).for_each(|buf| {
                // Safety. BlockBuffer is created with from_boxed
                let boxed_slice = unsafe { BlockBuf::into_boxed(buf) };
                drop(boxed_slice);
            });
        })
        .build();
    let submission = disk.submit(Arc::new(req));
    let req = submission.complete().await;
    assert!(req.response() == Some(Ok(())));

    // Check if all bytes read equal to the value
    req.access_bufs_with(|bufs| {
        for buf in bufs {
            if buf.as_slice().iter().any(|b| *b != val) {
                return Err(errno!(EINVAL, "found unexpected byte"));
            }
        }
        Ok(())
    })
}

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
        use $crate::{BioReq, BlockBuf, BlockDevice, BLOCK_SIZE};

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
                let bufs = (0..disk.total_blocks())
                    .map(|_| {
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
use crate::{BioCompletionCallback, BioReq, BlockBuf, BlockDevice, BLOCK_SIZE};

/// Check whether a disk is filled with a given byte value.
pub async fn check_disk_filled_with_val(disk: &dyn BlockDevice, val: u8) -> Result<()> {
    // Initiate multiple reads, each of which reads just one block
    let reads: Vec<_> = (0..disk.total_blocks())
        .map(|addr| {
            let bufs = {
                let boxed_slice = unsafe { Box::new_uninit_slice(BLOCK_SIZE).assume_init() };
                let buf = BlockBuf::from_boxed(boxed_slice);
                vec![buf]
            };
            let callback: BioCompletionCallback = Box::new(|_req, resp| {
                assert!(resp == Ok(()));
            } as _);
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

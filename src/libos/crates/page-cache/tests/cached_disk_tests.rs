//! CachedDisk tests
use async_rt::wait::Waiter;
use block_device::{mem_disk::MemDisk, Bid, BlockDeviceAsFile, BLOCK_SIZE};
use errno::prelude::*;
use page_cache::*;

use std::sync::Arc;
use std::time::Duration;

const MB: usize = 1024 * 1024;

macro_rules! new_cached_disk_for_tests {
    ($cache_size:expr) => {{
        // `MyPageAlloc` is a test-purpose fixed-size allocator.
        impl_fixed_size_page_alloc! { MyPageAlloc, $cache_size }

        let total_blocks = 1024 * 1024;
        let mem_disk = MemDisk::new(total_blocks).unwrap();

        CachedDisk::<MyPageAlloc>::new(Arc::new(mem_disk)).unwrap()
    }};
}

#[test]
fn cached_disk_read_write() -> Result<()> {
    async_rt::task::block_on(async move {
        let cached_disk = new_cached_disk_for_tests!(256 * MB);
        let content: u8 = 5;
        const RW_SIZE: usize = 2 * BLOCK_SIZE;
        let offset = 1024;

        let mut read_buf: [u8; RW_SIZE] = [0; RW_SIZE];
        let len = cached_disk.read(offset, &mut read_buf[..]).await?;
        assert_eq!(RW_SIZE, len, "[CachedDisk] read failed");

        let write_buf: [u8; RW_SIZE] = [content; RW_SIZE];
        let len = cached_disk.write(offset, &write_buf[..]).await?;
        assert_eq!(RW_SIZE, len, "[CachedDisk] write failed");

        let len = cached_disk.read(offset, &mut read_buf[..]).await?;
        assert_eq!(RW_SIZE, len, "[CachedDisk] read failed");
        assert_eq!(read_buf, write_buf, "[CachedDisk] read wrong content");

        let rw_cnt = 10_0000;
        let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
        for i in 0..rw_cnt {
            let offset = i * BLOCK_SIZE;
            cached_disk.read(offset, &mut buf[..]).await?;
            cached_disk.write(offset, &buf[..]).await?;
        }

        cached_disk.sync().await?;
        Ok(())
    })
}

#[test]
fn cached_disk_flush() -> Result<()> {
    async_rt::task::block_on(async move {
        let cached_disk = new_cached_disk_for_tests!(256 * MB);
        const SIZE: usize = BLOCK_SIZE;
        let write_cnt = 1;
        for i in 0..write_cnt {
            let offset = i * BLOCK_SIZE;
            let write_buf: [u8; SIZE] = [0; SIZE];
            let len = cached_disk.write(offset, &write_buf[..]).await?;
            assert_eq!(SIZE, len, "[CachedDisk] write failed");
        }
        let flush_num = cached_disk.flush().await?;
        assert_eq!(flush_num, write_cnt, "[CachedDisk] flush failed");

        let write_cnt = 1000;
        for i in 0..write_cnt {
            let offset = i * BLOCK_SIZE;
            let write_buf: [u8; SIZE] = [0; SIZE];
            let len = cached_disk.write(offset, &write_buf[..]).await?;
            assert_eq!(SIZE, len, "[CachedDisk] write failed");
        }
        let flush_num = cached_disk.flush().await?;
        assert_eq!(flush_num, write_cnt, "[CachedDisk] flush failed");

        cached_disk.sync().await?;
        Ok(())
    })
}

#[test]
fn cached_disk_flush_pages() -> Result<()> {
    async_rt::task::block_on(async move {
        let cached_disk = new_cached_disk_for_tests!(256 * MB);
        const SIZE: usize = BLOCK_SIZE;
        let write_cnt = 100;
        for i in 0..write_cnt {
            let offset = i * BLOCK_SIZE;
            let write_buf: [u8; SIZE] = [0; SIZE];
            let len = cached_disk.write(offset, &write_buf[..]).await?;
            assert_eq!(SIZE, len, "[CachedDisk] write failed");
        }

        let pages = vec![Bid::new(0), Bid::new(1), Bid::new(2)];
        let flush_num = cached_disk.flush_blocks(&pages).await?;
        assert_eq!(flush_num, pages.len(), "[CachedDisk] flush pages failed");

        cached_disk.sync().await?;
        Ok(())
    })
}

#[test]
fn cached_disk_flusher_task() -> Result<()> {
    async_rt::task::block_on(async move {
        let cached_disk = Arc::new(new_cached_disk_for_tests!(256 * MB));
        let reader = cached_disk.clone();
        let writer = cached_disk.clone();
        const SIZE: usize = 4096;
        let rw_cnt = 10;

        let writer_handle = async_rt::task::spawn(async move {
            for _ in 0..rw_cnt {
                let write_buf: [u8; SIZE] = [0; SIZE];
                writer.write(0, &write_buf[..]).await.unwrap();
            }
        });
        let reader_handle = async_rt::task::spawn(async move {
            let waiter = Waiter::new();
            for _ in 0..rw_cnt {
                let _ = waiter
                    .wait_timeout(Some(&mut Duration::from_millis(500)))
                    .await;
                let mut read_buf: [u8; SIZE] = [0; SIZE];
                reader.read(0, &mut read_buf[..]).await.unwrap();
            }
        });

        writer_handle.await;
        reader_handle.await;

        let flush_num = cached_disk.flush().await?;
        // Pages are already flushed by flusher task
        assert_eq!(flush_num, 0, "[CachedDisk] flush failed");

        cached_disk.sync().await?;
        Ok(())
    })
}

#[test]
fn cached_disk_evictor_task() -> Result<()> {
    async_rt::task::block_on(async move {
        let cached_disk = new_cached_disk_for_tests!(100 * BLOCK_SIZE);

        // Support out-limit read/write thanks to the evictor task
        let rw_cnt = 500;
        let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
        for i in 0..rw_cnt {
            let offset = i * BLOCK_SIZE;
            cached_disk.read(offset, &mut buf[..]).await?;
        }

        cached_disk.sync().await?;
        Ok(())
    })
}

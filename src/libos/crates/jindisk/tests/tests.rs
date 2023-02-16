//! Integration Test of JinDisk.
use block_device::BLOCK_SIZE;
use errno::prelude::*;
use jindisk::{DefaultCryptor, GiB, JinDisk, MiB};
use sgx_disk::{HostDisk, SyncIoDisk};

use std::sync::Arc;

#[allow(unused)]
fn create_new_jindisk() -> JinDisk {
    let total_blocks = 3 * GiB / BLOCK_SIZE;
    fn gen_unique_path() -> String {
        use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
        static UT_ID: AtomicU32 = AtomicU32::new(0);
        let ut_id = UT_ID.fetch_add(1, Relaxed);
        format!("jindisk{}.image", ut_id)
    }
    let path = gen_unique_path();
    let sync_disk = SyncIoDisk::create(&path, total_blocks).unwrap();
    let root_key = DefaultCryptor::gen_random_key();
    JinDisk::create(Arc::new(sync_disk), root_key)
}

#[test]
fn jindisk_open() -> Result<()> {
    async_rt::task::block_on(async move {
        let jindisk = create_new_jindisk();
        let sb1 = jindisk.superblock().clone();
        let disk = jindisk.disk().clone();
        let root_key = jindisk.root_key().clone();
        jindisk.sync().await?;
        drop(jindisk);

        let jindisk = JinDisk::open(disk, root_key).await.unwrap();

        let sb2 = jindisk.superblock().clone();
        assert_eq!(format!("{:?}", sb1), format!("{:?}", sb2));

        jindisk.sync().await?;
        Ok(())
    })
}

#[test]
fn jindisk_write_read() -> Result<()> {
    async_rt::task::block_on(async move {
        let jindisk = create_new_jindisk();

        let rw_cnt = 512 * MiB / BLOCK_SIZE;
        for i in 0..rw_cnt {
            let wbuf = [i as u8; BLOCK_SIZE];
            jindisk.write(i * BLOCK_SIZE, &wbuf).await?;
        }
        for i in 0..rw_cnt {
            let mut rbuf = [0u8; BLOCK_SIZE];
            jindisk.read(i * BLOCK_SIZE, &mut rbuf).await?;
            assert_eq!([i as u8; BLOCK_SIZE], rbuf, "read wrong content");
        }

        jindisk.sync().await?;
        Ok(())
    })
}

#[test]
fn range_query() -> Result<()> {
    async_rt::task::block_on(async move {
        let jindisk = create_new_jindisk();

        const RANGE: usize = 8 * BLOCK_SIZE;
        let rw_cnt = 1 * GiB / RANGE;
        for i in 0..rw_cnt {
            let wbuf = [i as u8; RANGE];
            jindisk.write(i * RANGE, &wbuf).await?;
        }
        for i in 0..rw_cnt {
            let mut rbuf = [0u8; RANGE];
            jindisk.read(i * RANGE, &mut rbuf).await?;
            assert_eq!([i as u8; RANGE], rbuf, "range read wrong content");
        }

        let mut rbuf = [0u8; 2 * RANGE];
        jindisk.read(512 * MiB - RANGE, &mut rbuf).await?;
        assert_eq!([255u8; RANGE], rbuf[0..RANGE], "range read wrong content");
        assert_eq!(
            [0u8; RANGE],
            rbuf[RANGE..2 * RANGE],
            "range read wrong content"
        );

        jindisk.sync().await?;
        Ok(())
    })
}

// TODO: Benchmarking this with `cargo bench`
#[test]
fn stress_test() -> Result<()> {
    async_rt::task::block_on(async move {
        let jindisk = create_new_jindisk();
        const TOTAL_SIZE: usize = 2 * GiB;

        const RW_SIZE: usize = 32 * BLOCK_SIZE;
        let rw_cnt: usize = TOTAL_SIZE / RW_SIZE;

        let start = std::time::Instant::now();
        for i in 0..rw_cnt {
            let wbuf = [i as u8; RW_SIZE];
            jindisk.write(i * RW_SIZE, &wbuf).await?;
        }

        let duration = start.elapsed();
        println!("Time elapsed in stress_test::write is: {:?}", duration);

        let start = std::time::Instant::now();
        for i in 0..rw_cnt {
            let mut rbuf = [0u8; RW_SIZE];
            jindisk.read(i * RW_SIZE, &mut rbuf).await?;
            assert_eq!(rbuf, [i as u8; RW_SIZE])
        }

        let duration = start.elapsed();
        println!("Time elapsed in stress_test::read is: {:?}", duration);

        let start = std::time::Instant::now();
        jindisk.sync().await?;
        let duration = start.elapsed();
        println!("Time elapsed in stress_test::sync is: {:?}", duration);

        for id in 0..4 {
            let _ = std::fs::remove_file(format!("jindisk{}.image", id));
        }
        Ok(())
    })
}

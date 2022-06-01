#![feature(new_uninit)]

use std::time::Instant;

use errno::prelude::{Errno::*, *};

use self::benches::{Bench, BenchBuilder, IoPattern, IoType};
use self::consts::*;
use self::tmp_disk::DiskType;
use self::util::{DisplayData, DisplayThroughput};

fn main() {
    // Specify the number of threads to execute async tasks
    async_rt::config::set_parallelism(4);

    // Specify all benchmarks
    let mut benches = [
        BenchBuilder::new("sync_io_disk::write_seq")
            .disk_type(DiskType::SyncIoDisk)
            .io_type(IoType::Write)
            .io_pattern(IoPattern::Seq)
            .total_bytes(1 * GiB)
            .concurrency(1)
            .build()
            .unwrap(),
        BenchBuilder::new("sync_io_disk::read_seq")
            .disk_type(DiskType::SyncIoDisk)
            .io_type(IoType::Read)
            .io_pattern(IoPattern::Seq)
            .total_bytes(1 * GiB)
            .concurrency(1)
            .build()
            .unwrap(),
        BenchBuilder::new("io_uring_disk::write_seq")
            .disk_type(DiskType::IoUringDisk)
            .io_type(IoType::Write)
            .io_pattern(IoPattern::Seq)
            .total_bytes(1 * GiB)
            .concurrency(1)
            .build()
            .unwrap(),
        BenchBuilder::new("io_uring_disk::read_seq")
            .disk_type(DiskType::IoUringDisk)
            .io_type(IoType::Read)
            .io_pattern(IoPattern::Seq)
            .total_bytes(1 * GiB)
            .concurrency(1)
            .build()
            .unwrap(),
    ];

    // Run all benchmarks and output the results
    run_benches(&mut benches);
}

fn run_benches(benches: &mut [Box<dyn Bench>]) {
    println!("");

    let mut benched_count = 0;
    let mut failed_count = 0;
    for b in benches {
        print!("bench {} ... ", &b);
        let start = Instant::now();
        let res = b.run();
        if let Err(e) = res {
            failed_count += 1;
            println!("failed due to error {:?}", e);
            continue;
        }

        let end = Instant::now();
        let elapsed = end - start;
        let throughput = DisplayThroughput::new(b.total_bytes(), elapsed);
        println!("{}", throughput);
        benched_count += 1;
    }

    let bench_res = if failed_count == 0 { "ok" } else { "failed" };
    println!(
        "\nbench result: {}. {} benched; {} failed.",
        bench_res, benched_count, failed_count
    );
}

mod benches {
    use std::fmt::{self};
    use std::sync::Arc;

    use super::tmp_disk::{DiskType, TmpDisk};
    use super::*;
    use async_rt::task::JoinHandle;
    use async_trait::async_trait;
    use block_device::{BlockDevice, BlockDeviceExt, BLOCK_SIZE};

    pub trait Bench: fmt::Display {
        /// Returns the name of the benchmark.
        fn name(&self) -> &str;

        /// Returns the total number of bytes read or written.
        fn total_bytes(&self) -> usize;

        /// Run the benchmark.
        fn run(&mut self) -> Result<()>;
    }

    pub struct BenchBuilder {
        name: String,
        disk_type: Option<DiskType>,
        io_type: Option<IoType>,
        io_pattern: Option<IoPattern>,
        buf_size: usize,
        total_bytes: usize,
        concurrency: u32,
    }

    impl BenchBuilder {
        pub fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                disk_type: None,
                io_type: None,
                io_pattern: None,
                buf_size: 4 * KiB,
                total_bytes: 10 * MB,
                concurrency: 1,
            }
        }

        pub fn disk_type(mut self, disk_type: DiskType) -> Self {
            self.disk_type = Some(disk_type);
            self
        }

        pub fn io_type(mut self, io_type: IoType) -> Self {
            self.io_type = Some(io_type);
            self
        }

        pub fn io_pattern(mut self, io_pattern: IoPattern) -> Self {
            self.io_pattern = Some(io_pattern);
            self
        }

        pub fn buf_size(mut self, buf_size: usize) -> Self {
            self.buf_size = buf_size;
            self
        }

        pub fn total_bytes(mut self, total_bytes: usize) -> Self {
            self.total_bytes = total_bytes;
            self
        }

        pub fn concurrency(mut self, concurrency: u32) -> Self {
            self.concurrency = concurrency;
            self
        }

        pub fn build(self) -> Result<Box<dyn Bench>> {
            let Self {
                name,
                disk_type,
                io_type,
                io_pattern,
                buf_size,
                total_bytes,
                concurrency,
            } = self;

            let disk_type = match disk_type {
                Some(disk_type) => disk_type,
                None => return Err(errno!(EINVAL, "disk_type is not given")),
            };
            let io_type = match io_type {
                Some(io_type) => io_type,
                None => return Err(errno!(EINVAL, "io_type is not given")),
            };
            let io_pattern = match io_pattern {
                Some(io_pattern) => io_pattern,
                None => return Err(errno!(EINVAL, "io_pattern is not given")),
            };
            if buf_size == 0 {
                return Err(errno!(EINVAL, "buf_size must be greater than 0"));
            }
            if total_bytes == 0 {
                return Err(errno!(EINVAL, "total_bytes must be greater than 0"));
            }
            if concurrency == 0 {
                return Err(errno!(EINVAL, "concurrency must be greater than 0"));
            }

            Ok(Box::new(SimpleDiskBench {
                name,
                disk_type,
                io_type,
                io_pattern,
                buf_size,
                total_bytes,
                concurrency,
            }))
        }
    }

    pub struct SimpleDiskBench {
        name: String,
        disk_type: DiskType,
        io_type: IoType,
        io_pattern: IoPattern,
        buf_size: usize,
        total_bytes: usize,
        concurrency: u32,
    }

    impl Bench for SimpleDiskBench {
        fn name(&self) -> &str {
            &self.name
        }

        fn total_bytes(&self) -> usize {
            self.total_bytes
        }

        fn run(&mut self) -> Result<()> {
            let disk = Arc::new(TmpDisk::create(self.disk_type, self.total_bytes)?);
            let io_type = self.io_type;
            let io_pattern = self.io_pattern;
            let buf_size = self.buf_size;
            let total_bytes = self.total_bytes;
            let concurrency = self.concurrency;
            async_rt::task::block_on(async move {
                let mut join_handles: Vec<JoinHandle<Result<()>>> = (0..concurrency)
                    .map(|i| {
                        let disk = disk.clone();
                        let local_bytes = total_bytes / (concurrency as usize);
                        let local_offset = (i as usize) * local_bytes;
                        async_rt::task::spawn(async move {
                            match (io_type, io_pattern) {
                                (IoType::Read, IoPattern::Seq) => {
                                    disk.read_seq(local_offset, local_bytes, buf_size).await
                                }
                                //(IoType::Read, IoPattern::Rnd) => disk.read_rnd(total_bytes, buf_size).await,
                                (IoType::Write, IoPattern::Seq) => {
                                    disk.write_seq(local_offset, local_bytes, buf_size).await
                                }
                                //(IoType::Write, IoPattern::Rnd) => disk.write_rnd(total_bytes, buf_size).await,
                                _ => Err(errno!(ENOSYS, "random I/O is not supported yet")),
                            }
                        })
                    })
                    .collect();

                let mut any_error = None;
                for (i, join_handle) in join_handles.iter_mut().enumerate() {
                    let res = join_handle.await;
                    if let Err(e) = res {
                        println!("benchmark task error: {:?}", &e);
                        any_error = Some(e);
                    }
                }
                match any_error {
                    None => Ok(()),
                    Some(e) => Err(e),
                }
            })
        }
    }

    #[async_trait]
    pub trait BlockDeviceBenchExt {
        async fn read_seq(&self, offset: usize, total_bytes: usize, buf_size: usize) -> Result<()>;
        async fn write_seq(&self, offset: usize, total_bytes: usize, buf_size: usize)
            -> Result<()>;
    }

    #[async_trait]
    impl<B: BlockDevice> BlockDeviceBenchExt for B {
        async fn read_seq(
            &self,
            mut offset: usize,
            total_bytes: usize,
            buf_size: usize,
        ) -> Result<()> {
            let disk_size = self.total_blocks() * BLOCK_SIZE;
            let mut buf: Box<[u8]> = unsafe { Box::new_uninit_slice(buf_size).assume_init() };
            let mut remain_bytes = total_bytes;
            while remain_bytes > 0 {
                if offset >= disk_size {
                    offset = 0;
                }

                let max_read = remain_bytes.min(buf_size).min(disk_size - offset);
                let read_len = self.read(offset, &mut buf[..max_read]).await?;

                remain_bytes -= read_len;
                offset += read_len;
            }
            Ok(())
        }

        async fn write_seq(
            &self,
            mut offset: usize,
            total_bytes: usize,
            buf_size: usize,
        ) -> Result<()> {
            let disk_size = self.total_blocks() * BLOCK_SIZE;
            let buf: Box<[u8]> = unsafe { Box::new_zeroed_slice(buf_size).assume_init() };
            let mut remain_bytes = total_bytes;
            while remain_bytes > 0 {
                if offset >= disk_size {
                    offset = 0;
                }

                let max_write = remain_bytes.min(buf_size).min(disk_size - offset);
                let write_len = self.write(offset, &buf[..max_write]).await?;

                remain_bytes -= write_len;
                offset += write_len;
            }
            self.flush().await?;
            Ok(())
        }
    }

    impl fmt::Display for SimpleDiskBench {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{} (total = {}, buf = {}, tasks = {})",
                self.name(),
                DisplayData::new(self.total_bytes),
                DisplayData::new(self.buf_size),
                self.concurrency
            )
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum IoType {
        Read,
        Write,
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum IoPattern {
        Seq,
        Rnd,
    }
}

mod consts {
    pub const B: usize = 1;

    pub const KiB: usize = 1024 * B;
    pub const MiB: usize = 1024 * KiB;
    pub const GiB: usize = 1024 * MiB;

    pub const KB: usize = 1000 * B;
    pub const MB: usize = 1000 * KB;
    pub const GB: usize = 1000 * MB;
}

mod tmp_disk {
    use std::sync::Arc;

    use block_device::{BioReq, BioSubmission, BlockDevice, BlockDeviceExt, BLOCK_SIZE};
    use sgx_disk::{HostDisk, IoUringDisk, SyncIoDisk};

    use self::io_uring_rt::IoUringSingleton;
    use super::util::align_up;
    use super::*;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum DiskType {
        SyncIoDisk,
        IoUringDisk,
    }

    pub struct TmpDisk {
        file_path: String,
        inner_disk: Box<dyn BlockDevice>,
    }

    impl TmpDisk {
        pub fn create(disk_type: DiskType, total_bytes: usize) -> Result<Self> {
            let file_path = Self::gen_tmp_path();
            let total_blocks = align_up(total_bytes, BLOCK_SIZE) / BLOCK_SIZE;
            let inner_disk: Box<dyn BlockDevice> = match disk_type {
                DiskType::SyncIoDisk => Box::new(SyncIoDisk::create(&file_path, total_blocks)?),
                DiskType::IoUringDisk => Box::new(IoUringDisk::<IoUringSingleton>::create(
                    &file_path,
                    total_blocks,
                )?),
            };
            let tmp_disk = Self {
                file_path,
                inner_disk,
            };
            Ok(tmp_disk)
        }

        fn gen_tmp_path() -> String {
            use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
            static NEXT_ID: AtomicU32 = AtomicU32::new(0);
            let id = NEXT_ID.fetch_add(1, Relaxed);
            format!("disk{}.image", id)
        }
    }

    impl BlockDevice for TmpDisk {
        fn total_blocks(&self) -> usize {
            self.inner_disk.total_blocks()
        }

        fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
            self.inner_disk.submit(req)
        }
    }

    impl Drop for TmpDisk {
        fn drop(&mut self) {
            std::fs::remove_file(&self.file_path);
        }
    }

    mod io_uring_rt {
        use super::*;
        use io_uring_callback::{Builder, IoUring};
        use lazy_static::lazy_static;
        use sgx_disk::IoUringProvider;

        pub struct IoUringSingleton;

        impl IoUringProvider for IoUringSingleton {
            fn io_uring() -> &'static IoUring {
                &*IO_URING
            }
        }

        lazy_static! {
            static ref IO_URING: Arc<IoUring> = {
                let ring = Arc::new(Builder::new().build(256).unwrap());
                unsafe {
                    ring.start_enter_syscall_thread();
                }
                async_rt::task::spawn({
                    let ring = ring.clone();
                    async move {
                        loop {
                            ring.poll_completions();
                            async_rt::sched::yield_().await;
                        }
                    }
                });
                ring
            };
        }
    }
}

mod util {
    use super::*;
    use std::fmt::{self};
    use std::time::Duration;

    pub fn align_up(n: usize, a: usize) -> usize {
        debug_assert!(a >= 2 && a.is_power_of_two());
        (n + a - 1) & !(a - 1)
    }

    /// Display the amount of data in the unit of GB, MB, KB, or bytes.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct DisplayData(usize);

    impl DisplayData {
        pub fn new(nbytes: usize) -> Self {
            Self(nbytes)
        }
    }

    impl fmt::Display for DisplayData {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            const UNIT_TABLE: [(&str, usize); 4] =
                [("GiB", GiB), ("MiB", MiB), ("KiB", KiB), ("bytes", 0)];
            let (unit_str, unit_val) = {
                let (unit_str, mut unit_val) = UNIT_TABLE
                    .iter()
                    .find(|(_, unit_val)| self.0 >= *unit_val)
                    .unwrap();
                if unit_val == 0 {
                    unit_val = 1;
                }
                (unit_str, unit_val)
            };
            let data_val_in_unit = (self.0 as f64) / (unit_val as f64);
            write!(f, "{:.1} {}", data_val_in_unit, unit_str)
        }
    }

    /// Display throughput in the unit of bytes/s, KB/s, MB/s, or GB/s.
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct DisplayThroughput(f64);

    impl DisplayThroughput {
        pub fn new(total_bytes: usize, elapsed: Duration) -> Self {
            let total_bytes = total_bytes as f64;
            let elapsed_secs = elapsed.as_secs_f64();
            let throughput = total_bytes / elapsed_secs;
            Self(throughput)
        }
    }

    impl fmt::Display for DisplayThroughput {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            const UNIT_TABLE: [(&str, usize); 4] =
                [("GB/s", GB), ("MB/s", MB), ("KB/s", KB), ("bytes/s", 0)];
            let (unit_str, unit_val) = {
                let (unit_str, mut unit_val) = UNIT_TABLE
                    .iter()
                    .find(|(_, unit_val)| self.0 >= (*unit_val as f64))
                    .unwrap();
                if unit_val == 0 {
                    unit_val = 1;
                }
                (unit_str, unit_val)
            };
            let throughput_in_unit = self.0 / (unit_val as f64);
            write!(f, "{:.1} {}", throughput_in_unit, unit_str)
        }
    }
}

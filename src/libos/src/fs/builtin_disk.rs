//! Built-in disks for testing purposes
use std::lazy::SyncLazy as Lazy;
use std::path::PathBuf;

use block_device::{mem_disk::MemDisk, BlockDeviceAsFile, BLOCK_SIZE};
use jindisk::JinDisk;
use lazy_static::lazy_static;
use runtime::IoUringRuntime;
use sgx_disk::{CryptDisk, HostDisk, IoUringDisk, PfsDisk, SyncIoDisk};

use super::*;

pub async fn try_open_disk(fs: &FsView, fs_path: &FsPath) -> Result<Option<Arc<DiskFile>>> {
    let abs_path = fs.convert_fspath_to_abs(&fs_path)?;
    if !abs_path.starts_with("/dev") {
        return Ok(None);
    }

    let disk: Arc<dyn BlockDeviceAsFile> = match abs_path.as_str() {
        "/dev/mem_disk" => {
            let total_blocks = 32 * MB / BLOCK_SIZE;
            let mem_disk = MemDisk::new(total_blocks)?;
            Arc::new(mem_disk)
        }
        "/dev/sync_disk" => {
            let file_path = "run/sync_disk.image";
            let total_blocks = 2 * GB / BLOCK_SIZE;
            let disk = SyncIoDisk::open(file_path)
                .or_else(|_| SyncIoDisk::create(file_path, total_blocks))?;
            Arc::new(disk)
        }
        "/dev/iou_disk" => {
            let file_path = "run/iou_disk.image";
            let total_blocks = 2 * GB / BLOCK_SIZE;
            let disk = IoUringDisk::<IoUringRuntime>::open(file_path)
                .or_else(|_| IoUringDisk::create(file_path, total_blocks))?;
            Arc::new(disk)
        }
        "/dev/crypt_sync_disk" => {
            let file_path = "run/crypt_sync_disk.image";
            let total_blocks = 2 * GB / BLOCK_SIZE;
            let disk = SyncIoDisk::open(file_path)
                .or_else(|_| SyncIoDisk::create(file_path, total_blocks))?;
            let crypt_disk = CryptDisk::new(Box::new(disk));
            Arc::new(crypt_disk)
        }
        "/dev/crypt_iou_disk" => {
            let file_path = "run/crypt_iou_disk.image";
            let total_blocks = 2 * GB / BLOCK_SIZE;
            let disk = IoUringDisk::<IoUringRuntime>::open(file_path)
                .or_else(|_| IoUringDisk::create(file_path, total_blocks))?;
            let crypt_disk = CryptDisk::new(Box::new(disk));
            Arc::new(crypt_disk)
        }
        "/dev/pfs_disk" => {
            let file_path = "run/pfs_disk.image";
            let total_blocks = 2 * GB / BLOCK_SIZE;
            let disk =
                PfsDisk::open(file_path).or_else(|_| PfsDisk::create(file_path, total_blocks))?;
            Arc::new(disk)
        }
        "/dev/jindisk" => {
            let file_path = "run/jindisk.image";
            let total_blocks = 5 * GB / BLOCK_SIZE;
            let sync_disk = Arc::new(
                SyncIoDisk::open(file_path)
                    .or_else(|_| SyncIoDisk::create(file_path, total_blocks))?,
            );
            let root_key = sgx_key_128bit_t::default();
            let jindisk = JinDisk::open(sync_disk.clone(), root_key)
                .await
                .unwrap_or(JinDisk::create(sync_disk, root_key));
            Arc::new(jindisk)
        }
        _ => {
            return Ok(None);
        }
    };
    let disk_file = Arc::new(DiskFile::new(disk));
    Ok(Some(disk_file))
}

const MB: usize = 1024 * 1024;
const GB: usize = 1024 * 1024 * 1024;

mod runtime {
    use io_uring_callback::IoUring;
    use sgx_disk::IoUringProvider;

    pub struct IoUringRuntime;

    impl IoUringProvider for IoUringRuntime {
        fn io_uring() -> &'static IoUring {
            &*crate::io_uring::SINGLETON
        }
    }
}

//! Built-in disks for testing purposes
use std::lazy::SyncLazy as Lazy;
use std::path::PathBuf;

use block_device::{mem_disk::MemDisk, BlockDevice, BLOCK_SIZE};
use lazy_static::lazy_static;
use runtime::IoUringRuntime;
use sgx_disk::{CryptDisk, HostDisk, IoUringDisk, PfsDisk, SyncIoDisk};

use super::*;

pub fn try_open_disk(fs: &FsView, fs_path: &FsPath) -> Result<Option<Arc<DiskFile>>> {
    let abs_path = fs.convert_fspath_to_abs(&fs_path)?;
    if !abs_path.starts_with("/dev") {
        return Ok(None);
    }

    let abs_path = PathBuf::from(abs_path);
    if let Some(lazy_disk) = BUILTIN_DISKS.get(&abs_path) {
        let disk: Arc<dyn BlockDevice> = lazy_disk.deref().clone();
        let disk_file = Arc::new(DiskFile::new(disk));
        Ok(Some(disk_file))
    } else {
        Ok(None)
    }
}

lazy_static! {
    static ref BUILTIN_DISKS: HashMap<PathBuf, LazyDisk> = {
        fn new_disk_entry<F>(
            name: &str,
            disk_size: usize,
            new_disk_fn: F,
        ) -> (PathBuf, LazyDisk)
            where F: Fn(/*file_path:*/ &str, /*total_blocks:*/ usize) -> Arc<dyn BlockDevice> + Send + 'static
        {
            let name = name.to_string();
            let dev_path = PathBuf::from(format!("/dev/{}", &name));
            let lazy_disk: LazyDisk = Lazy::new(Box::new(move || {
                let total_blocks = to_blocks(disk_size);
                let file_path = format!("{}.image", name);
                (new_disk_fn)(&file_path, total_blocks)
            }));
            (dev_path, lazy_disk)
        }

        let disk_entries = vec![
            new_disk_entry("mem_disk", 32*MB, |_, total_blocks| {
                let mem_disk = MemDisk::new(total_blocks).unwrap();
                Arc::new(mem_disk)
            }),
            new_disk_entry("sync_disk", 2*GB, |file_path, total_blocks| {
                let disk = SyncIoDisk::create(file_path, total_blocks).unwrap();
                Arc::new(disk)
            }),
            new_disk_entry("iou_disk", 2*GB, |file_path, total_blocks| {
                let disk = IoUringDisk::<IoUringRuntime>::create(file_path, total_blocks).unwrap();
                Arc::new(disk)
            }),
            new_disk_entry("crypt_sync_disk", 2*GB, |file_path, total_blocks| {
                let sync_disk = SyncIoDisk::create(file_path, total_blocks).unwrap();
                let crypt_disk = CryptDisk::new(Box::new(sync_disk));
                Arc::new(crypt_disk)
            }),
            new_disk_entry("crypt_iou_disk", 2*GB, |file_path, total_blocks| {
                let iou_disk = IoUringDisk::<IoUringRuntime>::create(file_path, total_blocks).unwrap();
                let crypt_disk = CryptDisk::new(Box::new(iou_disk));
                Arc::new(crypt_disk)
            }),
            new_disk_entry("pfs_disk", 2*GB, |file_path, total_blocks| {
                let disk = PfsDisk::create(file_path, total_blocks).unwrap();
                Arc::new(disk)
            }),
        ];
        let builtin_disks: HashMap<PathBuf, LazyDisk> = disk_entries
            .into_iter()
            .collect();
        builtin_disks
    };
}

type LazyDisk = Lazy<Arc<dyn BlockDevice>, Box<dyn FnOnce() -> Arc<dyn BlockDevice> + Send>>;

const MB: usize = 1024 * 1024;
const GB: usize = 1024 * 1024 * 1024;

fn to_blocks(size_in_bytes: usize) -> usize {
    align_up(size_in_bytes, BLOCK_SIZE) / BLOCK_SIZE
}

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

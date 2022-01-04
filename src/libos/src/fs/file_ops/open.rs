use host_disk::{HostDisk, IoUringDisk, SyncIoDisk};

use super::*;
use crate::fs::DiskFile;
use disks::try_open_disk;

pub fn do_openat(fs_path: &FsPath, flags: u32, mode: FileMode) -> Result<FileDesc> {
    debug!(
        "openat: fs_path: {:?}, flags: {:#o}, mode: {:#o}",
        fs_path, flags, mode
    );

    let current = current!();
    let fs = current.fs().read().unwrap();
    let masked_mode = mode & !current.process().umask();

    let file_ref = if let Some(disk_file) = try_open_disk(&fs, fs_path)? {
        FileRef::new_disk(disk_file)
    } else {
        let inode_file = fs.open_file(&fs_path, flags, masked_mode)?;
        FileRef::new_inode(inode_file)
    };

    let fd = {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        current.add_file(file_ref, creation_flags.must_close_on_spawn())
    };
    Ok(fd)
}

mod disks {
    use super::*;

    pub fn try_open_disk(fs: &FsView, fs_path: &FsPath) -> Result<Option<Arc<DiskFile>>> {
        let abs_path = fs.convert_fspath_to_abs(&fs_path)?;
        if !abs_path.starts_with("/dev") {
            return Ok(None);
        }

        const GB: usize = 1024 * 1024 * 1024;
        let disk_file = if abs_path == "/dev/sync_io_dev" {
            let total_blocks = to_blocks(2 * GB);
            let path = String::from("sync_io_dev");
            let disk = SyncIoDisk::create(&path, total_blocks).unwrap();
            DiskFile::new(disk)
        } else if abs_path == "/dev/async_io_dev" {
            let total_blocks = to_blocks(2 * GB);
            let path = String::from("async_io_dev");
            let disk = IoUringDisk::<runtime::DeviceRuntime>::create(&path, total_blocks).unwrap();
            DiskFile::new(disk)
        } else {
            return Ok(None);
        };
        Ok(Some(Arc::new(disk_file)))
    }

    const fn to_blocks(size_in_bytes: usize) -> usize {
        const PAGE_SIZE: usize = 4096;
        size_in_bytes / PAGE_SIZE
    }

    mod runtime {
        use host_disk::IoUringProvider;
        use io_uring_callback::IoUring;

        pub struct DeviceRuntime;

        impl IoUringProvider for DeviceRuntime {
            fn io_uring() -> &'static IoUring {
                &*crate::io_uring::SINGLETON
            }
        }
    }
}

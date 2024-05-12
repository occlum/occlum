use super::rootfs::mount_fs_at;
use super::*;

use rcore_fs::vfs;
use rcore_fs_devfs::DevFS;
use rcore_fs_mountfs::MountFS;
use rcore_fs_ramfs::RamFS;

use self::dev_fd::DevFd;
use self::dev_null::DevNull;
use self::dev_random::DevRandom;
use self::dev_sgx::DevSgx;
use self::dev_shm::DevShm;
use self::dev_zero::DevZero;
use blk::DevDisk;

mod dev_fd;
mod dev_null;
mod dev_random;
mod dev_sgx;
mod dev_shm;
mod dev_zero;

/// API to initialize the DevFS
pub fn init_devfs(disk_options: &[DevDiskOption]) -> Result<Arc<MountFS>> {
    let devfs = DevFS::new();
    let dev_null = Arc::new(DevNull) as _;
    devfs.add("null", dev_null)?;
    let dev_zero = Arc::new(DevZero) as _;
    devfs.add("zero", dev_zero)?;
    let dev_random = Arc::new(DevRandom) as _;
    devfs.add("random", Arc::clone(&dev_random))?;
    devfs.add("urandom", Arc::clone(&dev_random))?;
    devfs.add("arandom", Arc::clone(&dev_random))?;
    let dev_sgx = Arc::new(DevSgx) as _;
    devfs.add("sgx", dev_sgx)?;
    let dev_shm = Arc::new(DevShm) as _;
    devfs.add("shm", dev_shm)?;
    let dev_fd = Arc::new(DevFd) as _;
    devfs.add("fd", dev_fd);
    for disk_option in disk_options {
        let disk_name = &disk_option.name;
        let dev_disk = Arc::new(DevDisk::open_or_create(disk_name)?);
        devfs.add(disk_name, dev_disk)?;
    }
    let mountable_devfs = MountFS::new(devfs);
    // Mount the ramfs at '/shm'
    let ramfs = RamFS::new();
    mount_fs_at(
        ramfs,
        &mountable_devfs.root_inode(),
        &Path::new("/shm"),
        true,
    )?;
    // TODO: Add stdio(stdin, stdout, stderr) into DevFS
    Ok(mountable_devfs)
}

/// Options of block device under the DevFS.
pub struct DevDiskOption {
    name: &'static str,
}

impl DevDiskOption {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

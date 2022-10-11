use super::*;

use rcore_fs::vfs;
use rcore_fs_devfs::DevFS;

use self::dev_fd::DevFd;
use self::dev_null::DevNull;
use self::dev_random::DevRandom;
use self::dev_sgx::DevSgx;
use self::dev_shm::DevShm;
use self::dev_zero::DevZero;

mod dev_fd;
mod dev_null;
mod dev_random;
mod dev_sgx;
mod dev_shm;
mod dev_zero;

/// API to initialize the DevFS
pub fn init_devfs() -> Result<Arc<DevFS>> {
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
    // TODO: Add stdio(stdin, stdout, stderr) into DevFS
    Ok(devfs)
}

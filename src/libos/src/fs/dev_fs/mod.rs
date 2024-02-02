use super::rootfs::mount_fs_at;
use super::*;

use rcore_fs::vfs;
use rcore_fs_devfs::DevFS;
use rcore_fs_mountfs::MountFS;
use rcore_fs_ramfs::RamFS;

#[cfg(feature = "dcap")]
use self::dev_attestation::DevAttestQuote;
#[cfg(feature = "dcap")]
use self::dev_attestation::DevAttestReportData;
#[cfg(feature = "dcap")]
use self::dev_attestation::DevAttestType;
use self::dev_fd::DevFd;
use self::dev_null::DevNull;
use self::dev_random::DevRandom;
use self::dev_sgx::DevSgx;
use self::dev_shm::DevShm;
use self::dev_zero::DevZero;

#[cfg(feature = "dcap")]
mod dev_attestation;
mod dev_fd;
mod dev_null;
mod dev_random;
mod dev_sgx;
mod dev_shm;
mod dev_zero;

/// API to initialize the DevFS
pub fn init_devfs() -> Result<Arc<MountFS>> {
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
    #[cfg(feature = "dcap")]
    {
        let dev_attest_type = Arc::new(DevAttestType) as _;
        devfs.add("attestation_type", dev_attest_type)?;
        let dev_attest_report_data = Arc::new(DevAttestReportData) as _;
        devfs.add("attestation_report_data", dev_attest_report_data)?;
        let dev_attest_quote = Arc::new(DevAttestQuote) as _;
        devfs.add("attestation_quote", dev_attest_quote)?;
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

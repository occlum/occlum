use super::*;
use rcore_fs::vfs::FsInfo;
use std::convert::TryFrom;
use std::ffi::CString;

pub async fn do_fstatfs(fd: FileDesc) -> Result<Statfs> {
    debug!("fstatfs: fd: {}", fd);

    let file_ref = current!().file(fd)?;
    let statfs = {
        let fs_info = if let Some(async_file_handle) = file_ref.as_async_file_handle() {
            async_file_handle.dentry().inode().fs().info().await
        } else {
            return_errno!(EBADF, "not an inode");
        };
        Statfs::try_from(fs_info)?
    };
    trace!("fstatfs result: {:?}", statfs);
    Ok(statfs)
}

pub async fn do_statfs(path: &FsPath) -> Result<Statfs> {
    debug!("statfs: path: {:?}", path);

    let inode = {
        let current = current!();
        let fs = current.fs();
        fs.lookup_inode(path).await?
    };
    let statfs = {
        let fs_info = inode.fs().info().await;
        Statfs::try_from(fs_info)?
    };
    trace!("statfs result: {:?}", statfs);
    Ok(statfs)
}

#[derive(Default, Debug)]
#[repr(C)]
pub struct Statfs {
    /// Type of filesystem
    f_type: usize,
    /// Optimal transfer block size
    f_bsize: usize,
    /// Total data blocks in filesystem
    f_blocks: usize,
    /// Free blocks in filesystem
    f_bfree: usize,
    /// Free blocks available to unprivileged user
    f_bavail: usize,
    /// Total inodes in filesystem
    f_files: usize,
    /// Free inodes in filesystem
    f_ffree: usize,
    /// Filesystem ID
    f_fsid: [i32; 2],
    /// Maximum length of filenames
    f_namelen: usize,
    /// Fragment size
    f_frsize: usize,
    /// Mount flags of filesystem
    f_flags: usize,
    /// Padding bytes reserved for future use
    f_spare: [usize; 4],
}

impl Statfs {
    fn validate(&self) -> Result<()> {
        if self.f_blocks < self.f_bfree || self.f_blocks < self.f_bavail {
            return_errno!(EINVAL, "invalid blocks");
        }
        if self.f_files < self.f_ffree {
            return_errno!(EINVAL, "invalid inodes");
        }
        if self.f_bsize == 0 || self.f_namelen == 0 || self.f_frsize == 0 {
            return_errno!(EINVAL, "invalid non-zero fields");
        }
        Ok(())
    }
}

impl TryFrom<FsInfo> for Statfs {
    type Error = errno::Error;

    fn try_from(info: FsInfo) -> Result<Self> {
        let statfs = if info.magic == rcore_fs_unionfs::UNIONFS_MAGIC
            || info.magic == rcore_fs_sefs::SEFS_MAGIC as usize
        {
            let mut host_statfs = {
                let host_rootfs_dir = unsafe { format!("{}{}", INSTANCE_DIR, "/run/mount/__ROOT") };
                fetch_host_statfs(&host_rootfs_dir)?
            };
            host_statfs.f_type = info.magic;
            host_statfs
        } else {
            Self {
                f_type: match info.magic {
                    // The "/dev" and "/dev/shm" are tmpfs on Linux, so we transform the
                    // magic number to TMPFS_MAGIC.
                    rcore_fs_ramfs::RAMFS_MAGIC | rcore_fs_devfs::DEVFS_MAGIC => {
                        const TMPFS_MAGIC: usize = 0x0102_1994;
                        TMPFS_MAGIC
                    }
                    val => val,
                },
                f_bsize: info.bsize,
                f_blocks: info.blocks,
                f_bfree: info.bfree,
                f_bavail: info.bavail,
                f_files: info.files,
                f_ffree: info.ffree,
                f_fsid: [0i32; 2],
                f_namelen: info.namemax,
                f_frsize: info.frsize,
                f_flags: 0,
                f_spare: [0usize; 4],
            }
        };
        Ok(statfs)
    }
}

impl From<Statfs> for FsInfo {
    fn from(statfs: Statfs) -> Self {
        Self {
            magic: statfs.f_type,
            bsize: statfs.f_bsize,
            frsize: statfs.f_frsize,
            blocks: statfs.f_blocks,
            bfree: statfs.f_bfree,
            bavail: statfs.f_bavail,
            files: statfs.f_files,
            ffree: statfs.f_ffree,
            namemax: statfs.f_namelen,
        }
    }
}

pub fn fetch_host_statfs(path: &str) -> Result<Statfs> {
    extern "C" {
        fn occlum_ocall_statfs(ret: *mut i32, path: *const i8, buf: *mut Statfs) -> sgx_status_t;
    }

    let mut ret: i32 = 0;
    let mut statfs: Statfs = Default::default();
    let host_dir = CString::new(path.as_bytes()).unwrap();
    let sgx_status = unsafe { occlum_ocall_statfs(&mut ret, host_dir.as_ptr(), &mut statfs) };
    assert!(sgx_status == sgx_status_t::SGX_SUCCESS);
    assert!(ret == 0 || libc::errno() == Errno::EINTR as i32);
    if ret != 0 {
        return_errno!(EINTR, "failed to get host statfs");
    }

    // do sanity check
    statfs.validate().expect("invalid statfs");
    Ok(statfs)
}

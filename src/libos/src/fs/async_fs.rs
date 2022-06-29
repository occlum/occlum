use super::*;

use async_sfs::AsyncSimpleFS;
use block_device::{BlockDevice, BLOCK_SIZE};
use sgx_disk::{HostDisk, IoUringDisk, SyncIoDisk};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::untrusted::path::PathEx;

pub const ASYNC_SFS_NAME: &str = "async_sfs";

pub async fn try_open_async_file(
    fs: &FsView,
    fs_path: &FsPath,
    flags: u32,
    mode: FileMode,
) -> Result<Option<AsyncFileHandle>> {
    let abs_path = fs.convert_fspath_to_abs(&fs_path)?;
    if !abs_path.trim_start_matches('/').starts_with(ASYNC_SFS_NAME) {
        return Ok(None);
    }
    let file_handle = fs.open_file(fs_path, flags, mode).await?;
    Ok(Some(file_handle))
}

/// Get or initilize the async sfs
pub async fn async_sfs() -> &'static Arc<dyn AsyncFileSystem> {
    loop {
        match STATE.compare_exchange(
            UNINITIALIZED,
            INITIALIZING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                let async_sfs = init_async_sfs().await.unwrap();
                unsafe {
                    ASYNC_SFS = Some(async_sfs);
                }
                STATE.store(INITIALIZED, Ordering::SeqCst);
                break;
            }
            Err(cur) if cur == INITIALIZED => break,
            Err(_) => {
                // current state is INITIALIZING, try again
            }
        }
    }
    // current state is INITIALIZED
    unsafe { ASYNC_SFS.as_ref().unwrap() }
}

/// Whether the async sfs is in initilized state
pub fn async_sfs_initilized() -> bool {
    let cur = STATE.load(Ordering::SeqCst);
    if cur == INITIALIZED {
        return true;
    } else if cur == UNINITIALIZED {
        return false;
    }

    while STATE.load(Ordering::SeqCst) == INITIALIZING {
        // spin loop to wait state becoming INITIALIZED
    }
    true
}

async fn init_async_sfs() -> Result<Arc<dyn AsyncFileSystem>> {
    let image_path = PathBuf::from(ASYNC_SFS_IMAGE_PATH);
    let async_sfs = if image_path.exists() {
        let sync_disk = SyncIoDisk::open(&image_path)?;
        AsyncSimpleFS::open(Arc::new(sync_disk)).await?
    } else {
        const GB: usize = 1024 * 1024 * 1024;
        let total_blocks = 8 * GB / BLOCK_SIZE;
        let sync_disk = SyncIoDisk::create_new(&image_path, total_blocks)?;
        AsyncSimpleFS::create(Arc::new(sync_disk)).await?
    };
    Ok(async_sfs as _)
}

const ASYNC_SFS_IMAGE_PATH: &str = "./run/async_sfs.image";
static mut ASYNC_SFS: Option<Arc<dyn AsyncFileSystem>> = None;
static STATE: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

use super::*;

use async_sfs::AsyncSimpleFS;
use block_device::{BlockDeviceAsFile, BLOCK_SIZE};
use page_cache::{impl_fixed_size_page_alloc, CachedDisk};
use sgx_disk::{HostDisk, IoUringDisk, SyncIoDisk};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::untrusted::path::PathEx;

pub const ASYNC_SFS_NAME: &str = "async_sfs";

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
    const GB: usize = 1024 * 1024 * 1024;
    // AsyncFsPageAlloc is a fixed-size allocator for page cache.
    impl_fixed_size_page_alloc! { AsyncFsPageAlloc, 1 * GB };

    const ASYNC_SFS_IMAGE_PATH: &str = "./run/async_sfs.image";
    let image_path = PathBuf::from(ASYNC_SFS_IMAGE_PATH);

    // Open or create the FS
    let async_sfs = if image_path.exists() {
        let cache_disk = {
            let sync_disk = SyncIoDisk::open(&image_path)?;
            CachedDisk::<AsyncFsPageAlloc>::new(Arc::new(sync_disk))?
        };
        AsyncSimpleFS::open(Arc::new(cache_disk)).await?
    } else {
        let cache_disk = {
            let total_blocks = 8 * GB / BLOCK_SIZE;
            let sync_disk = SyncIoDisk::create_new(&image_path, total_blocks)?;
            CachedDisk::<AsyncFsPageAlloc>::new(Arc::new(sync_disk))?
        };
        AsyncSimpleFS::create(Arc::new(cache_disk)).await?
    };
    Ok(async_sfs as _)
}

static mut ASYNC_SFS: Option<Arc<dyn AsyncFileSystem>> = None;
static STATE: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

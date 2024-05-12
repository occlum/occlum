//! Block device wrapper, currently used for the DevFS.
use super::{BlockDevice, BlockDeviceExt, RawDisk, GB};
use crate::fs::*;
use crate::prelude::*;
use crate::util::sgx::get_autokey;

use core::any::Any;
use rcore_fs::vfs::{self, FileType, INode, Metadata, Timespec};
use std::path::PathBuf;
use sworndisk_v2::{AeadKey, SwornDisk, BLOCK_SIZE};

lazy_static! {
    pub static ref SWORNDISK: RwLock<Option<Arc<SwornDisk<RawDisk>>>> = { RwLock::new(None) };
    pub static ref SWORNDISK_METADATA: RwLock<SwornDiskMeta> =
        { RwLock::new(SwornDiskMeta::default()) };
}

pub const DEV_SWORNDISK: &str = "sworndisk";

/// Block device wrapper.
pub struct DevDisk {
    disk: Arc<dyn BlockDevice>,
}

impl DevDisk {
    pub fn open_or_create(name: &str) -> Result<Self> {
        let disk: Arc<dyn BlockDevice> = match name {
            // Currently only support SwornDisk
            DEV_SWORNDISK => {
                let mut sworndisk_opt = SWORNDISK.write().unwrap();
                if let Some(sworndisk) = sworndisk_opt.as_ref() {
                    sworndisk.clone()
                } else {
                    let metadata = SWORNDISK_METADATA.read().unwrap();
                    if !metadata.is_setup {
                        return_errno!(EINVAL, "SwornDisk not set up");
                    }
                    let total_blocks = metadata.size / BLOCK_SIZE;
                    let image_path = {
                        let mut path = metadata.image_dir.clone();
                        path.push("sworndisk.image");
                        path
                    };
                    let raw_disk =
                        RawDisk::open_or_create(total_blocks, image_path.to_str().unwrap())?;
                    let root_key = metadata.root_key;

                    let sworndisk = Arc::new(
                        SwornDisk::open(raw_disk.clone(), root_key, None).unwrap_or_else(|_e| {
                            SwornDisk::create(raw_disk, root_key, None).unwrap()
                        }),
                    );
                    sworndisk_opt.insert(sworndisk.clone());
                    sworndisk
                }
            }
            _ => return_errno!(EINVAL, "Unrecognized block device"),
        };
        Ok(Self { disk })
    }

    pub fn disk(&self) -> Arc<dyn BlockDevice> {
        self.disk.clone()
    }
}

// Used for registering the disk to the DevFS
impl INode for DevDisk {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        if rw_args_block_aligned(offset, buf.len()) {
            self.disk
                .read_blocks((offset / BLOCK_SIZE) as _, &mut [buf])?;
        } else {
            self.disk.read_bytes(offset, buf)?;
        }

        Ok(buf.len())
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        if rw_args_block_aligned(offset, buf.len()) {
            self.disk.write_blocks((offset / BLOCK_SIZE) as _, &[buf])?;
        } else {
            self.disk.write_bytes(offset, buf)?;
        }

        Ok(buf.len())
    }

    fn metadata(&self) -> vfs::Result<Metadata> {
        Ok(Metadata {
            dev: 0,
            inode: 0xfe23_1d08,
            size: self.disk.total_bytes(),
            blk_size: BLOCK_SIZE,
            blocks: self.disk.total_blocks(),
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: FileType::File,
            mode: 0o666,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn sync_all(&self) -> vfs::Result<()> {
        self.disk.sync()?;
        Ok(())
    }

    fn sync_data(&self) -> vfs::Result<()> {
        self.disk.sync()?;
        Ok(())
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

fn rw_args_block_aligned(offset: usize, buf_len: usize) -> bool {
    if offset % BLOCK_SIZE == 0 && buf_len > 0 && buf_len % BLOCK_SIZE == 0 {
        true
    } else {
        false
    }
}

/// Metadata for SwornDisk.
#[derive(Debug)]
pub struct SwornDiskMeta {
    size: usize,
    root_key: AeadKey,
    image_dir: PathBuf,
    is_setup: bool,
}

impl Default for SwornDiskMeta {
    fn default() -> Self {
        Self {
            size: 0,
            root_key: AeadKey::default(),
            image_dir: PathBuf::from("run"),
            is_setup: false,
        }
    }
}

impl SwornDiskMeta {
    pub fn setup(
        disk_size: u64,
        user_key: &Option<sgx_key_128bit_t>,
        source_path: Option<&PathBuf>,
    ) -> Result<()> {
        let mut metadata = SWORNDISK_METADATA.write().unwrap();
        if metadata.is_setup {
            return_errno!(EEXIST, "SwornDisk already set up");
        };
        if disk_size < (5 * GB) as _ {
            return_errno!(EINVAL, "Disk size too small for SwornDisk");
        };
        metadata.size = disk_size as _;
        if let Some(source_path) = source_path {
            metadata.image_dir = source_path.clone();
        }
        let root_key = if let Some(user_key) = user_key {
            *user_key
        } else {
            get_autokey(&metadata.image_dir)?
        };
        metadata.root_key = AeadKey::from(root_key);
        metadata.is_setup = true;
        Ok(())
    }

    pub fn is_setup() -> bool {
        SWORNDISK_METADATA.read().unwrap().is_setup
    }
}

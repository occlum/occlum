use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sgxfs::{OpenOptions as PfsOpenOptions, SgxFile as PfsFile};

use super::{PfsDisk, PFS_INNER_OFFSET};
use crate::prelude::*;

/// Options that are used to configure how a PFS disk is opened.
pub struct OpenOptions {
    read: bool,
    write: bool,
    clear: bool,
    create: bool,
    create_new: bool,
    total_blocks: Option<usize>,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    pub fn new() -> Self {
        Self {
            read: false,
            write: false,
            clear: false,
            create: false,
            create_new: false,
            total_blocks: None,
        }
    }

    /// Sets the option for read access.
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    /// Sets the option to create a new PFS disk, or open it if it already exists.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    /// Sets the option to create a new PFS disk, failing if it already exists.
    ///
    /// If `.create_new(true)` is set, then `.create()` is ignored.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    /// Sets the option for clearing the content of the PFS disk, if it already
    /// exists.
    pub fn clear(&mut self, clear: bool) -> &mut Self {
        self.clear = clear;
        self
    }

    /// Sets the option for the size of the host disk in blocks.
    pub fn total_blocks(&mut self, total_blocks: usize) -> &mut Self {
        self.total_blocks = Some(total_blocks);
        self
    }

    /// Opens a PFS disk at `path` with the options specified by `self`.
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<PfsDisk> {
        if !self.read && !self.write {
            return Err(errno!(EINVAL, "the disk must be readable or writable"));
        }
        if (self.create || self.create_new) && self.total_blocks.is_none() {
            return Err(errno!(EINVAL, "the disk size must be given"));
        }

        // Open or create the PFS file
        let mut file_exists = false;
        let mut pfs_file = {
            let mut pfs_file_opt = None;

            // If not create_new, then we should first try to open it
            if !self.create_new {
                pfs_file_opt = match open_pfs_file(path.as_ref()) {
                    Ok(file) => {
                        file_exists = true;
                        Some(file)
                    }
                    Err(e) if e.errno() == Errno::ENOENT => None,
                    Err(e) => return Err(e),
                };
            }

            // If we haven't opened one, then create it
            if pfs_file_opt.is_none() && (self.create || self.create_new) {
                pfs_file_opt = Some(create_pfs_file(path.as_ref())?);
            }

            match pfs_file_opt {
                Some(pfs_file) => pfs_file,
                None => return Err(errno!(ENOENT, "file not found")),
            }
        };

        // Get the current length of the PFS file
        let old_len = if file_exists {
            let file_len = pfs_file.seek(SeekFrom::End(0)).unwrap() as usize;
            if file_len < (PFS_INNER_OFFSET + BLOCK_SIZE) {
                return Err(errno!(EINVAL, "file size is too small"));
            }
            if (file_len - PFS_INNER_OFFSET) % BLOCK_SIZE != 0 {
                return Err(errno!(EINVAL, "file size is not aligned"));
            }
            file_len
        } else {
            0
        };

        // Determine the total blocks
        let total_blocks = if let Some(total_blocks) = self.total_blocks {
            let new_len = PFS_INNER_OFFSET + total_blocks * BLOCK_SIZE;
            if old_len > new_len {
                return Err(errno!(EEXIST, "cannot shrink an existed disk"));
            }
            write_zeros(&mut pfs_file, old_len, new_len);
            total_blocks
        } else {
            debug_assert!(file_exists);
            (old_len - PFS_INNER_OFFSET) / BLOCK_SIZE
        };

        // Ensure all existing data are zeroed if clear is required
        if self.clear {
            write_zeros(&mut pfs_file, 0, old_len);
        }

        let pfs_disk = PfsDisk {
            file: Mutex::new(pfs_file),
            path: path.as_ref().to_path_buf(),
            total_blocks,
            can_read: self.read,
            can_write: self.write,
        };
        Ok(pfs_disk)
    }
}

/// Open an existing PFS file with read and write permissions.
fn open_pfs_file<P: AsRef<Path>>(path: P) -> Result<PfsFile> {
    PfsOpenOptions::new()
        .read(true)
        .update(true)
        .open(path.as_ref())
        .map_err(|e| e.into())
}

/// Create a PFS file with read and write permissions. The length of the
/// opened file is zero.
fn create_pfs_file<P: AsRef<Path>>(path: P) -> Result<PfsFile> {
    PfsOpenOptions::new()
        .write(true)
        .update(true)
        .open(path.as_ref())
        .map_err(|e| e.into())
}

fn write_zeros(pfs_file: &mut PfsFile, begin: usize, end: usize) {
    debug_assert!(begin <= end);

    const ZEROS: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];

    pfs_file.seek(SeekFrom::Start(begin as u64)).unwrap();
    let mut remain = end - begin;
    while remain > 0 {
        let buf_len = remain.min(ZEROS.len());
        pfs_file.write(&ZEROS[0..buf_len]).unwrap();
        remain -= buf_len;
    }
}

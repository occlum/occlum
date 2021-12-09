use std::fs::{File, OpenOptions as FileOpenOptions};
use std::marker::PhantomData;
use std::path::Path;

use crate::prelude::*;
use crate::HostDisk;

/// Options that are used to configure how a disk is opened.
///
/// This builder exposes the ability to configure how a host disk is opened and
/// what operations are permitted on the open host disk.
pub struct OpenOptions<D> {
    read: bool,
    write: bool,
    clear: bool,
    create: bool,
    create_new: bool,
    num_blocks: Option<usize>,
    _phantom: PhantomData<D>,
}

impl<D: HostDisk + Sized> OpenOptions<D> {
    /// Creates a blank new set of options ready for configuration.
    pub fn new() -> Self {
        Self {
            read: false,
            write: false,
            clear: false,
            create: false,
            create_new: false,
            num_blocks: None,
            _phantom: PhantomData,
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

    /// Sets the option to create a new host disk, or open it if it already exists.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    /// Sets the option to create a new host disk, failing if it already exists.
    ///
    /// If `.create_new(true)` is set, then `.create()` is ignored.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    /// Sets the option for clearing the content of the host disk, if it already
    /// exists.
    pub fn clear(&mut self, clear: bool) -> &mut Self {
        self.clear = clear;
        self
    }

    /// Sets the option for the size of the host disk in blocks.
    pub fn num_blocks(&mut self, num_blocks: usize) -> &mut Self {
        self.num_blocks = Some(num_blocks);
        self
    }

    /// Opens a host disk at `path` with the options specified by `self`.
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<D> {
        // Try to capture input errors before creating a file on the host
        let maybe_new_file = self.create || self.create_new;
        let num_blocks = self.num_blocks.unwrap_or(0);
        if maybe_new_file && num_blocks == 0 {
            return Err(errno!(
                EINVAL,
                "a new host disk must be given a non-zero size"
            ));
        }
        if num_blocks.checked_mul(block_device::BLOCK_SIZE).is_none() {
            return Err(errno!(EOVERFLOW, "the disk size is too large"));
        }

        // Open or create a file on the host
        let file = FileOpenOptions::new()
            .read(self.read)
            .write(self.write)
            .create(self.create)
            .create_new(self.create_new)
            .truncate(self.clear)
            .open(path)?;

        // If the size of the disk is specified, we set the length regradless
        // of the file is new or existing.
        if let Some(num_blocks) = self.num_blocks {
            let file_len = num_blocks * block_device::BLOCK_SIZE;
            file.set_len(file_len as u64)
                .expect("an error from set_len at this stage is hard to recover");
        }

        D::from_options_and_file(self, file)
    }
}

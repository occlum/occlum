use block_device::BlockDevice;
use fs::File;
use std::path::Path;

use super::OpenOptions;
use crate::prelude::*;

/// A host disk is a block device backed by a file on the host Linux.
pub trait HostDisk: BlockDevice {
    /// Returns a new `OpenOptions` object.
    ///
    /// This is the most flexible way to create host disks. For common use cases,
    /// try other convenient constructor APIs.
    fn with_options() -> OpenOptions<Self>
    where
        Self: Sized,
    {
        OpenOptions::<Self>::new()
    }

    /// Open a host disk backed by an existing file on the host Linux.
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>
    where
        Self: Sized,
    {
        OpenOptions::new().read(true).write(true).open(path)
    }

    /// Open a host disk backed by opening or creating a new file on the host Linux.
    ///
    /// If there exists a file on the given path, then the file must have the
    /// exact number of blocks as specified by the method. And all blocks of
    /// the file will be zeroed.
    fn create<P: AsRef<Path>>(path: P, total_blocks: usize) -> Result<Self>
    where
        Self: Sized,
    {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .clear(true)
            .total_blocks(total_blocks)
            .open(path)
    }

    /// Open a host disk backed by creating a new file on the host Linux.
    ///
    /// If there exists a file on the given path, then an error will be returned.
    fn create_new<P: AsRef<Path>>(path: P, total_blocks: usize) -> Result<Self>
    where
        Self: Sized,
    {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .total_blocks(total_blocks)
            .open(path)
    }

    /// Returns the path of the host file which stores the content of the disk.
    fn path(&self) -> &Path;

    /// Create a host disk according to the options and backed by the file.
    ///
    /// This method is supposed to use directly. So we hide its API document.
    /// Users should use `Self::with_options` instead.
    #[doc(hidden)]
    fn from_options_and_file(options: &OpenOptions<Self>, file: File, path: &Path) -> Result<Self>
    where
        Self: Sized;
}

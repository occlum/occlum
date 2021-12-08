use block_device::BlockDevice;
use std::fs::File;

use crate::prelude::*;
use crate::OpenOptions;

/// A virtual disk backed by a file on the host Linux.
pub trait HostDisk: BlockDevice + Sized {
    /// Create a host disk according to the options and backed by the file.
    fn new(options: &OpenOptions<Self>, file: File) -> Result<Self>;
}

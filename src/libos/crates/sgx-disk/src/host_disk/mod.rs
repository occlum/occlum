//! Host disks are untrusted virtual disks that are backed by a file on the host.
//!
//! There are two types of host disks.
//! * `SyncIoDisk` is a disk that uses normal sync I/O operations.
//! * `IoUringDisk` is a disk that uses async I/O operations via io_uring.

mod host_disk;
mod io_uring_disk;
mod open_options;
mod sync_io_disk;

pub use self::host_disk::HostDisk;
pub use self::io_uring_disk::{IoUringDisk, IoUringProvider};
pub use self::open_options::OpenOptions;
pub use self::sync_io_disk::SyncIoDisk;

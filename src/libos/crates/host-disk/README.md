# host-disk

This crate implements two virtual disks, `SyncIoDisk` and `IoUringDisk`, 
which are backed by files on the host Linux kernel. Both of them implements
the `BlockDevice` trait of the block-device crate.

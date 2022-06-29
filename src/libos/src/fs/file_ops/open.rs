use super::async_fs::try_open_async_file;
use super::builtin_disk::try_open_disk;
use super::*;
use crate::fs::DiskFile;

pub async fn do_openat(fs_path: &FsPath, flags: u32, mode: FileMode) -> Result<FileDesc> {
    debug!(
        "openat: fs_path: {:?}, flags: {:#o}, mode: {:#o}",
        fs_path, flags, mode
    );

    let current = current!();
    let fs = current.fs();
    let masked_mode = mode & !current.process().umask();

    let file_ref = if let Some(disk_file) = try_open_disk(&fs, fs_path)? {
        FileRef::new_disk(disk_file)
    } else if let Some(async_file_handle) =
        try_open_async_file(&fs, fs_path, flags, masked_mode).await?
    {
        FileRef::new_async_file_handle(async_file_handle)
    } else {
        let inode_file = fs.open_file_sync(&fs_path, flags, masked_mode)?;
        FileRef::new_inode(inode_file)
    };

    let fd = {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        current.add_file(file_ref, creation_flags.must_close_on_spawn())
    };
    Ok(fd)
}

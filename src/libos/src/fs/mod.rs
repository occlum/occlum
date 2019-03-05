use super::*;
use prelude::*;
use std::sgxfs as fs_impl;
use {process, std};

mod file;
mod file_table;
mod pipe;
mod inode_file;

pub use self::file::{File, FileRef, SgxFile, StdinFile, StdoutFile};
pub use self::file_table::{FileDesc, FileTable};
pub use self::pipe::Pipe;
pub use self::inode_file::{INodeFile, ROOT_INODE};
use rcore_fs::vfs::{FsError, FileType, INode};
use self::inode_file::OpenOptions;

// TODO: use the type defined in Rust libc.
//
// However, off_t is defined as u64 in the current Rust SGX SDK, which is
// wrong (see issue https://github.com/baidu/rust-sgx-sdk/issues/46)
#[allow(non_camel_case_types)]
pub type off_t = i64;

pub fn do_open(path: &str, flags: u32, mode: u32) -> Result<FileDesc, Error> {
    let flags = OpenFlags::from_bits_truncate(flags);
    info!("open: path: {:?}, flags: {:?}, mode: {:#o}", path, flags, mode);

    let inode =
        if flags.contains(OpenFlags::CREATE) {
            let (dir_path, file_name) = split_path(&path);
            let dir_inode = ROOT_INODE.lookup(dir_path)?;
            match dir_inode.find(file_name) {
                Ok(file_inode) => {
                    if flags.contains(OpenFlags::EXCLUSIVE) {
                        return Err(Error::new(EEXIST, "file exists"));
                    }
                    file_inode
                },
                Err(FsError::EntryNotFound) => {
                    dir_inode.create(file_name, FileType::File, mode)?
                }
                Err(e) => return Err(Error::from(e)),
            }
        } else {
            ROOT_INODE.lookup(&path)?
        };

    let file_ref: Arc<Box<File>> = Arc::new(Box::new(
        INodeFile::open(inode, flags.to_options())?
    ));

    let fd = {
        let current_ref = process::get_current();
        let mut current = current_ref.lock().unwrap();
        let close_on_spawn = flags.contains(OpenFlags::CLOEXEC);
        current.get_files_mut().put(file_ref, close_on_spawn)
    };
    Ok(fd)
}

pub fn do_write(fd: FileDesc, buf: &[u8]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().get(fd)?;
    file_ref.write(buf)
}

pub fn do_read(fd: FileDesc, buf: &mut [u8]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().get(fd)?;
    file_ref.read(buf)
}

pub fn do_writev<'a, 'b>(fd: FileDesc, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().get(fd)?;
    file_ref.writev(bufs)
}

pub fn do_readv<'a, 'b>(fd: FileDesc, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().get(fd)?;
    file_ref.readv(bufs)
}

pub fn do_lseek<'a, 'b>(fd: FileDesc, offset: SeekFrom) -> Result<off_t, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().get(fd)?;
    file_ref.seek(offset)
}

pub fn do_close(fd: FileDesc) -> Result<(), Error> {
    let current_ref = process::get_current();
    let mut current_process = current_ref.lock().unwrap();
    let file_table = current_process.get_files_mut();
    file_table.del(fd)?;
    Ok(())
}

pub fn do_pipe2(flags: u32) -> Result<[FileDesc; 2], Error> {
    let flags = OpenFlags::from_bits_truncate(flags);
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let pipe = Pipe::new()?;

    let mut file_table = current.get_files_mut();
    let close_on_spawn = flags.contains(OpenFlags::CLOEXEC);
    let reader_fd = file_table.put(Arc::new(Box::new(pipe.reader)), close_on_spawn);
    let writer_fd = file_table.put(Arc::new(Box::new(pipe.writer)), close_on_spawn);
    Ok([reader_fd, writer_fd])
}

pub fn do_dup(old_fd: FileDesc) -> Result<FileDesc, Error> {
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let file_table = current.get_files_mut();
    let file = file_table.get(old_fd)?;
    let new_fd = file_table.put(file, false);
    Ok(new_fd)
}

pub fn do_dup2(old_fd: FileDesc, new_fd: FileDesc) -> Result<FileDesc, Error> {
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let file_table = current.get_files_mut();
    let file = file_table.get(old_fd)?;
    if old_fd != new_fd {
        file_table.put_at(new_fd, file, false);
    }
    Ok(new_fd)
}

pub fn do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32) -> Result<FileDesc, Error> {
    let flags = OpenFlags::from_bits_truncate(flags);
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let file_table = current.get_files_mut();
    let file = file_table.get(old_fd)?;
    if old_fd == new_fd {
        return errno!(EINVAL, "old_fd must not be equal to new_fd");
    }
    let close_on_spawn = flags.contains(OpenFlags::CLOEXEC);
    file_table.put_at(new_fd, file, close_on_spawn);
    Ok(new_fd)
}

pub fn do_sync() -> Result<(), Error> {
    unsafe {
        ocall_sync();
    }
    Ok(())
}

extern "C" {
    fn ocall_sync() -> sgx_status_t;
}

/// Split a `path` str to `(base_path, file_name)`
fn split_path(path: &str) -> (&str, &str) {
    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let file_name = split.next().unwrap();
    let dir_path = split.next().unwrap_or(".");
    (dir_path, file_name)
}

bitflags! {
    struct OpenFlags: u32 {
        /// read only
        const RDONLY = 0;
        /// write only
        const WRONLY = 1;
        /// read write
        const RDWR = 2;
        /// create file if it does not exist
        const CREATE = 1 << 6;
        /// error if CREATE and the file exists
        const EXCLUSIVE = 1 << 7;
        /// truncate file upon open
        const TRUNCATE = 1 << 9;
        /// append on each write
        const APPEND = 1 << 10;
        /// close on exec
        const CLOEXEC = 1 << 19;
    }
}

impl OpenFlags {
    fn readable(&self) -> bool {
        let b = self.bits() & 0b11;
        b == OpenFlags::RDONLY.bits() || b == OpenFlags::RDWR.bits()
    }
    fn writable(&self) -> bool {
        let b = self.bits() & 0b11;
        b == OpenFlags::WRONLY.bits() || b == OpenFlags::RDWR.bits()
    }
    fn to_options(&self) -> OpenOptions {
        OpenOptions {
            read: self.readable(),
            write: self.writable(),
            append: self.contains(OpenFlags::APPEND),
        }
    }
}

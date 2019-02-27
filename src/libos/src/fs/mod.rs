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

pub const O_RDONLY: u32 = 0x00000000;
pub const O_WRONLY: u32 = 0x00000001;
pub const O_RDWR: u32 = 0x00000002;
pub const O_CREAT: u32 = 0x00000040;
pub const O_TRUNC: u32 = 0x00000200;
pub const O_APPEND: u32 = 0x00000400;
pub const O_CLOEXEC: u32 = 0x00080000;

// TODO: use the type defined in Rust libc.
//
// However, off_t is defined as u64 in the current Rust SGX SDK, which is
// wrong (see issue https://github.com/baidu/rust-sgx-sdk/issues/46)
#[allow(non_camel_case_types)]
pub type off_t = i64;

pub fn do_open(path: &str, flags: u32, mode: u32) -> Result<FileDesc, Error> {
    let open_options = {
        let mut open_options = fs_impl::OpenOptions::new();

        if ((flags & O_TRUNC) != 0 || (flags & O_CREAT) != 0) {
            open_options.write(true);
        } else {
            open_options.read(true);
        }
        open_options.update(true).binary(true);

        open_options
    };

    let mut sgx_file = {
        let key: sgx_key_128bit_t = [0 as uint8_t; 16];
        // TODO: what if two processes open the same underlying SGX file?
        let sgx_file = open_options
            .open_ex(path, &key)
            .map_err(|e| (Errno::ENOENT, "Failed to open the SGX-protected file"))?;
        Arc::new(SgxMutex::new(sgx_file))
    };

    let is_readable = (flags & O_WRONLY) == 0;
    let is_writable = (flags & O_WRONLY != 0) || (flags & O_RDWR != 0);
    let is_append = (flags & O_APPEND != 0);
    let file_ref: Arc<Box<File>> = Arc::new(Box::new(SgxFile::new(
        sgx_file,
        is_readable,
        is_writable,
        is_append,
    )?));

    let fd = {
        let current_ref = process::get_current();
        let mut current = current_ref.lock().unwrap();
        let close_on_spawn = flags & O_CLOEXEC != 0;
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
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let pipe = Pipe::new()?;

    let mut file_table = current.get_files_mut();
    let close_on_spawn = flags & O_CLOEXEC != 0;
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
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let file_table = current.get_files_mut();
    let file = file_table.get(old_fd)?;
    if old_fd == new_fd {
        return errno!(EINVAL, "old_fd must not be equal to new_fd");
    }
    let close_on_spawn = flags & O_CLOEXEC != 0;
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

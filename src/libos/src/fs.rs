use prelude::*;
use {std, file, file_table, process};
use file::{File, SgxFile};
use file_table::{FileDesc};

use std::sgxfs as fs_impl;

pub const O_RDONLY : u32 = 0x00000000;
pub const O_WRONLY : u32 = 0x00000001;
pub const O_RDWR   : u32 = 0x00000002;
pub const O_CREAT  : u32 = 0x00000040;
pub const O_TRUNC  : u32 = 0x00000200;
pub const O_APPEND : u32 = 0x00000400;

pub fn do_open(path: &str, flags: u32, mode: u32) -> Result<FileDesc, Error> {
    let open_options = {
        let mut open_options = fs_impl::OpenOptions::new();

        if ((flags & O_TRUNC) != 0 || (flags & O_CREAT) != 0) {
            open_options.write(true);
        }
        else {
            open_options.read(true);
        }
        open_options.update(true).binary(true);

        open_options
    };

    let mut sgx_file = {
        let key : sgx_key_128bit_t = [0 as uint8_t; 16];
        let sgx_file = open_options.open_ex(path, &key)
            .map_err(|e| (Errno::ENOENT, "Failed to open the SGX-protected file") )?;
        Arc::new(SgxMutex::new(sgx_file))
    };

    let is_readable = (flags & O_RDONLY != 0) || (flags & O_RDWR != 0);
    let is_writable = (flags & O_WRONLY != 0) || (flags & O_RDWR != 0);
    let is_append = (flags & O_APPEND != 0);
    let file_ref : Arc<Box<File>> = Arc::new(Box::new(
            SgxFile::new(sgx_file, is_readable, is_writable, is_append)));

    let current_ref = process::get_current();
    let mut current_process = current_ref.lock().unwrap();
    let fd = current_process.file_table.put(file_ref);

    Ok(fd)
}

pub fn do_write(fd: FileDesc, buf: &[u8]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.file_table.get(fd)
        .ok_or_else(|| Error::new(Errno::EBADF, "Invalid file descriptor [do_write]"))?;
    file_ref.write(buf)
}

pub fn do_read(fd: FileDesc, buf: &mut [u8]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.file_table.get(fd)
        .ok_or_else(|| Error::new(Errno::EBADF, "Invalid file descriptor [do_read]"))?;
    file_ref.read(buf)
}

pub fn do_writev<'a, 'b>(fd: FileDesc, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.file_table.get(fd)
        .ok_or_else(|| Error::new(Errno::EBADF, "Invalid file descriptor [do_write]"))?;
    file_ref.writev(bufs)
}

pub fn do_readv<'a, 'b>(fd: FileDesc, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.file_table.get(fd)
        .ok_or_else(|| Error::new(Errno::EBADF, "Invalid file descriptor [do_read]"))?;
    file_ref.readv(bufs)
}

pub fn do_close(fd: FileDesc) -> Result<(), Error> {
    let current_ref = process::get_current();
    let mut current_process = current_ref.lock().unwrap();
    let file_table = &mut current_process.file_table;
    match file_table.del(fd) {
        Some(_) => Ok(()),
        None => Err(Error::new(Errno::EBADF, "Invalid file descriptor [do_close]")),
    }
}

use prelude::*;
use {std, file, file_table, fs, process};
use std::ffi::{CStr, CString};
// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

#[allow(non_camel_case_types)]
pub struct iovec_t {
    base: *const c_void,
    len: size_t,
}

fn check_ptr_from_user<T>(user_ptr: *const T) -> Result<(), Error> {
    Ok(())
}

fn check_mut_ptr_from_user<T>(user_ptr: *mut T) -> Result<(), Error> {
    Ok(())
}

fn check_array_from_user<T>(user_buf: *const T, count: usize) -> Result<(), Error> {
    Ok(())
}

fn check_mut_array_from_user<T>(user_buf: *mut T, count: usize) -> Result<(), Error> {
    Ok(())
}

fn clone_string_from_user_safely(user_ptr: *const c_char)
    -> Result<String, Error>
{
    check_ptr_from_user(user_ptr)?;
    let string = unsafe {
        CStr::from_ptr(user_ptr).to_string_lossy().into_owned()
    };
    Ok(string)
}

fn clone_cstrings_from_user_safely(user_ptr: *const *const c_char)
    -> Result<Vec<CString>, Error>
{
    let cstrings = Vec::new();
    Ok(cstrings)
}


fn do_read(fd: c_int, buf: *mut c_void, size: size_t)
    -> Result<size_t, Error>
{
    let fd = fd as file_table::FileDesc;
    let safe_buf = {
        let buf = buf as *mut u8;
        let size = size as usize;
        check_mut_array_from_user(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    fs::do_read(fd, safe_buf)
}

fn do_write(fd: c_int, buf: *const c_void, size: size_t)
    -> Result<size_t, Error>
{
    let fd = fd as file_table::FileDesc;
    let safe_buf = {
        let buf = buf as *mut u8;
        let size = size as usize;
        check_array_from_user(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    fs::do_write(fd, safe_buf)
}

fn do_writev(fd: c_int, iov: *const iovec_t, count: c_int)
    -> Result<size_t, Error>
{
    let fd = fd as file_table::FileDesc;

    let count = {
        if count < 0 {
            return Err(Error::new(Errno::EINVAL, "Invalid count of iovec"));
        }
        count as usize
    };

    check_array_from_user(iov, count);
    let bufs_vec = {
        let mut bufs_vec = Vec::with_capacity(count);
        for iov_i in 0..count {
            let iov_ptr = unsafe { iov.offset(iov_i as isize) };
            let iov = unsafe { &*iov_ptr };
            let buf = unsafe {
                std::slice::from_raw_parts(iov.base as * const u8, iov.len)
            };
            bufs_vec[iov_i] = buf;
        }
        bufs_vec
    };
    let bufs = &bufs_vec[..];

    fs::do_writev(fd, bufs)
}

fn do_readv(fd: c_int, iov: *mut iovec_t, count: c_int)
    -> Result<size_t, Error>
{
    let fd = fd as file_table::FileDesc;

    let count = {
        if count < 0 {
            return Err(Error::new(Errno::EINVAL, "Invalid count of iovec"));
        }
        count as usize
    };

    check_array_from_user(iov, count);
    let mut bufs_vec = {
        let mut bufs_vec = Vec::with_capacity(count);
        for iov_i in 0..count {
            let iov_ptr = unsafe { iov.offset(iov_i as isize) };
            let iov = unsafe { &*iov_ptr };
            let buf = unsafe {
                std::slice::from_raw_parts_mut(iov.base as * mut u8, iov.len)
            };
            bufs_vec[iov_i] = buf;
        }
        bufs_vec
    };
    let bufs = &mut bufs_vec[..];

    fs::do_readv(fd, bufs)
}


pub fn do_lseek(fd: c_int, offset: off_t, whence: c_int) -> Result<off_t, Error>
{

    let fd = fd as file_table::FileDesc;

    let seek_from = match whence {
        0 => { // SEEK_SET
            if offset < 0 {
                return Err(Error::new(Errno::EINVAL, "Invalid offset"));
            }
            SeekFrom::Start(offset as u64)
        }
        1 => { // SEEK_CUR
            SeekFrom::Current(offset)
        }
        2 => { // SEEK_END
            SeekFrom::End(offset)
        }
        _ => {
            return Err(Error::new(Errno::EINVAL, "Invalid whence"));
        }
    };

    fs::do_lseek(fd, seek_from)
}


#[no_mangle]
pub extern "C" fn occlum_open(path_buf: * const c_char, flags: c_int, mode: c_int) -> c_int {
    let path = unsafe {
        CStr::from_ptr(path_buf as * const i8).to_string_lossy().into_owned()
    };
    match fs::do_open(&path, flags as u32, mode as u32) {
        Ok(fd) => {
            fd as c_int
        },
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_close(fd: c_int) -> c_int {
    match fs::do_close(fd as file_table::FileDesc) {
        Ok(()) => {
            0
        },
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_read(fd: c_int, buf: * mut c_void, size: size_t) -> ssize_t {
    match do_read(fd, buf, size) {
        Ok(read_len) => {
            read_len as ssize_t
        },
        Err(e) => {
            e.errno.as_retval() as ssize_t
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_write(fd: c_int, buf: * const c_void, size: size_t) -> ssize_t {
    match do_write(fd, buf, size) {
        Ok(write_len) => {
            write_len as ssize_t
        },
        Err(e) => {
            e.errno.as_retval() as ssize_t
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_readv(fd: c_int, iov: * mut iovec_t, count: c_int) -> ssize_t {
    match do_readv(fd, iov, count) {
        Ok(read_len) => {
            read_len as ssize_t
        },
        Err(e) => {
            e.errno.as_retval() as ssize_t
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_writev(fd: c_int, iov: * const iovec_t, count: c_int) -> ssize_t {
    match do_writev(fd, iov, count) {
        Ok(write_len) => {
            write_len as ssize_t
        },
        Err(e) => {
            e.errno.as_retval() as ssize_t
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t {
    match do_lseek(fd, offset, whence) {
        Ok(ret) => {
            ret
        },
        Err(e) => {
            -1 as off_t // this special value indicates error
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_getpid() -> c_uint
{
    process::do_getpid()
}

#[no_mangle]
pub extern "C" fn occlum_exit(status: i32)
{
    process::do_exit(status);
}

#[no_mangle]
pub extern "C" fn occlum_unknown(num: u32)
{
    println!("[WARNING] Unknown syscall (num = {})", num);
}

fn do_spawn(child_pid_ptr: *mut c_uint,
            path: *const c_char,
            argv: *const *const c_char,
            envp: *const *const c_char)
    -> Result<(), Error>
{
    check_mut_ptr_from_user(child_pid_ptr)?;
    let path = clone_string_from_user_safely(path)?;
    let argv = clone_cstrings_from_user_safely(argv)?;
    let envp = clone_cstrings_from_user_safely(envp)?;

    let child_pid = process::do_spawn(&path, &argv, &envp)?;

    unsafe { *child_pid_ptr = child_pid };
    Ok(())
}

#[no_mangle]
pub extern "C" fn occlum_spawn(
    child_pid: *mut c_uint, path: *const c_char,
    argv: *const *const c_char, envp: *const *const c_char) -> c_int
{
    match do_spawn(child_pid, path, argv, envp) {
        Ok(()) => 0,
        Err(e) => { e.errno.as_retval() }
    }
}

#[no_mangle]
pub extern "C" fn occlum_wait4(child_pid: c_int, _exit_code: *mut c_int,
    options: c_int/*, rusage: *mut Rusage*/) -> c_int
{
    match process::do_wait4(child_pid as u32) {
        Ok(exit_code) => unsafe {
            *_exit_code = exit_code;
            0
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

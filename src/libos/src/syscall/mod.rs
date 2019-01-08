use super::*;
use prelude::*;
use {std, fs, process, vm};
use std::{ptr};
use std::ffi::{CStr, CString};
use fs::{off_t, FileDesc};
use vm::{VMAreaFlags, VMResizeOptions};
use process::{pid_t, ChildProcessFilter, FileAction};
use time::{timeval_t};
use util::mem_util::from_user::{*};
// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

#[allow(non_camel_case_types)]
pub struct iovec_t {
    base: *const c_void,
    len: size_t,
}


/*
 * This Rust-version of fdop correspond to the C-version one in Occlum.
 * See <path_to_musl_libc>/src/process/fdop.h.
 */
const FDOP_CLOSE : u32 = 1;
const FDOP_DUP2 : u32 = 2;
const FDOP_OPEN : u32 = 3;

#[repr(C)]
#[derive(Debug)]
pub struct FdOp {
    // We actually switch the prev and next fields in the libc definition.
    prev: *const FdOp,
    next: *const FdOp,
    cmd: u32,
    fd: u32,
    srcfd: u32,
    oflag: u32,
    mode: u32,
    path: *const u8,
}

fn clone_file_actions_safely(fdop_ptr: *const FdOp)
    -> Result<Vec<FileAction>, Error>
{
    let mut file_actions = Vec::new();

    let mut fdop_ptr = fdop_ptr;
    while fdop_ptr != ptr::null() {
        check_ptr(fdop_ptr)?;
        let fdop = unsafe { &*fdop_ptr };

        let file_action = match fdop.cmd {
            FDOP_CLOSE => {
                FileAction::Close(fdop.fd)
            },
            FDOP_DUP2 => {
                FileAction::Dup2(fdop.srcfd, fdop.fd)
            },
            FDOP_OPEN => {
                return errno!(EINVAL, "Not implemented");
            },
            _ => {
                return errno!(EINVAL, "Unknown file action command");
            },
        };
        file_actions.push(file_action);

        fdop_ptr = fdop.next;
    }

    Ok(file_actions)
}

fn do_spawn(child_pid_ptr: *mut c_uint,
            path: *const c_char,
            argv: *const *const c_char,
            envp: *const *const c_char,
            fdop_list: *const FdOp,
            )
    -> Result<(), Error>
{
    check_mut_ptr(child_pid_ptr)?;
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let argv = clone_cstrings_safely(argv)?;
    let envp = clone_cstrings_safely(envp)?;
    let file_actions = clone_file_actions_safely(fdop_list)?;
    let parent = process::get_current();

    let child_pid = process::do_spawn(&path, &argv, &envp, &file_actions, &parent)?;

    unsafe { *child_pid_ptr = child_pid };
    Ok(())
}

fn do_read(fd: c_int, buf: *mut c_void, size: size_t)
    -> Result<size_t, Error>
{
    let fd = fd as FileDesc;
    let safe_buf = {
        let buf = buf as *mut u8;
        let size = size as usize;
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    fs::do_read(fd, safe_buf)
}

fn do_write(fd: c_int, buf: *const c_void, size: size_t)
    -> Result<size_t, Error>
{
    let fd = fd as FileDesc;
    let safe_buf = {
        let buf = buf as *mut u8;
        let size = size as usize;
        check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    fs::do_write(fd, safe_buf)
}

fn do_writev(fd: c_int, iov: *const iovec_t, count: c_int)
    -> Result<size_t, Error>
{
    let fd = fd as FileDesc;

    let count = {
        if count < 0 {
            return Err(Error::new(Errno::EINVAL, "Invalid count of iovec"));
        }
        count as usize
    };

    check_array(iov, count);
    let bufs_vec = {
        let mut bufs_vec = Vec::with_capacity(count);
        for iov_i in 0..count {
            let iov_ptr = unsafe { iov.offset(iov_i as isize) };
            let iov = unsafe { &*iov_ptr };
            let buf = unsafe {
                std::slice::from_raw_parts(iov.base as * const u8, iov.len)
            };
            bufs_vec.push(buf);
        }
        bufs_vec
    };
    let bufs = &bufs_vec[..];

    fs::do_writev(fd, bufs)
}

fn do_readv(fd: c_int, iov: *mut iovec_t, count: c_int)
    -> Result<size_t, Error>
{
    let fd = fd as FileDesc;

    let count = {
        if count < 0 {
            return Err(Error::new(Errno::EINVAL, "Invalid count of iovec"));
        }
        count as usize
    };

    check_array(iov, count);
    let mut bufs_vec = {
        let mut bufs_vec = Vec::with_capacity(count);
        for iov_i in 0..count {
            let iov_ptr = unsafe { iov.offset(iov_i as isize) };
            let iov = unsafe { &*iov_ptr };
            let buf = unsafe {
                std::slice::from_raw_parts_mut(iov.base as * mut u8, iov.len)
            };
            bufs_vec.push(buf);
        }
        bufs_vec
    };
    let bufs = &mut bufs_vec[..];

    fs::do_readv(fd, bufs)
}


pub fn do_lseek(fd: c_int, offset: off_t, whence: c_int) -> Result<off_t, Error>
{

    let fd = fd as FileDesc;

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

fn do_mmap(addr: *const c_void, size: size_t, prot: c_int,
           flags: c_int, fd: c_int, offset: off_t)
    -> Result<*const c_void, Error>
{
    let addr = addr as usize;
    let size = size as usize;
    let flags = VMAreaFlags(prot as u32);
    vm::do_mmap(addr, size, flags).map(|ret_addr| ret_addr as *const c_void)
}

fn do_munmap(addr: *const c_void, size: size_t) -> Result<(), Error> {
    let addr = addr as usize;
    let size = size as usize;
    vm::do_munmap(addr, size)
}

fn do_mremap(old_addr: *const c_void, old_size: size_t,
             new_size: size_t, flags: c_int, new_addr: *const c_void)
    -> Result<*const c_void, Error>
{
    let old_addr = old_addr as usize;
    let old_size = old_size as usize;
    let mut options = VMResizeOptions::new(new_size)?;
    // TODO: handle flags and new_addr
    vm::do_mremap(old_addr, old_size, &options)
        .map(|ret_addr| ret_addr as *const c_void)
}

fn do_brk(new_brk_addr: *const c_void) -> Result<*const c_void, Error> {
    let new_brk_addr = new_brk_addr as usize;
    vm::do_brk(new_brk_addr).map(|ret_brk_addr| ret_brk_addr as *const c_void)
}

fn do_wait4(pid: c_int, _exit_status: *mut c_int) -> Result<pid_t, Error> {
    if _exit_status != 0 as *mut c_int {
        check_mut_ptr(_exit_status)?;
    }

    let child_process_filter = match pid {
        pid if pid < -1 => {
            process::ChildProcessFilter::WithPGID((-pid) as pid_t)
        },
        -1 => {
            process::ChildProcessFilter::WithAnyPID
        },
        0 => {
            let gpid = process::do_getgpid();
            process::ChildProcessFilter::WithPGID(gpid)
        },
        pid if pid > 0 => {
            process::ChildProcessFilter::WithPID(pid as pid_t)
        },
        _ => {
            panic!("THIS SHOULD NEVER HAPPEN!");
        }
    };
    let mut exit_status = 0;
    match process::do_wait4(&child_process_filter, &mut exit_status) {
        Ok(pid) => {
            if _exit_status != 0 as *mut c_int {
                unsafe { *_exit_status = exit_status; }
            }
            Ok(pid)
        }
        Err(e) => {
            Err(e)
        }
    }
}

fn do_pipe2(fds_u: *mut c_int, flags: c_int) -> Result<(), Error> {
    check_mut_array(fds_u, 2)?;
    // TODO: how to deal with open flags???
    let fds = fs::do_pipe2(flags as u32)?;
    unsafe {
        *fds_u.offset(0) = fds[0] as c_int;
        *fds_u.offset(1) = fds[1] as c_int;
    }
    Ok(())
}

fn do_gettimeofday(tv_u: *mut timeval_t) -> Result<(), Error> {
    check_mut_ptr(tv_u)?;
    let tv = time::do_gettimeofday();
    unsafe { *tv_u = tv; }
    Ok(())
}


const MAP_FAILED : *const c_void = ((-1) as i64) as *const c_void;

#[no_mangle]
pub extern "C" fn occlum_mmap(addr: *const c_void, length: size_t, prot: c_int,
                              flags: c_int, fd: c_int, offset: off_t)
    -> *const c_void
{
    match do_mmap(addr, length, prot, flags, fd, offset) {
        Ok(ret_addr) => { ret_addr },
        Err(e) => { MAP_FAILED }
    }
}

#[no_mangle]
pub extern "C" fn occlum_munmap(addr: *const c_void, length: size_t) -> c_int {
    match do_munmap(addr, length) {
        Ok(()) => { 0 },
        Err(e) => { -1 }
    }
}

#[no_mangle]
pub extern "C" fn occlum_mremap(old_addr: *const c_void, old_size: size_t,
                                new_size: size_t, flags: c_int,
                                new_addr: *const c_void)
    -> *const c_void
{
    match do_mremap(old_addr, old_size, new_size, flags, new_addr) {
        Ok(ret_addr) => { ret_addr },
        Err(e) => { MAP_FAILED }
    }
}

#[no_mangle]
pub extern "C" fn occlum_brk(addr: *const c_void) ->  *const c_void {
    match do_brk(addr) {
        Ok(ret_addr) => { ret_addr },
        Err(e) => { MAP_FAILED }
    }
}

#[no_mangle]
pub extern "C" fn occlum_pipe(fds: *mut c_int) ->  c_int {
    occlum_pipe2(fds, 0)
}

#[no_mangle]
pub extern "C" fn occlum_pipe2(fds: *mut c_int, flags: c_int) ->  c_int {
    match do_pipe2(fds, flags) {
        Ok(()) => {
            0
        },
        Err(e) => {
            e.errno.as_retval()
        }
    }
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
    match fs::do_close(fd as FileDesc) {
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
pub extern "C" fn occlum_getppid() -> c_uint
{
    process::do_getppid()
}

#[no_mangle]
pub extern "C" fn occlum_exit(status: i32)
{
    process::do_exit(status);
}

#[no_mangle]
pub extern "C" fn occlum_unknown(num: u32)
{
    if cfg!(debug_assertions) {
        //println!("[WARNING] Unknown syscall (num = {})", num);
    }
}

#[no_mangle]
pub extern "C" fn occlum_spawn(
    child_pid: *mut c_uint, path: *const c_char,
    argv: *const *const c_char, envp: *const *const c_char,
    fdop_list: *const FdOp) -> c_int
{
    match do_spawn(child_pid, path, argv, envp, fdop_list) {
        Ok(()) => 0,
        Err(e) => { e.errno.as_retval() }
    }
}

#[no_mangle]
pub extern "C" fn occlum_wait4(child_pid: c_int, exit_status: *mut c_int,
    options: c_int/*, rusage: *mut Rusage*/) -> c_int
{
    match do_wait4(child_pid, exit_status) {
        Ok(pid) => {
            pid as c_int
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_dup(old_fd: c_int) -> c_int
{
    let old_fd = old_fd as FileDesc;
    match fs::do_dup(old_fd) {
        Ok(new_fd) => {
            new_fd as c_int
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_dup2(old_fd: c_int, new_fd: c_int)
    -> c_int
{
    let old_fd = old_fd as FileDesc;
    let new_fd = new_fd as FileDesc;
    match fs::do_dup2(old_fd, new_fd) {
        Ok(new_fd) => {
            new_fd as c_int
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_dup3(old_fd: c_int, new_fd: c_int, flags: c_int)
    -> c_int
{
    let old_fd = old_fd as FileDesc;
    let new_fd = new_fd as FileDesc;
    let flags = flags as u32;
    match fs::do_dup3(old_fd, new_fd, flags) {
        Ok(new_fd) => {
            new_fd as c_int
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

// TODO: handle tz: timezone_t
#[no_mangle]
pub extern "C" fn occlum_gettimeofday(tv: *mut timeval_t) -> c_int
{
    match do_gettimeofday(tv) {
        Ok(()) => {
            0
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

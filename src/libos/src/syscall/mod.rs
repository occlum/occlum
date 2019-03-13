use super::*;
use fs::{off_t, FileDesc};
use prelude::*;
use process::{pid_t, ChildProcessFilter, FileAction};
use std::ffi::{CStr, CString};
use std::ptr;
use time::timeval_t;
use util::mem_util::from_user::*;
use vm::{VMAreaFlags, VMResizeOptions};
use {fs, process, std, vm};
// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

use fs::File;

#[no_mangle]
pub extern "C" fn dispatch_syscall(
    num: u32,
    arg0: isize,
    arg1: isize,
    arg2: isize,
    arg3: isize,
    arg4: isize,
    arg5: isize,
) -> isize {
    debug!("syscall {}: {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}", num, arg0, arg1, arg2, arg3, arg4, arg5);
    let ret = match num {
        002 => do_open(arg0 as *const i8, arg1 as u32, arg2 as u32),
        003 => do_close(arg0 as FileDesc),
        000 => do_read(arg0 as FileDesc, arg1 as *mut u8, arg2 as usize),
        001 => do_write(arg0 as FileDesc, arg1 as *const u8, arg2 as usize),
        019 => do_readv(arg0 as FileDesc, arg1 as *mut iovec_t, arg2 as i32),
        020 => do_writev(arg0 as FileDesc, arg1 as *mut iovec_t, arg2 as i32),
        004 => do_stat(arg0 as *const i8, arg1 as *mut fs::Stat),
        005 => do_fstat(arg0 as FileDesc, arg1 as *mut fs::Stat),
        006 => do_lstat(arg0 as *const i8, arg1 as *mut fs::Stat),
        008 => do_lseek(arg0 as FileDesc, arg1 as off_t, arg2 as i32),
        074 => do_fsync(arg0 as FileDesc),
        075 => do_fdatasync(arg0 as FileDesc),
        076 => do_truncate(arg0 as *const i8, arg1 as usize),
        077 => do_ftruncate(arg0 as FileDesc, arg1 as usize),
        217 => do_getdents64(arg0 as FileDesc, arg1 as *mut u8, arg2 as usize),
        162 => do_sync(),
        079 => do_getcwd(arg0 as *mut u8, arg1 as usize),

        060 => do_exit(arg0 as i32),
        360 => do_spawn(
            arg0 as *mut u32,
            arg1 as *mut i8,
            arg2 as *const *const i8,
            arg3 as *const *const i8,
            arg4 as *const FdOp,
        ),
        061 => do_wait4(arg0 as i32, arg1 as *mut i32),
        039 => do_getpid(),
        110 => do_getppid(),

        009 => do_mmap(
            arg0 as usize,
            arg1 as usize,
            arg2 as i32,
            arg3 as i32,
            arg4 as FileDesc,
            arg5 as off_t,
        ),
        011 => do_munmap(arg0 as usize, arg1 as usize),
        025 => do_mremap(
            arg0 as usize,
            arg1 as usize,
            arg2 as usize,
            arg3 as i32,
            arg4 as usize,
        ),
        012 => do_brk(arg0 as usize),

        022 => do_pipe2(arg0 as *mut i32, 0),
        293 => do_pipe2(arg0 as *mut i32, arg1 as u32),
        032 => do_dup(arg0 as FileDesc),
        033 => do_dup2(arg0 as FileDesc, arg1 as FileDesc),
        292 => do_dup3(arg0 as FileDesc, arg1 as FileDesc, arg2 as u32),

        096 => do_gettimeofday(arg0 as *mut timeval_t),

        _ => do_unknown(num),
    };
    debug!("syscall return: {:?}", ret);

    match ret {
        Ok(code) => code as isize,
        Err(e) => e.errno.as_retval() as isize,
    }
}

#[allow(non_camel_case_types)]
pub struct iovec_t {
    base: *const c_void,
    len: size_t,
}

/*
 * This Rust-version of fdop correspond to the C-version one in Occlum.
 * See <path_to_musl_libc>/src/process/fdop.h.
 */
const FDOP_CLOSE: u32 = 1;
const FDOP_DUP2: u32 = 2;
const FDOP_OPEN: u32 = 3;

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

fn clone_file_actions_safely(fdop_ptr: *const FdOp) -> Result<Vec<FileAction>, Error> {
    let mut file_actions = Vec::new();

    let mut fdop_ptr = fdop_ptr;
    while fdop_ptr != ptr::null() {
        check_ptr(fdop_ptr)?;
        let fdop = unsafe { &*fdop_ptr };

        let file_action = match fdop.cmd {
            FDOP_CLOSE => FileAction::Close(fdop.fd),
            FDOP_DUP2 => FileAction::Dup2(fdop.srcfd, fdop.fd),
            FDOP_OPEN => {
                return errno!(EINVAL, "Not implemented");
            }
            _ => {
                return errno!(EINVAL, "Unknown file action command");
            }
        };
        file_actions.push(file_action);

        fdop_ptr = fdop.next;
    }

    Ok(file_actions)
}

fn do_spawn(
    child_pid_ptr: *mut u32,
    path: *const i8,
    argv: *const *const i8,
    envp: *const *const i8,
    fdop_list: *const FdOp,
) -> Result<isize, Error> {
    check_mut_ptr(child_pid_ptr)?;
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let argv = clone_cstrings_safely(argv)?;
    let envp = clone_cstrings_safely(envp)?;
    let file_actions = clone_file_actions_safely(fdop_list)?;
    let parent = process::get_current();
    info!("spawn: path: {:?}, argv: {:?}, envp: {:?}, fdop: {:?}", path, argv, envp, file_actions);

    let child_pid = process::do_spawn(&path, &argv, &envp, &file_actions, &parent)?;

    unsafe { *child_pid_ptr = child_pid };
    Ok(0)
}

fn do_open(path: *const i8, flags: u32, mode: u32) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let fd = fs::do_open(&path, flags, mode)?;
    Ok(fd as isize)
}

fn do_close(fd: FileDesc) -> Result<isize, Error> {
    info!("close: fd: {}", fd);
    fs::do_close(fd)?;
    Ok(0)
}

fn do_read(fd: FileDesc, buf: *mut u8, size: usize) -> Result<isize, Error> {
    info!("write: fd: {}, buf: {:?}, size: {}", fd, buf, size);
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = fs::do_read(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_write(fd: FileDesc, buf: *const u8, size: usize) -> Result<isize, Error> {
    info!("write: fd: {}, buf: {:?}, size: {}", fd, buf, size);
    let safe_buf = {
        check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    let len = fs::do_write(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_writev(fd: FileDesc, iov: *const iovec_t, count: i32) -> Result<isize, Error> {
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
            let buf = unsafe { std::slice::from_raw_parts(iov.base as *const u8, iov.len) };
            bufs_vec.push(buf);
        }
        bufs_vec
    };
    let bufs = &bufs_vec[..];

    let len = fs::do_writev(fd, bufs)?;
    Ok(len as isize)
}

fn do_readv(fd: FileDesc, iov: *mut iovec_t, count: i32) -> Result<isize, Error> {
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
            let buf = unsafe { std::slice::from_raw_parts_mut(iov.base as *mut u8, iov.len) };
            bufs_vec.push(buf);
        }
        bufs_vec
    };
    let bufs = &mut bufs_vec[..];

    let len = fs::do_readv(fd, bufs)?;
    Ok(len as isize)
}

fn do_stat(path: *const i8, stat_buf: *mut fs::Stat) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_stat(&path)?;
    unsafe { stat_buf.write(stat); }
    Ok(0)
}

fn do_fstat(fd: FileDesc, stat_buf: *mut fs::Stat) -> Result<isize, Error> {
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_fstat(fd)?;
    unsafe { stat_buf.write(stat); }
    Ok(0)
}

fn do_lstat(path: *const i8, stat_buf: *mut fs::Stat) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_lstat(&path)?;
    unsafe { stat_buf.write(stat); }
    Ok(0)
}

fn do_lseek(fd: FileDesc, offset: off_t, whence: i32) -> Result<isize, Error> {
    let seek_from = match whence {
        0 => {
            // SEEK_SET
            if offset < 0 {
                return Err(Error::new(Errno::EINVAL, "Invalid offset"));
            }
            SeekFrom::Start(offset as u64)
        }
        1 => {
            // SEEK_CUR
            SeekFrom::Current(offset)
        }
        2 => {
            // SEEK_END
            SeekFrom::End(offset)
        }
        _ => {
            return Err(Error::new(Errno::EINVAL, "Invalid whence"));
        }
    };

    let offset = fs::do_lseek(fd, seek_from)?;
    Ok(offset as isize)
}

fn do_fsync(fd: FileDesc) -> Result<isize, Error> {
    fs::do_fsync(fd)?;
    Ok(0)
}

fn do_fdatasync(fd: FileDesc) -> Result<isize, Error> {
    fs::do_fdatasync(fd)?;
    Ok(0)
}

fn do_truncate(path: *const i8, len: usize) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_truncate(&path, len)?;
    Ok(0)
}

fn do_ftruncate(fd: FileDesc, len: usize) -> Result<isize, Error> {
    fs::do_ftruncate(fd, len)?;
    Ok(0)
}

fn do_getdents64(fd: FileDesc, buf: *mut u8, buf_size: usize) -> Result<isize, Error> {
    let safe_buf = {
        check_mut_array(buf, buf_size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, buf_size) }
    };
    let len = fs::do_getdents64(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_sync() -> Result<isize, Error> {
    fs::do_sync()?;
    Ok(0)
}

fn do_mmap(
    addr: usize,
    size: usize,
    prot: i32,
    flags: i32,
    fd: FileDesc,
    offset: off_t,
) -> Result<isize, Error> {
    let flags = VMAreaFlags(prot as u32);
    let addr = vm::do_mmap(addr, size, flags)?;
    Ok(addr as isize)
}

fn do_munmap(addr: usize, size: usize) -> Result<isize, Error> {
    vm::do_munmap(addr, size)?;
    Ok(0)
}

fn do_mremap(
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: i32,
    new_addr: usize,
) -> Result<isize, Error> {
    let mut options = VMResizeOptions::new(new_size)?;
    // TODO: handle flags and new_addr
    let ret_addr = vm::do_mremap(old_addr, old_size, &options)?;
    Ok(ret_addr as isize)
}

fn do_brk(new_brk_addr: usize) -> Result<isize, Error> {
    let ret_brk_addr = vm::do_brk(new_brk_addr)?;
    Ok(ret_brk_addr as isize)
}

fn do_wait4(pid: i32, _exit_status: *mut i32) -> Result<isize, Error> {
    if !_exit_status.is_null() {
        check_mut_ptr(_exit_status)?;
    }

    let child_process_filter = match pid {
        pid if pid < -1 => process::ChildProcessFilter::WithPGID((-pid) as pid_t),
        -1 => process::ChildProcessFilter::WithAnyPID,
        0 => {
            let gpid = process::do_getgpid();
            process::ChildProcessFilter::WithPGID(gpid)
        }
        pid if pid > 0 => process::ChildProcessFilter::WithPID(pid as pid_t),
        _ => {
            panic!("THIS SHOULD NEVER HAPPEN!");
        }
    };
    let mut exit_status = 0;
    match process::do_wait4(&child_process_filter, &mut exit_status) {
        Ok(pid) => {
            if !_exit_status.is_null() {
                unsafe {
                    *_exit_status = exit_status;
                }
            }
            Ok(pid as isize)
        }
        Err(e) => Err(e),
    }
}

fn do_getpid() -> Result<isize, Error> {
    info!("getpid");
    let pid = process::do_getpid();
    Ok(pid as isize)
}

fn do_getppid() -> Result<isize, Error> {
    let ppid = process::do_getppid();
    Ok(ppid as isize)
}

fn do_pipe2(fds_u: *mut i32, flags: u32) -> Result<isize, Error> {
    check_mut_array(fds_u, 2)?;
    // TODO: how to deal with open flags???
    let fds = fs::do_pipe2(flags as u32)?;
    unsafe {
        *fds_u.offset(0) = fds[0] as c_int;
        *fds_u.offset(1) = fds[1] as c_int;
    }
    Ok(0)
}

fn do_dup(old_fd: FileDesc) -> Result<isize, Error> {
    let new_fd = fs::do_dup(old_fd)?;
    Ok(new_fd as isize)
}

fn do_dup2(old_fd: FileDesc, new_fd: FileDesc) -> Result<isize, Error> {
    let new_fd = fs::do_dup2(old_fd, new_fd)?;
    Ok(new_fd as isize)
}

fn do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32) -> Result<isize, Error> {
    let new_fd = fs::do_dup3(old_fd, new_fd, flags)?;
    Ok(new_fd as isize)
}

// TODO: handle tz: timezone_t
fn do_gettimeofday(tv_u: *mut timeval_t) -> Result<isize, Error> {
    check_mut_ptr(tv_u)?;
    let tv = time::do_gettimeofday();
    unsafe {
        *tv_u = tv;
    }
    Ok(0)
}

// FIXME: use this
const MAP_FAILED: *const c_void = ((-1) as i64) as *const c_void;

fn do_exit(status: i32) -> ! {
    info!("exit: {}", status);
    extern "C" {
        fn do_exit_task() -> !;
    }
    process::do_exit(status);
    unsafe {
        do_exit_task();
    }
}

fn do_unknown(num: u32) -> Result<isize, Error> {
    warn!("Unknown syscall (num = {})", num);
    Err(Error::new(ENOSYS, "Unknown syscall"))
}

fn do_getcwd(buf: *mut u8, size: usize) -> Result<isize, Error> {
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let proc_ref = process::get_current();
    let mut proc = proc_ref.lock().unwrap();
    let cwd = proc.get_cwd();
    if cwd.len() + 1 > safe_buf.len() {
        return Err(Error::new(ERANGE, "buf is not long enough"));
    }
    safe_buf[..cwd.len()].copy_from_slice(cwd.as_bytes());
    safe_buf[cwd.len()] = 0;
    Ok(0)
}

use {fs, process, std, vm};
use fs::{File, FileDesc, off_t, AccessModes, AccessFlags, AT_FDCWD, FcntlCmd};
use prelude::*;
use process::{ChildProcessFilter, FileAction, pid_t, CloneFlags, FutexFlags, FutexOp};
use std::ffi::{CStr, CString};
use std::ptr;
use time::timeval_t;
use util::mem_util::from_user::*;
use vm::{VMAreaFlags, VMResizeOptions};
use misc::{utsname_t, resource_t, rlimit_t};

use super::*;

use self::consts::*;

// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

mod consts;

#[no_mangle]
#[deny(unreachable_patterns)]
pub extern "C" fn dispatch_syscall(
    num: u32,
    arg0: isize,
    arg1: isize,
    arg2: isize,
    arg3: isize,
    arg4: isize,
    arg5: isize,
) -> isize {
    debug!(
        "syscall {}: {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
        num, arg0, arg1, arg2, arg3, arg4, arg5
    );
    let ret = match num {
        SYS_OPEN => do_open(arg0 as *const i8, arg1 as u32, arg2 as u32),
        SYS_CLOSE => do_close(arg0 as FileDesc),
        SYS_READ => do_read(arg0 as FileDesc, arg1 as *mut u8, arg2 as usize),
        SYS_WRITE => do_write(arg0 as FileDesc, arg1 as *const u8, arg2 as usize),
        SYS_PREAD64 => do_pread(
            arg0 as FileDesc,
            arg1 as *mut u8,
            arg2 as usize,
            arg3 as usize,
        ),
        SYS_PWRITE64 => do_pwrite(
            arg0 as FileDesc,
            arg1 as *const u8,
            arg2 as usize,
            arg3 as usize,
        ),
        SYS_READV => do_readv(arg0 as FileDesc, arg1 as *mut iovec_t, arg2 as i32),
        SYS_WRITEV => do_writev(arg0 as FileDesc, arg1 as *mut iovec_t, arg2 as i32),
        SYS_STAT => do_stat(arg0 as *const i8, arg1 as *mut fs::Stat),
        SYS_FSTAT => do_fstat(arg0 as FileDesc, arg1 as *mut fs::Stat),
        SYS_LSTAT => do_lstat(arg0 as *const i8, arg1 as *mut fs::Stat),
        SYS_ACCESS => do_access(arg0 as *const i8, arg1 as u32),
        SYS_FACCESSAT => do_faccessat(arg0 as i32, arg1 as *const i8, arg2 as u32, arg3 as u32),
        SYS_LSEEK => do_lseek(arg0 as FileDesc, arg1 as off_t, arg2 as i32),
        SYS_FSYNC => do_fsync(arg0 as FileDesc),
        SYS_FDATASYNC => do_fdatasync(arg0 as FileDesc),
        SYS_TRUNCATE => do_truncate(arg0 as *const i8, arg1 as usize),
        SYS_FTRUNCATE => do_ftruncate(arg0 as FileDesc, arg1 as usize),
        SYS_GETDENTS64 => do_getdents64(arg0 as FileDesc, arg1 as *mut u8, arg2 as usize),
        SYS_SYNC => do_sync(),
        SYS_GETCWD => do_getcwd(arg0 as *mut u8, arg1 as usize),
        SYS_CHDIR => do_chdir(arg0 as *mut i8),
        SYS_RENAME => do_rename(arg0 as *const i8, arg1 as *const i8),
        SYS_MKDIR => do_mkdir(arg0 as *const i8, arg1 as usize),
        SYS_RMDIR => do_rmdir(arg0 as *const i8),
        SYS_LINK => do_link(arg0 as *const i8, arg1 as *const i8),
        SYS_UNLINK => do_unlink(arg0 as *const i8),
        SYS_FCNTL => do_fcntl(arg0 as FileDesc, arg1 as u32, arg2 as u64),

        SYS_EXIT => do_exit(arg0 as i32),
        SYS_SPAWN => do_spawn(
            arg0 as *mut u32,
            arg1 as *mut i8,
            arg2 as *const *const i8,
            arg3 as *const *const i8,
            arg4 as *const FdOp,
        ),
        SYS_WAIT4 => do_wait4(arg0 as i32, arg1 as *mut i32),

        SYS_GETPID => do_getpid(),
        SYS_GETTID => do_gettid(),
        SYS_GETPPID => do_getppid(),
        SYS_GETPGID => do_getpgid(),

        SYS_GETUID => do_getuid(),
        SYS_GETGID => do_getgid(),
        SYS_GETEUID => do_geteuid(),
        SYS_GETEGID => do_getegid(),

        SYS_RT_SIGACTION => do_rt_sigaction(),
        SYS_RT_SIGPROCMASK => do_rt_sigprocmask(),

        SYS_CLONE => do_clone(
            arg0 as u32,
            arg1 as usize,
            arg2 as *mut pid_t,
            arg3 as *mut pid_t,
            arg4 as usize,
        ),
        SYS_FUTEX => do_futex(
            arg0 as *const i32,
            arg1 as u32,
            arg2 as i32,
            // TODO: accept other optional arguments
        ),
        SYS_ARCH_PRCTL => do_arch_prctl(arg0 as u32, arg1 as *mut usize),
        SYS_SET_TID_ADDRESS => do_set_tid_address(arg0 as *mut pid_t),

        SYS_MMAP => do_mmap(
            arg0 as usize,
            arg1 as usize,
            arg2 as i32,
            arg3 as i32,
            arg4 as FileDesc,
            arg5 as off_t,
        ),
        SYS_MUNMAP => do_munmap(arg0 as usize, arg1 as usize),
        SYS_MREMAP => do_mremap(
            arg0 as usize,
            arg1 as usize,
            arg2 as usize,
            arg3 as i32,
            arg4 as usize,
        ),
        SYS_MPROTECT => do_mprotect(
            arg0 as usize,
            arg1 as usize,
            arg2 as u32,
        ),
        SYS_BRK => do_brk(arg0 as usize),

        SYS_PIPE => do_pipe2(arg0 as *mut i32, 0),
        SYS_PIPE2 => do_pipe2(arg0 as *mut i32, arg1 as u32),
        SYS_DUP => do_dup(arg0 as FileDesc),
        SYS_DUP2 => do_dup2(arg0 as FileDesc, arg1 as FileDesc),
        SYS_DUP3 => do_dup3(arg0 as FileDesc, arg1 as FileDesc, arg2 as u32),

        SYS_GETTIMEOFDAY => do_gettimeofday(arg0 as *mut timeval_t),

        SYS_UNAME => do_uname(arg0 as *mut utsname_t),

        SYS_PRLIMIT64 => do_prlimit(arg0 as pid_t, arg1 as u32, arg2 as *const rlimit_t, arg3 as *mut rlimit_t),

        _ => do_unknown(num, arg0, arg1, arg2, arg3, arg4, arg5),
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
    info!(
        "spawn: path: {:?}, argv: {:?}, envp: {:?}, fdop: {:?}",
        path, argv, envp, file_actions
    );

    let child_pid = process::do_spawn(&path, &argv, &envp, &file_actions, &parent)?;

    unsafe { *child_pid_ptr = child_pid };
    Ok(0)
}

pub fn do_clone(
    flags: u32,
    stack_addr: usize,
    ptid: *mut pid_t,
    ctid: *mut pid_t,
    new_tls: usize,
) -> Result<isize, Error> {
    let flags = CloneFlags::from_bits_truncate(flags);
    check_mut_ptr(stack_addr as *mut u64)?;
    let ptid = {
        if flags.contains(CloneFlags::CLONE_PARENT_SETTID) {
            check_mut_ptr(ptid)?;
            Some(ptid)
        }
        else {
            None
        }
    };
    let ctid = {
        if flags.contains(CloneFlags::CLONE_CHILD_CLEARTID) {
            check_mut_ptr(ctid)?;
            Some(ctid)
        }
        else {
            None
        }
    };
    let new_tls = {
        if flags.contains(CloneFlags::CLONE_SETTLS) {
            check_mut_ptr(new_tls as *mut usize)?;
            Some(new_tls)
        }
        else {
            None
        }
    };

    let child_pid = process::do_clone(flags, stack_addr, ptid, ctid, new_tls)?;

    Ok(child_pid as isize)
}

pub fn do_futex(
    futex_addr: *const i32,
    futex_op: u32,
    futex_val: i32,
) -> Result<isize, Error> {
    check_ptr(futex_addr)?;
    let (futex_op, futex_flags) = process::futex_op_and_flags_from_u32(futex_op)?;
    match futex_op {
        FutexOp::FUTEX_WAIT => {
            process::futex_wait(futex_addr, futex_val).map(|_| 0)
        }
        FutexOp::FUTEX_WAKE => {
            let max_count = {
                if futex_val < 0 {
                    return errno!(EINVAL, "the count must not be negative");
                }
                futex_val as usize
            };
            process::futex_wake(futex_addr, max_count)
                .map(|count| count as isize)
        },
        _ => errno!(ENOSYS, "the futex operation is not supported"),
    }
}

fn do_open(path: *const i8, flags: u32, mode: u32) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let fd = fs::do_open(&path, flags, mode)?;
    Ok(fd as isize)
}

fn do_close(fd: FileDesc) -> Result<isize, Error> {
    fs::do_close(fd)?;
    Ok(0)
}

fn do_read(fd: FileDesc, buf: *mut u8, size: usize) -> Result<isize, Error> {
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = fs::do_read(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_write(fd: FileDesc, buf: *const u8, size: usize) -> Result<isize, Error> {
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

fn do_pread(fd: FileDesc, buf: *mut u8, size: usize, offset: usize) -> Result<isize, Error> {
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = fs::do_pread(fd, safe_buf, offset)?;
    Ok(len as isize)
}

fn do_pwrite(fd: FileDesc, buf: *const u8, size: usize, offset: usize) -> Result<isize, Error> {
    let safe_buf = {
        check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    let len = fs::do_pwrite(fd, safe_buf, offset)?;
    Ok(len as isize)
}

fn do_stat(path: *const i8, stat_buf: *mut fs::Stat) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_stat(&path)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

fn do_fstat(fd: FileDesc, stat_buf: *mut fs::Stat) -> Result<isize, Error> {
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_fstat(fd)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

fn do_lstat(path: *const i8, stat_buf: *mut fs::Stat) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_lstat(&path)?;
    unsafe {
        stat_buf.write(stat);
    }
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

fn do_mprotect(
    addr: usize,
    len: usize,
    prot: u32,
) -> Result<isize, Error> {
    // TODO: implement it
    Ok(0)
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
            let pgid = process::do_getpgid();
            process::ChildProcessFilter::WithPGID(pgid)
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
    let pid = process::do_getpid();
    Ok(pid as isize)
}

fn do_gettid() -> Result<isize, Error> {
    let tid = process::do_gettid();
    Ok(tid as isize)
}

fn do_getppid() -> Result<isize, Error> {
    let ppid = process::do_getppid();
    Ok(ppid as isize)
}

fn do_getpgid() -> Result<isize, Error> {
    let pgid = process::do_getpgid();
    Ok(pgid as isize)
}

// TODO: implement uid, gid, euid, egid

fn do_getuid() -> Result<isize, Error> {
    Ok(0)
}

fn do_getgid() -> Result<isize, Error> {
    Ok(0)
}

fn do_geteuid() -> Result<isize, Error> {
    Ok(0)
}

fn do_getegid() -> Result<isize, Error> {
    Ok(0)
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
    extern "C" {
        fn do_exit_task() -> !;
    }
    process::do_exit(status);
    unsafe {
        do_exit_task();
    }
}

fn do_unknown(num: u32, arg0: isize, arg1: isize, arg2: isize, arg3: isize, arg4: isize, arg5: isize) -> Result<isize, Error> {
    warn!(
        "unknown or unsupported syscall (# = {}): {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
        num, arg0, arg1, arg2, arg3, arg4, arg5
    );
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
    Ok(buf as isize)
}

fn do_chdir(path: *const i8) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_chdir(&path)?;
    Ok(0)
}

fn do_rename(oldpath: *const i8, newpath: *const i8) -> Result<isize, Error> {
    let oldpath = clone_cstring_safely(oldpath)?
        .to_string_lossy()
        .into_owned();
    let newpath = clone_cstring_safely(newpath)?
        .to_string_lossy()
        .into_owned();
    fs::do_rename(&oldpath, &newpath)?;
    Ok(0)
}

fn do_mkdir(path: *const i8, mode: usize) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_mkdir(&path, mode)?;
    Ok(0)
}

fn do_rmdir(path: *const i8) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_rmdir(&path)?;
    Ok(0)
}

fn do_link(oldpath: *const i8, newpath: *const i8) -> Result<isize, Error> {
    let oldpath = clone_cstring_safely(oldpath)?
        .to_string_lossy()
        .into_owned();
    let newpath = clone_cstring_safely(newpath)?
        .to_string_lossy()
        .into_owned();
    fs::do_link(&oldpath, &newpath)?;
    Ok(0)
}

fn do_unlink(path: *const i8) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_unlink(&path)?;
    Ok(0)
}

fn do_fcntl(fd: FileDesc, cmd: u32, arg: u64) -> Result<isize, Error> {
    let cmd = FcntlCmd::from_raw(cmd, arg)?;
    fs::do_fcntl(fd, &cmd)
}

fn do_arch_prctl(code: u32, addr: *mut usize) -> Result<isize, Error> {
    let code = process::ArchPrctlCode::from_u32(code)?;
    check_mut_ptr(addr)?;
    process::do_arch_prctl(code, addr).map(|_| 0)
}

fn do_set_tid_address(tidptr: *mut pid_t) -> Result<isize, Error> {
    check_mut_ptr(tidptr)?;
    process::do_set_tid_address(tidptr).map(|tid| tid as isize)
}

fn do_uname(name: *mut utsname_t) -> Result<isize, Error> {
    check_mut_ptr(name)?;
    let name = unsafe { &mut *name };
    misc::do_uname(name).map(|_| 0)
}

fn do_prlimit(pid: pid_t, resource: u32, new_limit: *const rlimit_t, old_limit: *mut rlimit_t) -> Result<isize, Error> {
    let resource = resource_t::from_u32(resource)?;
    let new_limit = {
        if new_limit != ptr::null() {
            check_ptr(new_limit)?;
            Some(unsafe { &*new_limit })
        }
        else {
            None
        }
    };
    let old_limit = {
        if old_limit != ptr::null_mut() {
            check_mut_ptr(old_limit)?;
            Some(unsafe { &mut *old_limit })
        }
        else {
            None
        }
    };
    misc::do_prlimit(pid, resource, new_limit, old_limit).map(|_| 0)
}

fn do_access(path: *const i8, mode: u32) -> Result<isize, Error> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let mode = AccessModes::from_u32(mode)?;
    fs::do_access(&path, mode).map(|_| 0)
}

fn do_faccessat(dirfd: i32, path: *const i8, mode: u32, flags: u32) -> Result<isize, Error> {
    let dirfd = if dirfd >= 0 {
        Some(dirfd as FileDesc)
    } else if dirfd == AT_FDCWD {
        None
    } else {
        return errno!(EINVAL, "invalid dirfd");
    };
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let mode = AccessModes::from_u32(mode)?;
    let flags = AccessFlags::from_u32(flags)?;
    fs::do_faccessat(dirfd, &path, mode, flags).map(|_| 0)
}

// TODO: implement signals

fn do_rt_sigaction() -> Result<isize, Error> {
    Ok(0)
}

fn do_rt_sigprocmask() -> Result<isize, Error> {
    Ok(0)
}

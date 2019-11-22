//! System call handler
//!
//! # Syscall processing flow
//!
//! 1. User call `__occlum_syscall` (at `syscall_entry_x86_64.S`)
//! 2. Do some bound checks then call `dispatch_syscall` (at this file)
//! 3. Dispatch the syscall to `do_*` (at this file)
//! 4. Do some memory checks then call `mod::do_*` (at each module)

use fs::*;
use misc::{resource_t, rlimit_t, utsname_t};
use process::{pid_t, ChildProcessFilter, CloneFlags, CpuSet, FileAction, FutexFlags, FutexOp};
use std::ffi::{CStr, CString};
use std::ptr;
use time::{clockid_t, timespec_t, timeval_t};
use util::mem_util::from_user::*;
use vm::{MMapFlags, VMPerms};
use {fs, process, std, vm};

use super::*;

use self::consts::*;
use std::any::Any;
use std::io::{Read, Seek, SeekFrom, Write};

// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

mod consts;

static mut SYSCALL_TIMING: [usize; 361] = [0; 361];

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
    #[cfg(feature = "syscall_timing")]
    let time_start = {
        static mut LAST_PRINT: usize = 0;
        let time = crate::time::do_gettimeofday().as_usec();
        unsafe {
            if time / 1000000 / 5 > LAST_PRINT {
                LAST_PRINT = time / 1000000 / 5;
                print_syscall_timing();
            }
        }
        time
    };

    let ret = match num {
        // file
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
        SYS_READLINK => do_readlink(arg0 as *const i8, arg1 as *mut u8, arg2 as usize),
        SYS_SENDFILE => do_sendfile(
            arg0 as FileDesc,
            arg1 as FileDesc,
            arg2 as *mut off_t,
            arg3 as usize,
        ),
        SYS_FCNTL => do_fcntl(arg0 as FileDesc, arg1 as u32, arg2 as u64),
        SYS_IOCTL => do_ioctl(arg0 as FileDesc, arg1 as u32, arg2 as *mut u8),

        // IO multiplexing
        SYS_SELECT => do_select(
            arg0 as c_int,
            arg1 as *mut libc::fd_set,
            arg2 as *mut libc::fd_set,
            arg3 as *mut libc::fd_set,
            arg4 as *const libc::timeval,
        ),
        SYS_POLL => do_poll(
            arg0 as *mut libc::pollfd,
            arg1 as libc::nfds_t,
            arg2 as c_int,
        ),
        SYS_EPOLL_CREATE => do_epoll_create(arg0 as c_int),
        SYS_EPOLL_CREATE1 => do_epoll_create1(arg0 as c_int),
        SYS_EPOLL_CTL => do_epoll_ctl(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *const libc::epoll_event,
        ),
        SYS_EPOLL_WAIT => do_epoll_wait(
            arg0 as c_int,
            arg1 as *mut libc::epoll_event,
            arg2 as c_int,
            arg3 as c_int,
        ),
        SYS_EPOLL_PWAIT => do_epoll_pwait(
            arg0 as c_int,
            arg1 as *mut libc::epoll_event,
            arg2 as c_int,
            arg3 as c_int,
            arg4 as *const usize, //TODO:add sigset_t
        ),

        // process
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
        SYS_SCHED_GETAFFINITY => {
            do_sched_getaffinity(arg0 as pid_t, arg1 as size_t, arg2 as *mut c_uchar)
        }
        SYS_SCHED_SETAFFINITY => {
            do_sched_setaffinity(arg0 as pid_t, arg1 as size_t, arg2 as *const c_uchar)
        }

        // memory
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
        SYS_MPROTECT => do_mprotect(arg0 as usize, arg1 as usize, arg2 as u32),
        SYS_BRK => do_brk(arg0 as usize),

        SYS_PIPE => do_pipe2(arg0 as *mut i32, 0),
        SYS_PIPE2 => do_pipe2(arg0 as *mut i32, arg1 as u32),
        SYS_DUP => do_dup(arg0 as FileDesc),
        SYS_DUP2 => do_dup2(arg0 as FileDesc, arg1 as FileDesc),
        SYS_DUP3 => do_dup3(arg0 as FileDesc, arg1 as FileDesc, arg2 as u32),

        SYS_GETTIMEOFDAY => do_gettimeofday(arg0 as *mut timeval_t),
        SYS_CLOCK_GETTIME => do_clock_gettime(arg0 as clockid_t, arg1 as *mut timespec_t),

        SYS_NANOSLEEP => do_nanosleep(arg0 as *const timespec_t, arg1 as *mut timespec_t),

        SYS_UNAME => do_uname(arg0 as *mut utsname_t),

        SYS_PRLIMIT64 => do_prlimit(
            arg0 as pid_t,
            arg1 as u32,
            arg2 as *const rlimit_t,
            arg3 as *mut rlimit_t,
        ),

        // socket
        SYS_SOCKET => do_socket(arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SYS_CONNECT => do_connect(
            arg0 as c_int,
            arg1 as *const libc::sockaddr,
            arg2 as libc::socklen_t,
        ),
        SYS_ACCEPT => do_accept4(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
            0,
        ),
        SYS_ACCEPT4 => do_accept4(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
            arg3 as c_int,
        ),
        SYS_SHUTDOWN => do_shutdown(arg0 as c_int, arg1 as c_int),
        SYS_BIND => do_bind(
            arg0 as c_int,
            arg1 as *const libc::sockaddr,
            arg2 as libc::socklen_t,
        ),
        SYS_LISTEN => do_listen(arg0 as c_int, arg1 as c_int),
        SYS_SETSOCKOPT => do_setsockopt(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *const c_void,
            arg4 as libc::socklen_t,
        ),
        SYS_GETSOCKOPT => do_getsockopt(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *mut c_void,
            arg4 as *mut libc::socklen_t,
        ),
        SYS_GETPEERNAME => do_getpeername(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
        ),
        SYS_GETSOCKNAME => do_getsockname(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
        ),
        SYS_SENDTO => do_sendto(
            arg0 as c_int,
            arg1 as *const c_void,
            arg2 as size_t,
            arg3 as c_int,
            arg4 as *const libc::sockaddr,
            arg5 as libc::socklen_t,
        ),
        SYS_RECVFROM => do_recvfrom(
            arg0 as c_int,
            arg1 as *mut c_void,
            arg2 as size_t,
            arg3 as c_int,
            arg4 as *mut libc::sockaddr,
            arg5 as *mut libc::socklen_t,
        ),
        SYS_SOCKETPAIR => do_socketpair(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *mut c_int,
        ),

        _ => do_unknown(num, arg0, arg1, arg2, arg3, arg4, arg5),
    };

    #[cfg(feature = "syscall_timing")]
    {
        let time_end = crate::time::do_gettimeofday().as_usec();
        let time = time_end - time_start;
        unsafe {
            SYSCALL_TIMING[num as usize] += time as usize;
        }
    }

    info!("=> {:?}", ret);

    match ret {
        Ok(retval) => retval as isize,
        Err(e) => {
            warn!("{}", e.backtrace());

            let retval = -(e.errno() as isize);
            debug_assert!(retval != 0);
            retval
        }
    }
}

#[cfg(feature = "syscall_timing")]
fn print_syscall_timing() {
    println!("syscall timing:");
    for (i, &time) in unsafe { SYSCALL_TIMING }.iter().enumerate() {
        if time == 0 {
            continue;
        }
        println!("{:>3}: {:>6} us", i, time);
    }
    for x in unsafe { SYSCALL_TIMING.iter_mut() } {
        *x = 0;
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
    path: *const i8,
}

fn clone_file_actions_safely(fdop_ptr: *const FdOp) -> Result<Vec<FileAction>> {
    let mut file_actions = Vec::new();

    let mut fdop_ptr = fdop_ptr;
    while fdop_ptr != ptr::null() {
        check_ptr(fdop_ptr)?;
        let fdop = unsafe { &*fdop_ptr };

        let file_action = match fdop.cmd {
            FDOP_CLOSE => FileAction::Close(fdop.fd),
            FDOP_DUP2 => FileAction::Dup2(fdop.srcfd, fdop.fd),
            FDOP_OPEN => FileAction::Open {
                path: clone_cstring_safely(fdop.path)?
                    .to_string_lossy()
                    .into_owned(),
                mode: fdop.mode,
                oflag: fdop.oflag,
                fd: fdop.fd,
            },
            _ => {
                return_errno!(EINVAL, "Unknown file action command");
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
) -> Result<isize> {
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
) -> Result<isize> {
    let flags = CloneFlags::from_bits_truncate(flags);
    check_mut_ptr(stack_addr as *mut u64)?;
    let ptid = {
        if flags.contains(CloneFlags::CLONE_PARENT_SETTID) {
            check_mut_ptr(ptid)?;
            Some(ptid)
        } else {
            None
        }
    };
    let ctid = {
        if flags.contains(CloneFlags::CLONE_CHILD_CLEARTID) {
            check_mut_ptr(ctid)?;
            Some(ctid)
        } else {
            None
        }
    };
    let new_tls = {
        if flags.contains(CloneFlags::CLONE_SETTLS) {
            check_mut_ptr(new_tls as *mut usize)?;
            Some(new_tls)
        } else {
            None
        }
    };

    let child_pid = process::do_clone(flags, stack_addr, ptid, ctid, new_tls)?;

    Ok(child_pid as isize)
}

pub fn do_futex(futex_addr: *const i32, futex_op: u32, futex_val: i32) -> Result<isize> {
    check_ptr(futex_addr)?;
    let (futex_op, futex_flags) = process::futex_op_and_flags_from_u32(futex_op)?;
    match futex_op {
        FutexOp::FUTEX_WAIT => process::futex_wait(futex_addr, futex_val).map(|_| 0),
        FutexOp::FUTEX_WAKE => {
            let max_count = {
                if futex_val < 0 {
                    return_errno!(EINVAL, "the count must not be negative");
                }
                futex_val as usize
            };
            process::futex_wake(futex_addr, max_count).map(|count| count as isize)
        }
        _ => return_errno!(ENOSYS, "the futex operation is not supported"),
    }
}

fn do_open(path: *const i8, flags: u32, mode: u32) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let fd = fs::do_open(&path, flags, mode)?;
    Ok(fd as isize)
}

fn do_close(fd: FileDesc) -> Result<isize> {
    fs::do_close(fd)?;
    Ok(0)
}

fn do_read(fd: FileDesc, buf: *mut u8, size: usize) -> Result<isize> {
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = fs::do_read(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_write(fd: FileDesc, buf: *const u8, size: usize) -> Result<isize> {
    let safe_buf = {
        check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    let len = fs::do_write(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_writev(fd: FileDesc, iov: *const iovec_t, count: i32) -> Result<isize> {
    let count = {
        if count < 0 {
            return_errno!(EINVAL, "Invalid count of iovec");
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

fn do_readv(fd: FileDesc, iov: *mut iovec_t, count: i32) -> Result<isize> {
    let count = {
        if count < 0 {
            return_errno!(EINVAL, "Invalid count of iovec");
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

fn do_pread(fd: FileDesc, buf: *mut u8, size: usize, offset: usize) -> Result<isize> {
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = fs::do_pread(fd, safe_buf, offset)?;
    Ok(len as isize)
}

fn do_pwrite(fd: FileDesc, buf: *const u8, size: usize, offset: usize) -> Result<isize> {
    let safe_buf = {
        check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    let len = fs::do_pwrite(fd, safe_buf, offset)?;
    Ok(len as isize)
}

fn do_stat(path: *const i8, stat_buf: *mut fs::Stat) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_stat(&path)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

fn do_fstat(fd: FileDesc, stat_buf: *mut fs::Stat) -> Result<isize> {
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_fstat(fd)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

fn do_lstat(path: *const i8, stat_buf: *mut fs::Stat) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    check_mut_ptr(stat_buf)?;

    let stat = fs::do_lstat(&path)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

fn do_lseek(fd: FileDesc, offset: off_t, whence: i32) -> Result<isize> {
    let seek_from = match whence {
        0 => {
            // SEEK_SET
            if offset < 0 {
                return_errno!(EINVAL, "Invalid offset");
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
            return_errno!(EINVAL, "Invalid whence");
        }
    };

    let offset = fs::do_lseek(fd, seek_from)?;
    Ok(offset as isize)
}

fn do_fsync(fd: FileDesc) -> Result<isize> {
    fs::do_fsync(fd)?;
    Ok(0)
}

fn do_fdatasync(fd: FileDesc) -> Result<isize> {
    fs::do_fdatasync(fd)?;
    Ok(0)
}

fn do_truncate(path: *const i8, len: usize) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_truncate(&path, len)?;
    Ok(0)
}

fn do_ftruncate(fd: FileDesc, len: usize) -> Result<isize> {
    fs::do_ftruncate(fd, len)?;
    Ok(0)
}

fn do_getdents64(fd: FileDesc, buf: *mut u8, buf_size: usize) -> Result<isize> {
    let safe_buf = {
        check_mut_array(buf, buf_size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, buf_size) }
    };
    let len = fs::do_getdents64(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_sync() -> Result<isize> {
    fs::do_sync()?;
    Ok(0)
}

fn do_mmap(
    addr: usize,
    size: usize,
    perms: i32,
    flags: i32,
    fd: FileDesc,
    offset: off_t,
) -> Result<isize> {
    let perms = VMPerms::from_u32(perms as u32)?;
    let flags = MMapFlags::from_u32(flags as u32)?;
    let addr = vm::do_mmap(addr, size, perms, flags, fd, offset as usize)?;
    Ok(addr as isize)
}

fn do_munmap(addr: usize, size: usize) -> Result<isize> {
    vm::do_munmap(addr, size)?;
    Ok(0)
}

fn do_mremap(
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: i32,
    new_addr: usize,
) -> Result<isize> {
    warn!("mremap: not implemented!");
    return_errno!(ENOSYS, "not supported yet")
}

fn do_mprotect(addr: usize, len: usize, prot: u32) -> Result<isize> {
    // TODO: implement it
    Ok(0)
}

fn do_brk(new_brk_addr: usize) -> Result<isize> {
    let ret_brk_addr = vm::do_brk(new_brk_addr)?;
    Ok(ret_brk_addr as isize)
}

fn do_wait4(pid: i32, _exit_status: *mut i32) -> Result<isize> {
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

fn do_getpid() -> Result<isize> {
    let pid = process::do_getpid();
    Ok(pid as isize)
}

fn do_gettid() -> Result<isize> {
    let tid = process::do_gettid();
    Ok(tid as isize)
}

fn do_getppid() -> Result<isize> {
    let ppid = process::do_getppid();
    Ok(ppid as isize)
}

fn do_getpgid() -> Result<isize> {
    let pgid = process::do_getpgid();
    Ok(pgid as isize)
}

// TODO: implement uid, gid, euid, egid

fn do_getuid() -> Result<isize> {
    Ok(0)
}

fn do_getgid() -> Result<isize> {
    Ok(0)
}

fn do_geteuid() -> Result<isize> {
    Ok(0)
}

fn do_getegid() -> Result<isize> {
    Ok(0)
}

fn do_pipe2(fds_u: *mut i32, flags: u32) -> Result<isize> {
    check_mut_array(fds_u, 2)?;
    // TODO: how to deal with open flags???
    let fds = fs::do_pipe2(flags as u32)?;
    unsafe {
        *fds_u.offset(0) = fds[0] as c_int;
        *fds_u.offset(1) = fds[1] as c_int;
    }
    Ok(0)
}

fn do_dup(old_fd: FileDesc) -> Result<isize> {
    let new_fd = fs::do_dup(old_fd)?;
    Ok(new_fd as isize)
}

fn do_dup2(old_fd: FileDesc, new_fd: FileDesc) -> Result<isize> {
    let new_fd = fs::do_dup2(old_fd, new_fd)?;
    Ok(new_fd as isize)
}

fn do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32) -> Result<isize> {
    let new_fd = fs::do_dup3(old_fd, new_fd, flags)?;
    Ok(new_fd as isize)
}

// TODO: handle tz: timezone_t
fn do_gettimeofday(tv_u: *mut timeval_t) -> Result<isize> {
    check_mut_ptr(tv_u)?;
    let tv = time::do_gettimeofday();
    unsafe {
        *tv_u = tv;
    }
    Ok(0)
}

fn do_clock_gettime(clockid: clockid_t, ts_u: *mut timespec_t) -> Result<isize> {
    check_mut_ptr(ts_u)?;
    let clockid = time::ClockID::from_raw(clockid)?;
    let ts = time::do_clock_gettime(clockid)?;
    unsafe {
        *ts_u = ts;
    }
    Ok(0)
}

// TODO: handle remainder
fn do_nanosleep(req_u: *const timespec_t, rem_u: *mut timespec_t) -> Result<isize> {
    check_ptr(req_u)?;
    if !rem_u.is_null() {
        check_mut_ptr(rem_u)?;
    }

    let req = timespec_t::from_raw_ptr(req_u)?;
    time::do_nanosleep(&req)?;
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

fn do_unknown(
    num: u32,
    arg0: isize,
    arg1: isize,
    arg2: isize,
    arg3: isize,
    arg4: isize,
    arg5: isize,
) -> Result<isize> {
    warn!(
        "unknown or unsupported syscall (# = {}): {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
        num, arg0, arg1, arg2, arg3, arg4, arg5
    );
    return_errno!(ENOSYS, "Unknown syscall")
}

fn do_getcwd(buf: *mut u8, size: usize) -> Result<isize> {
    let safe_buf = {
        check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let proc_ref = process::get_current();
    let mut proc = proc_ref.lock().unwrap();
    let cwd = proc.get_cwd();
    if cwd.len() + 1 > safe_buf.len() {
        return_errno!(ERANGE, "buf is not long enough");
    }
    safe_buf[..cwd.len()].copy_from_slice(cwd.as_bytes());
    safe_buf[cwd.len()] = 0;
    Ok(buf as isize)
}

fn do_chdir(path: *const i8) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_chdir(&path)?;
    Ok(0)
}

fn do_rename(oldpath: *const i8, newpath: *const i8) -> Result<isize> {
    let oldpath = clone_cstring_safely(oldpath)?
        .to_string_lossy()
        .into_owned();
    let newpath = clone_cstring_safely(newpath)?
        .to_string_lossy()
        .into_owned();
    fs::do_rename(&oldpath, &newpath)?;
    Ok(0)
}

fn do_mkdir(path: *const i8, mode: usize) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_mkdir(&path, mode)?;
    Ok(0)
}

fn do_rmdir(path: *const i8) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_rmdir(&path)?;
    Ok(0)
}

fn do_link(oldpath: *const i8, newpath: *const i8) -> Result<isize> {
    let oldpath = clone_cstring_safely(oldpath)?
        .to_string_lossy()
        .into_owned();
    let newpath = clone_cstring_safely(newpath)?
        .to_string_lossy()
        .into_owned();
    fs::do_link(&oldpath, &newpath)?;
    Ok(0)
}

fn do_unlink(path: *const i8) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    fs::do_unlink(&path)?;
    Ok(0)
}

fn do_readlink(path: *const i8, buf: *mut u8, size: usize) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let buf = {
        check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = fs::do_readlink(&path, buf)?;
    Ok(len as isize)
}

fn do_sendfile(
    out_fd: FileDesc,
    in_fd: FileDesc,
    offset_ptr: *mut off_t,
    count: usize,
) -> Result<isize> {
    let offset = if offset_ptr.is_null() {
        None
    } else {
        check_mut_ptr(offset_ptr)?;
        Some(unsafe { offset_ptr.read() })
    };

    let (len, offset) = fs::do_sendfile(out_fd, in_fd, offset, count)?;
    if !offset_ptr.is_null() {
        unsafe {
            offset_ptr.write(offset as off_t);
        }
    }
    Ok(len as isize)
}

fn do_fcntl(fd: FileDesc, cmd: u32, arg: u64) -> Result<isize> {
    let cmd = FcntlCmd::from_raw(cmd, arg)?;
    fs::do_fcntl(fd, &cmd)
}

fn do_ioctl(fd: FileDesc, cmd: u32, argp: *mut u8) -> Result<isize> {
    info!("ioctl: fd: {}, cmd: {}, argp: {:?}", fd, cmd, argp);
    let mut ioctl_cmd = unsafe {
        if argp.is_null() == false {
            check_mut_ptr(argp)?;
        }
        IoctlCmd::new(cmd, argp)?
    };
    fs::do_ioctl(fd, &mut ioctl_cmd)?;
    Ok(0)
}

fn do_arch_prctl(code: u32, addr: *mut usize) -> Result<isize> {
    let code = process::ArchPrctlCode::from_u32(code)?;
    check_mut_ptr(addr)?;
    process::do_arch_prctl(code, addr).map(|_| 0)
}

fn do_set_tid_address(tidptr: *mut pid_t) -> Result<isize> {
    check_mut_ptr(tidptr)?;
    process::do_set_tid_address(tidptr).map(|tid| tid as isize)
}

fn do_sched_getaffinity(pid: pid_t, cpusize: size_t, buf: *mut c_uchar) -> Result<isize> {
    // Construct safe Rust types
    let mut buf_slice = {
        check_mut_array(buf, cpusize)?;
        if cpusize == 0 {
            return_errno!(EINVAL, "cpuset size must be greater than zero");
        }
        if buf as *const _ == std::ptr::null() {
            return_errno!(EFAULT, "cpuset mask must NOT be null");
        }
        unsafe { std::slice::from_raw_parts_mut(buf, cpusize) }
    };
    // Call the memory-safe do_sched_getaffinity
    let mut cpuset = CpuSet::new(cpusize);
    let ret = process::do_sched_getaffinity(pid, &mut cpuset)?;
    debug!("sched_getaffinity cpuset: {:#x}", cpuset);
    // Copy from Rust types to C types
    buf_slice.copy_from_slice(cpuset.as_slice());
    Ok(ret as isize)
}

fn do_sched_setaffinity(pid: pid_t, cpusize: size_t, buf: *const c_uchar) -> Result<isize> {
    // Convert unsafe C types into safe Rust types
    let cpuset = {
        check_array(buf, cpusize)?;
        if cpusize == 0 {
            return_errno!(EINVAL, "cpuset size must be greater than zero");
        }
        if buf as *const _ == std::ptr::null() {
            return_errno!(EFAULT, "cpuset mask must NOT be null");
        }
        CpuSet::from_raw_buf(buf, cpusize)
    };
    debug!("sched_setaffinity cpuset: {:#x}", cpuset);
    // Call the memory-safe do_sched_setaffinity
    let ret = process::do_sched_setaffinity(pid, &cpuset)?;
    Ok(ret as isize)
}

fn do_socket(domain: c_int, socket_type: c_int, protocol: c_int) -> Result<isize> {
    info!(
        "socket: domain: {}, socket_type: 0x{:x}, protocol: {}",
        domain, socket_type, protocol
    );

    let file_ref: Arc<Box<File>> = match domain {
        libc::AF_LOCAL => {
            let unix_socket = UnixSocketFile::new(socket_type, protocol)?;
            Arc::new(Box::new(unix_socket))
        }
        _ => {
            let socket = SocketFile::new(domain, socket_type, protocol)?;
            Arc::new(Box::new(socket))
        }
    };

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();

    let fd = proc.get_files().lock().unwrap().put(file_ref, false);
    Ok(fd as isize)
}

fn do_connect(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t) -> Result<isize> {
    info!(
        "connect: fd: {}, addr: {:?}, addr_len: {}",
        fd, addr, addr_len
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::connect(socket.fd(), addr, addr_len));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr = addr as *const libc::sockaddr_un;
        check_ptr(addr)?; // TODO: check addr_len
        let path = clone_cstring_safely(unsafe { (&*addr).sun_path.as_ptr() })?
            .to_string_lossy()
            .into_owned();
        unix_socket.connect(path)?;
        Ok(0)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_accept4(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
    flags: c_int,
) -> Result<isize> {
    info!(
        "accept4: fd: {}, addr: {:?}, addr_len: {:?}, flags: {:#x}",
        fd, addr, addr_len, flags
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let socket = file_ref.as_socket()?;

        let new_socket = socket.accept(addr, addr_len, flags)?;
        let new_file_ref: Arc<Box<File>> = Arc::new(Box::new(new_socket));
        let new_fd = proc.get_files().lock().unwrap().put(new_file_ref, false);

        Ok(new_fd as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr = addr as *mut libc::sockaddr_un;
        check_mut_ptr(addr)?; // TODO: check addr_len

        let new_socket = unix_socket.accept()?;
        let new_file_ref: Arc<Box<File>> = Arc::new(Box::new(new_socket));
        let new_fd = proc.get_files().lock().unwrap().put(new_file_ref, false);

        Ok(new_fd as isize)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_shutdown(fd: c_int, how: c_int) -> Result<isize> {
    info!("shutdown: fd: {}, how: {}", fd, how);
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::shutdown(socket.fd(), how));
        Ok(ret as isize)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_bind(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t) -> Result<isize> {
    info!("bind: fd: {}, addr: {:?}, addr_len: {}", fd, addr, addr_len);
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        check_ptr(addr)?; // TODO: check addr_len
        let ret = try_libc!(libc::ocall::bind(socket.fd(), addr, addr_len));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr = addr as *const libc::sockaddr_un;
        check_ptr(addr)?; // TODO: check addr_len
        let path = clone_cstring_safely(unsafe { (&*addr).sun_path.as_ptr() })?
            .to_string_lossy()
            .into_owned();
        unix_socket.bind(path)?;
        Ok(0)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_listen(fd: c_int, backlog: c_int) -> Result<isize> {
    info!("listen: fd: {}, backlog: {}", fd, backlog);
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::listen(socket.fd(), backlog));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket.listen()?;
        Ok(0)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_setsockopt(
    fd: c_int,
    level: c_int,
    optname: c_int,
    optval: *const c_void,
    optlen: libc::socklen_t,
) -> Result<isize> {
    info!(
        "setsockopt: fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd, level, optname, optval, optlen
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::setsockopt(
            socket.fd(),
            level,
            optname,
            optval,
            optlen
        ));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("setsockopt for unix socket is unimplemented");
        Ok(0)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_getsockopt(
    fd: c_int,
    level: c_int,
    optname: c_int,
    optval: *mut c_void,
    optlen: *mut libc::socklen_t,
) -> Result<isize> {
    info!(
        "getsockopt: fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd, level, optname, optval, optlen
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    let socket = file_ref.as_socket()?;

    let ret = try_libc!(libc::ocall::getsockopt(
        socket.fd(),
        level,
        optname,
        optval,
        optlen
    ));
    Ok(ret as isize)
}

fn do_getpeername(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    info!(
        "getpeername: fd: {}, addr: {:?}, addr_len: {:?}",
        fd, addr, addr_len
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::getpeername(socket.fd(), addr, addr_len));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("getpeername for unix socket is unimplemented");
        return_errno!(
            ENOTCONN,
            "hack for php: Transport endpoint is not connected"
        )
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_getsockname(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    info!(
        "getsockname: fd: {}, addr: {:?}, addr_len: {:?}",
        fd, addr, addr_len
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::getsockname(socket.fd(), addr, addr_len));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("getsockname for unix socket is unimplemented");
        Ok(0)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_sendto(
    fd: c_int,
    base: *const c_void,
    len: size_t,
    flags: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    info!(
        "sendto: fd: {}, base: {:?}, len: {}, addr: {:?}, addr_len: {}",
        fd, base, len, addr, addr_len
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    let socket = file_ref.as_socket()?;

    let ret = try_libc!(libc::ocall::sendto(
        socket.fd(),
        base,
        len,
        flags,
        addr,
        addr_len
    ));
    Ok(ret as isize)
}

fn do_recvfrom(
    fd: c_int,
    base: *mut c_void,
    len: size_t,
    flags: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    info!(
        "recvfrom: fd: {}, base: {:?}, len: {}, flags: {}, addr: {:?}, addr_len: {:?}",
        fd, base, len, flags, addr, addr_len
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;
    let socket = file_ref.as_socket()?;

    let ret = try_libc!(libc::ocall::recvfrom(
        socket.fd(),
        base,
        len,
        flags,
        addr,
        addr_len
    ));
    Ok(ret as isize)
}

fn do_socketpair(
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
    sv: *mut c_int,
) -> Result<isize> {
    info!(
        "socketpair: domain: {}, type:0x{:x}, protocol: {}",
        domain, socket_type, protocol
    );
    let mut sock_pair = unsafe {
        check_mut_array(sv, 2)?;
        std::slice::from_raw_parts_mut(sv as *mut u32, 2)
    };

    if (domain == libc::AF_UNIX) {
        let (client_socket, server_socket) =
            UnixSocketFile::socketpair(socket_type as i32, protocol as i32)?;
        let current_ref = process::get_current();
        let mut proc = current_ref.lock().unwrap();
        sock_pair[0] = proc
            .get_files()
            .lock()
            .unwrap()
            .put(Arc::new(Box::new(client_socket)), false);
        sock_pair[1] = proc
            .get_files()
            .lock()
            .unwrap()
            .put(Arc::new(Box::new(server_socket)), false);

        info!("socketpair: ({}, {})", sock_pair[0], sock_pair[1]);
        Ok(0)
    } else if (domain == libc::AF_TIPC) {
        return_errno!(EAFNOSUPPORT, "cluster domain sockets not supported")
    } else {
        return_errno!(EAFNOSUPPORT, "domain not supported")
    }
}

fn do_select(
    nfds: c_int,
    readfds: *mut libc::fd_set,
    writefds: *mut libc::fd_set,
    exceptfds: *mut libc::fd_set,
    timeout: *const libc::timeval,
) -> Result<isize> {
    // check arguments
    if nfds < 0 || nfds >= libc::FD_SETSIZE as c_int {
        return_errno!(EINVAL, "nfds is negative or exceeds the resource limit");
    }
    let nfds = nfds as usize;

    let mut zero_fds0: libc::fd_set = unsafe { core::mem::zeroed() };
    let mut zero_fds1: libc::fd_set = unsafe { core::mem::zeroed() };
    let mut zero_fds2: libc::fd_set = unsafe { core::mem::zeroed() };

    let readfds = if !readfds.is_null() {
        check_mut_ptr(readfds)?;
        unsafe { &mut *readfds }
    } else {
        &mut zero_fds0
    };
    let writefds = if !writefds.is_null() {
        check_mut_ptr(writefds)?;
        unsafe { &mut *writefds }
    } else {
        &mut zero_fds1
    };
    let exceptfds = if !exceptfds.is_null() {
        check_mut_ptr(exceptfds)?;
        unsafe { &mut *exceptfds }
    } else {
        &mut zero_fds2
    };
    let timeout = if !timeout.is_null() {
        check_ptr(timeout)?;
        Some(unsafe { timeout.read() })
    } else {
        None
    };

    let n = fs::do_select(nfds, readfds, writefds, exceptfds, timeout)?;
    Ok(n as isize)
}

fn do_poll(fds: *mut libc::pollfd, nfds: libc::nfds_t, timeout: c_int) -> Result<isize> {
    check_mut_array(fds, nfds as usize)?;
    let polls = unsafe { std::slice::from_raw_parts_mut(fds, nfds as usize) };

    let n = fs::do_poll(polls, timeout)?;
    Ok(n as isize)
}

fn do_epoll_create(size: c_int) -> Result<isize> {
    if size <= 0 {
        return_errno!(EINVAL, "size is not positive");
    }
    do_epoll_create1(0)
}

fn do_epoll_create1(flags: c_int) -> Result<isize> {
    let fd = fs::do_epoll_create1(flags)?;
    Ok(fd as isize)
}

fn do_epoll_ctl(
    epfd: c_int,
    op: c_int,
    fd: c_int,
    event: *const libc::epoll_event,
) -> Result<isize> {
    if !event.is_null() {
        check_ptr(event)?;
    }
    fs::do_epoll_ctl(epfd as FileDesc, op, fd as FileDesc, event)?;
    Ok(0)
}

fn do_epoll_wait(
    epfd: c_int,
    events: *mut libc::epoll_event,
    maxevents: c_int,
    timeout: c_int,
) -> Result<isize> {
    let maxevents = {
        if maxevents <= 0 {
            return_errno!(EINVAL, "maxevents <= 0");
        }
        maxevents as usize
    };
    let events = {
        check_mut_array(events, maxevents)?;
        unsafe { std::slice::from_raw_parts_mut(events, maxevents) }
    };
    let count = fs::do_epoll_wait(epfd as FileDesc, events, timeout)?;
    Ok(count as isize)
}

fn do_epoll_pwait(
    epfd: c_int,
    events: *mut libc::epoll_event,
    maxevents: c_int,
    timeout: c_int,
    sigmask: *const usize, //TODO:add sigset_t
) -> Result<isize> {
    info!("epoll_pwait");
    //TODO:add signal support
    do_epoll_wait(epfd, events, maxevents, 0)
}

fn do_uname(name: *mut utsname_t) -> Result<isize> {
    check_mut_ptr(name)?;
    let name = unsafe { &mut *name };
    misc::do_uname(name).map(|_| 0)
}

fn do_prlimit(
    pid: pid_t,
    resource: u32,
    new_limit: *const rlimit_t,
    old_limit: *mut rlimit_t,
) -> Result<isize> {
    let resource = resource_t::from_u32(resource)?;
    let new_limit = {
        if new_limit != ptr::null() {
            check_ptr(new_limit)?;
            Some(unsafe { &*new_limit })
        } else {
            None
        }
    };
    let old_limit = {
        if old_limit != ptr::null_mut() {
            check_mut_ptr(old_limit)?;
            Some(unsafe { &mut *old_limit })
        } else {
            None
        }
    };
    misc::do_prlimit(pid, resource, new_limit, old_limit).map(|_| 0)
}

fn do_access(path: *const i8, mode: u32) -> Result<isize> {
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let mode = AccessModes::from_u32(mode)?;
    fs::do_access(&path, mode).map(|_| 0)
}

fn do_faccessat(dirfd: i32, path: *const i8, mode: u32, flags: u32) -> Result<isize> {
    let dirfd = if dirfd >= 0 {
        Some(dirfd as FileDesc)
    } else if dirfd == AT_FDCWD {
        None
    } else {
        return_errno!(EINVAL, "invalid dirfd");
    };
    let path = clone_cstring_safely(path)?.to_string_lossy().into_owned();
    let mode = AccessModes::from_u32(mode)?;
    let flags = AccessFlags::from_u32(flags)?;
    fs::do_faccessat(dirfd, &path, mode, flags).map(|_| 0)
}

// TODO: implement signals

fn do_rt_sigaction() -> Result<isize> {
    Ok(0)
}

fn do_rt_sigprocmask() -> Result<isize> {
    Ok(0)
}

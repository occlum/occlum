//! System call handler
//!
//! # Syscall processing flow
//!
//! 1. User call `__occlum_syscall` (at `syscall_entry_x86_64.S`)
//! 2. Do some bound checks then call `dispatch_syscall` (at this file)
//! 3. Dispatch the syscall to `do_*` (at this file)
//! 4. Do some memory checks then call `mod::do_*` (at each module)
pub use self::syscall_num::SyscallNum;

use fs::{File, FileDesc, FileRef, Stat};
use misc::{resource_t, rlimit_t, utsname_t};
use net::{msghdr, msghdr_mut, AsSocket, AsUnixSocket, SocketFile, UnixSocketFile};
use process::{pid_t, ChildProcessFilter, CloneFlags, CpuSet, FileAction, FutexFlags, FutexOp};
use std::any::Any;
use std::convert::TryFrom;
use std::ffi::{CStr, CString};
use std::io::{Read, Seek, SeekFrom, Write};
use std::ptr;
use time::{clockid_t, timespec_t, timeval_t, GLOBAL_PROFILER};
use util::mem_util::from_user::*;
use vm::{MMapFlags, VMPerms};
use {fs, process, std, vm};

use super::*;

mod syscall_num;

// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

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
    let pid = process::do_gettid();
    let syscall_num = SyscallNum::try_from(num).unwrap();

    debug!(
        "syscall tid:{}, {:?}: {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
        pid, syscall_num, arg0, arg1, arg2, arg3, arg4, arg5
    );

    #[cfg(feature = "syscall_timing")]
    GLOBAL_PROFILER
        .lock()
        .unwrap()
        .syscall_enter(syscall_num)
        .expect("unexpected error from profiler to enter syscall");

    use self::syscall_num::SyscallNum::*;
    let ret = match syscall_num {
        // file
        SysOpen => fs::do_open(arg0 as *const i8, arg1 as u32, arg2 as u32),
        SysClose => fs::do_close(arg0 as FileDesc),
        SysRead => fs::do_read(arg0 as FileDesc, arg1 as *mut u8, arg2 as usize),
        SysWrite => fs::do_write(arg0 as FileDesc, arg1 as *const u8, arg2 as usize),
        SysPread64 => fs::do_pread(
            arg0 as FileDesc,
            arg1 as *mut u8,
            arg2 as usize,
            arg3 as usize,
        ),
        SysPwrite64 => fs::do_pwrite(
            arg0 as FileDesc,
            arg1 as *const u8,
            arg2 as usize,
            arg3 as usize,
        ),
        SysReadv => fs::do_readv(arg0 as FileDesc, arg1 as *mut fs::iovec_t, arg2 as i32),
        SysWritev => fs::do_writev(arg0 as FileDesc, arg1 as *mut fs::iovec_t, arg2 as i32),
        SysStat => fs::do_stat(arg0 as *const i8, arg1 as *mut Stat),
        SysFstat => fs::do_fstat(arg0 as FileDesc, arg1 as *mut Stat),
        SysLstat => fs::do_lstat(arg0 as *const i8, arg1 as *mut Stat),
        SysAccess => fs::do_access(arg0 as *const i8, arg1 as u32),
        SysFaccessat => fs::do_faccessat(arg0 as i32, arg1 as *const i8, arg2 as u32, arg3 as u32),
        SysLseek => fs::do_lseek(arg0 as FileDesc, arg1 as off_t, arg2 as i32),
        SysFsync => fs::do_fsync(arg0 as FileDesc),
        SysFdatasync => fs::do_fdatasync(arg0 as FileDesc),
        SysTruncate => fs::do_truncate(arg0 as *const i8, arg1 as usize),
        SysFtruncate => fs::do_ftruncate(arg0 as FileDesc, arg1 as usize),
        SysGetdents64 => fs::do_getdents64(arg0 as FileDesc, arg1 as *mut u8, arg2 as usize),
        SysSync => fs::do_sync(),
        SysGetcwd => do_getcwd(arg0 as *mut u8, arg1 as usize),
        SysChdir => fs::do_chdir(arg0 as *mut i8),
        SysRename => fs::do_rename(arg0 as *const i8, arg1 as *const i8),
        SysMkdir => fs::do_mkdir(arg0 as *const i8, arg1 as usize),
        SysRmdir => fs::do_rmdir(arg0 as *const i8),
        SysLink => fs::do_link(arg0 as *const i8, arg1 as *const i8),
        SysUnlink => fs::do_unlink(arg0 as *const i8),
        SysReadlink => fs::do_readlink(arg0 as *const i8, arg1 as *mut u8, arg2 as usize),
        SysSendfile => fs::do_sendfile(
            arg0 as FileDesc,
            arg1 as FileDesc,
            arg2 as *mut off_t,
            arg3 as usize,
        ),
        SysFcntl => fs::do_fcntl(arg0 as FileDesc, arg1 as u32, arg2 as u64),
        SysIoctl => fs::do_ioctl(arg0 as FileDesc, arg1 as u32, arg2 as *mut u8),

        // Io multiplexing
        SysSelect => net::do_select(
            arg0 as c_int,
            arg1 as *mut libc::fd_set,
            arg2 as *mut libc::fd_set,
            arg3 as *mut libc::fd_set,
            arg4 as *const libc::timeval,
        ),
        SysPoll => net::do_poll(
            arg0 as *mut libc::pollfd,
            arg1 as libc::nfds_t,
            arg2 as c_int,
        ),
        SysEpollCreate => net::do_epoll_create(arg0 as c_int),
        SysEpollCreate1 => net::do_epoll_create1(arg0 as c_int),
        SysEpollCtl => net::do_epoll_ctl(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *const libc::epoll_event,
        ),
        SysEpollWait => net::do_epoll_wait(
            arg0 as c_int,
            arg1 as *mut libc::epoll_event,
            arg2 as c_int,
            arg3 as c_int,
        ),
        SysEpollPwait => net::do_epoll_pwait(
            arg0 as c_int,
            arg1 as *mut libc::epoll_event,
            arg2 as c_int,
            arg3 as c_int,
            arg4 as *const usize, //Todo:add sigset_t
        ),

        // process
        SysExit => do_exit(arg0 as i32),
        SysSpawn => do_spawn(
            arg0 as *mut u32,
            arg1 as *mut i8,
            arg2 as *const *const i8,
            arg3 as *const *const i8,
            arg4 as *const FdOp,
        ),
        SysWait4 => do_wait4(arg0 as i32, arg1 as *mut i32),

        SysGetpid => do_getpid(),
        SysGettid => do_gettid(),
        SysGetppid => do_getppid(),
        SysGetpgid => do_getpgid(),

        SysGetuid => do_getuid(),
        SysGetgid => do_getgid(),
        SysGeteuid => do_geteuid(),
        SysGetegid => do_getegid(),

        SysRtSigaction => do_rt_sigaction(),
        SysRtSigprocmask => do_rt_sigprocmask(),

        SysClone => do_clone(
            arg0 as u32,
            arg1 as usize,
            arg2 as *mut pid_t,
            arg3 as *mut pid_t,
            arg4 as usize,
        ),
        SysFutex => do_futex(
            arg0 as *const i32,
            arg1 as u32,
            arg2 as i32,
            arg3 as i32,
            arg4 as *const i32,
            // Todo: accept other optional arguments
        ),
        SysArchPrctl => do_arch_prctl(arg0 as u32, arg1 as *mut usize),
        SysSetTidAddress => do_set_tid_address(arg0 as *mut pid_t),

        // sched
        SysSchedYield => do_sched_yield(),
        SysSchedGetaffinity => {
            do_sched_getaffinity(arg0 as pid_t, arg1 as size_t, arg2 as *mut c_uchar)
        }
        SysSchedSetaffinity => {
            do_sched_setaffinity(arg0 as pid_t, arg1 as size_t, arg2 as *const c_uchar)
        }

        // memory
        SysMmap => do_mmap(
            arg0 as usize,
            arg1 as usize,
            arg2 as i32,
            arg3 as i32,
            arg4 as FileDesc,
            arg5 as off_t,
        ),
        SysMunmap => do_munmap(arg0 as usize, arg1 as usize),
        SysMremap => do_mremap(
            arg0 as usize,
            arg1 as usize,
            arg2 as usize,
            arg3 as i32,
            arg4 as usize,
        ),
        SysMprotect => do_mprotect(arg0 as usize, arg1 as usize, arg2 as u32),
        SysBrk => do_brk(arg0 as usize),

        SysPipe => fs::do_pipe2(arg0 as *mut i32, 0),
        SysPipe2 => fs::do_pipe2(arg0 as *mut i32, arg1 as u32),
        SysDup => fs::do_dup(arg0 as FileDesc),
        SysDup2 => fs::do_dup2(arg0 as FileDesc, arg1 as FileDesc),
        SysDup3 => fs::do_dup3(arg0 as FileDesc, arg1 as FileDesc, arg2 as u32),

        SysGettimeofday => do_gettimeofday(arg0 as *mut timeval_t),
        SysClockGettime => do_clock_gettime(arg0 as clockid_t, arg1 as *mut timespec_t),

        SysNanosleep => do_nanosleep(arg0 as *const timespec_t, arg1 as *mut timespec_t),

        SysUname => do_uname(arg0 as *mut utsname_t),

        SysPrlimit64 => do_prlimit(
            arg0 as pid_t,
            arg1 as u32,
            arg2 as *const rlimit_t,
            arg3 as *mut rlimit_t,
        ),

        // socket
        SysSocket => do_socket(arg0 as c_int, arg1 as c_int, arg2 as c_int),
        SysConnect => do_connect(
            arg0 as c_int,
            arg1 as *const libc::sockaddr,
            arg2 as libc::socklen_t,
        ),
        SysAccept => do_accept4(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
            0,
        ),
        SysAccept4 => do_accept4(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
            arg3 as c_int,
        ),
        SysShutdown => do_shutdown(arg0 as c_int, arg1 as c_int),
        SysBind => do_bind(
            arg0 as c_int,
            arg1 as *const libc::sockaddr,
            arg2 as libc::socklen_t,
        ),
        SysListen => do_listen(arg0 as c_int, arg1 as c_int),
        SysSetsockopt => do_setsockopt(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *const c_void,
            arg4 as libc::socklen_t,
        ),
        SysGetsockopt => do_getsockopt(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *mut c_void,
            arg4 as *mut libc::socklen_t,
        ),
        SysGetpeername => do_getpeername(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
        ),
        SysGetsockname => do_getsockname(
            arg0 as c_int,
            arg1 as *mut libc::sockaddr,
            arg2 as *mut libc::socklen_t,
        ),
        SysSendto => do_sendto(
            arg0 as c_int,
            arg1 as *const c_void,
            arg2 as size_t,
            arg3 as c_int,
            arg4 as *const libc::sockaddr,
            arg5 as libc::socklen_t,
        ),
        SysRecvfrom => do_recvfrom(
            arg0 as c_int,
            arg1 as *mut c_void,
            arg2 as size_t,
            arg3 as c_int,
            arg4 as *mut libc::sockaddr,
            arg5 as *mut libc::socklen_t,
        ),

        SysSocketpair => do_socketpair(
            arg0 as c_int,
            arg1 as c_int,
            arg2 as c_int,
            arg3 as *mut c_int,
        ),

        SysSendmsg => net::do_sendmsg(arg0 as c_int, arg1 as *const msghdr, arg2 as c_int),
        SysRecvmsg => net::do_recvmsg(arg0 as c_int, arg1 as *mut msghdr_mut, arg2 as c_int),

        _ => do_unknown(num, arg0, arg1, arg2, arg3, arg4, arg5),
    };

    #[cfg(feature = "syscall_timing")]
    GLOBAL_PROFILER
        .lock()
        .unwrap()
        .syscall_exit(syscall_num, ret.is_err())
        .expect("unexpected error from profiler to exit syscall");

    info!("tid: {} => {:?} ", process::do_gettid(), ret);

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

pub fn do_futex(
    futex_addr: *const i32,
    futex_op: u32,
    futex_val: i32,
    timeout: i32,
    futex_new_addr: *const i32,
) -> Result<isize> {
    check_ptr(futex_addr)?;
    let (futex_op, futex_flags) = process::futex_op_and_flags_from_u32(futex_op)?;

    let get_futex_val = |val| -> Result<usize> {
        if val < 0 {
            return_errno!(EINVAL, "the futex val must not be negative");
        }
        Ok(val as usize)
    };

    match futex_op {
        FutexOp::FUTEX_WAIT => process::futex_wait(futex_addr, futex_val).map(|_| 0),
        FutexOp::FUTEX_WAKE => {
            let max_count = get_futex_val(futex_val)?;
            process::futex_wake(futex_addr, max_count).map(|count| count as isize)
        }
        FutexOp::FUTEX_REQUEUE => {
            check_ptr(futex_new_addr)?;
            let max_nwakes = get_futex_val(futex_val)?;
            let max_nrequeues = get_futex_val(timeout)?;
            process::futex_requeue(futex_addr, max_nwakes, max_nrequeues, futex_new_addr)
                .map(|nwakes| nwakes as isize)
        }
        _ => return_errno!(ENOSYS, "the futex operation is not supported"),
    }
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

fn do_arch_prctl(code: u32, addr: *mut usize) -> Result<isize> {
    let code = process::ArchPrctlCode::from_u32(code)?;
    check_mut_ptr(addr)?;
    process::do_arch_prctl(code, addr).map(|_| 0)
}

fn do_set_tid_address(tidptr: *mut pid_t) -> Result<isize> {
    check_mut_ptr(tidptr)?;
    process::do_set_tid_address(tidptr).map(|tid| tid as isize)
}

fn do_sched_yield() -> Result<isize> {
    process::do_sched_yield();
    Ok(0)
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
    let retval = process::do_sched_getaffinity(pid, &mut cpuset)?;
    // Copy from Rust types to C types
    buf_slice.copy_from_slice(cpuset.as_slice());
    Ok(retval as isize)
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
    process::do_sched_setaffinity(pid, &cpuset)?;
    Ok(0)
}

fn do_socket(domain: c_int, socket_type: c_int, protocol: c_int) -> Result<isize> {
    info!(
        "socket: domain: {}, socket_type: 0x{:x}, protocol: {}",
        domain, socket_type, protocol
    );

    let file_ref: Arc<Box<dyn File>> = match domain {
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
        let new_file_ref: Arc<Box<dyn File>> = Arc::new(Box::new(new_socket));
        let new_fd = proc.get_files().lock().unwrap().put(new_file_ref, false);

        Ok(new_fd as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr = addr as *mut libc::sockaddr_un;
        check_mut_ptr(addr)?; // TODO: check addr_len

        let new_socket = unix_socket.accept()?;
        let new_file_ref: Arc<Box<dyn File>> = Arc::new(Box::new(new_socket));
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

// TODO: implement signals

fn do_rt_sigaction() -> Result<isize> {
    Ok(0)
}

fn do_rt_sigprocmask() -> Result<isize> {
    Ok(0)
}

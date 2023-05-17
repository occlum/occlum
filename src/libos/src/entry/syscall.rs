//! System call handler

use std::any::Any;
use std::convert::TryFrom;
use std::default::Default;
use std::ffi::{CStr, CString};
use std::io::{Read, Seek, SeekFrom, Write};
use std::ptr;

use crate::config::user_rootfs_config;

use crate::fs::{
    do_access, do_chdir, do_chmod, do_chown, do_close, do_creat, do_dup, do_dup2, do_dup3,
    do_eventfd, do_eventfd2, do_faccessat, do_fallocate, do_fchdir, do_fchmod, do_fchmodat,
    do_fchown, do_fchownat, do_fcntl, do_fdatasync, do_flock, do_fstat, do_fstatat, do_fstatfs,
    do_fsync, do_ftruncate, do_futimesat, do_getcwd, do_getdents, do_getdents64, do_ioctl,
    do_lchown, do_link, do_linkat, do_lseek, do_lstat, do_mkdir, do_mkdirat, do_mount,
    do_mount_rootfs, do_open, do_openat, do_pipe, do_pipe2, do_pread, do_preadv, do_pwrite,
    do_pwritev, do_read, do_readlink, do_readlinkat, do_readv, do_rename, do_renameat, do_rmdir,
    do_sendfile, do_stat, do_statfs, do_symlink, do_symlinkat, do_sync, do_truncate, do_umask,
    do_umount, do_unlink, do_unlinkat, do_utime, do_utimensat, do_utimes, do_write, do_writev,
    iovec_t, utimbuf_t, FileDesc, FileRef, StatBuf, Statfs,
};
use crate::ipc::{do_shmat, do_shmctl, do_shmdt, do_shmget, key_t, shmids_t};
use crate::misc::{resource_t, rlimit_t, sysinfo_t, utsname_t, RandFlags};
use crate::net::{
    do_accept, do_accept4, do_bind, do_connect, do_getpeername, do_getsockname, do_getsockopt,
    do_listen, do_recvfrom, do_recvmsg, do_sendmmsg, do_sendmsg, do_sendto, do_setsockopt,
    do_shutdown, do_socket, do_socketpair, mmsghdr,
};
use crate::poll::syscalls::{
    do_epoll_create, do_epoll_create1, do_epoll_ctl, do_epoll_pwait, do_epoll_wait, do_poll,
    do_ppoll, do_select,
};
use crate::prelude::*;
use crate::process::{
    do_arch_prctl, do_clone, do_execve, do_exit, do_exit_group, do_futex, do_get_robust_list,
    do_getegid, do_geteuid, do_getgid, do_getgroups, do_getpgid, do_getpgrp, do_getpid, do_getppid,
    do_gettid, do_getuid, do_prctl, do_set_robust_list, do_set_tid_address, do_setpgid,
    do_spawn_for_glibc, do_spawn_for_musl, do_vfork, do_wait4, pid_t, posix_spawnattr_t, FdOp,
    RobustListHead, SpawnFileActions, ThreadRef, ThreadStatus,
};
use crate::sched::{
    do_get_priority, do_getcpu, do_sched_getaffinity, do_sched_setaffinity, do_sched_yield,
    do_set_priority,
};
use crate::signal::{
    do_kill, do_rt_sigaction, do_rt_sigpending, do_rt_sigprocmask, do_rt_sigreturn,
    do_rt_sigtimedwait, do_sigaltstack, do_tgkill, do_tkill, sigaction_t, siginfo_t, sigset_t,
    stack_t,
};
use crate::time::{
    clockid_t, do_timerfd_create, do_timerfd_gettime, do_timerfd_settime, itimerspec_t, timespec_t,
    timeval_t, ClockId,
};
use crate::util::log::{self, LevelFilter};
use crate::util::mem_util::from_user::*;
use crate::vm::{MMapFlags, MRemapFlags, MSyncFlags, VMPerms};
use crate::{fs, process, std, vm};

use super::context_switch::{self, CpuContext, Fault, FpRegs, GpRegs, CURRENT_CONTEXT};

pub async fn handle_syscall() -> Result<()> {
    // Extract arguments from the CPU context. The arguments follows Linux's syscall ABI.
    let mut syscall = CURRENT_CONTEXT.with(|_context| {
        let context = _context.borrow();
        let gp_regs = &context.gp_regs;
        let num = gp_regs.rax as u32;
        let arg0 = gp_regs.rdi as isize;
        let arg1 = gp_regs.rsi as isize;
        let arg2 = gp_regs.rdx as isize;
        let arg3 = gp_regs.r10 as isize;
        let arg4 = gp_regs.r8 as isize;
        let arg5 = gp_regs.r9 as isize;
        Syscall::new(num, arg0, arg1, arg2, arg3, arg4, arg5)
    });
    if let Err(ref e) = syscall {
        let retval = -(e.errno() as i64);
        CURRENT_CONTEXT.with(|context| {
            context.borrow_mut().gp_regs.rax = retval as u64;
        });
    }

    let mut syscall = syscall?;
    let syscall_num = syscall.num;

    log::set_round_desc(Some(syscall_num.as_str()));
    trace!("{:?}", &syscall);

    let syscall_res = dispatch_syscall(syscall).await;

    // Put the return value into rax, except for syscalls that may modify CPU
    // Context directly. Currently, there is only one succh syscall: SigReturn.
    //
    // Sigreturn restores user's CPU state to the state when the last signal
    // handler is executed. So in the case of sigreturn, the user CPU context's
    // rax should not be updated with the return value of the syscall.
    if syscall_num != SyscallNum::RtSigreturn {
        let retval = match &syscall_res {
            Ok(retval) => *retval as i64,
            Err(e) => -(e.errno() as i64),
        };
        CURRENT_CONTEXT.with(|context| {
            context.borrow_mut().gp_regs.rax = retval as u64;
        });
        trace!("ret = 0x{:x}", retval);
    }

    syscall_res.map(|_| ())
}

/// System call table defined in a macro.
///
/// To keep the info about system calls in a centralized place and avoid redundant code, the system
/// call table is defined in this macro. This macro takes as input a callback macro, which
/// can then process the system call table defined in this macro and generated code accordingly.
///
/// # Why callback?
///
/// Since the system call table is quite big, we do not want to repeat it more than once in code. But
/// passing a block of grammarly malformed code (such as the system call table shown below) seems
/// difficult due to some limitations of Rust macros.
///
/// So instead of passing the syscall table to another macro, we do this the other way around: accepting
/// a macro callback as input, and then internally pass the system call table to the callback.
macro_rules! process_syscall_table_with_callback {
    ($callback: ident) => {
        $callback! {
            // System call table.
            //
            // Format:
            // (<SyscallName> = <SyscallNum>) => <SyscallFunc>(<SyscallArgs>),
            //
            // If the system call is implemented, <SyscallFunc> is the function that implements the system call.
            // Otherwise, it is set to an proper error handler function.
            //
            // Limitation:
            // <SyscallFunc> must be an identifier, not a path.
            //
            // TODO: Unify the use of C types. For example, u8 or i8 or char_c for C string?

            (Read = 0) => do_read(fd: FileDesc, buf: *mut u8, size: usize),
            (Write = 1) => do_write(fd: FileDesc, buf: *const u8, size: usize),
            (Open = 2) => do_open(path: *const i8, flags: u32, mode: u16),
            (Close = 3) => do_close(fd: FileDesc),
            (Stat = 4) => do_stat(path: *const i8, stat_buf: *mut StatBuf),
            (Fstat = 5) => do_fstat(fd: FileDesc, stat_buf: *mut StatBuf),
            (Lstat = 6) => do_lstat(path: *const i8, stat_buf: *mut StatBuf),
            (Poll = 7) => do_poll(fds: *mut libc::pollfd, nfds: libc::nfds_t, timeout: c_int),
            (Lseek = 8) => do_lseek(fd: FileDesc, offset: off_t, whence: i32),
            (Mmap = 9) => do_mmap(addr: usize, size: usize, perms: i32, flags: i32, fd: FileDesc, offset: off_t),
            (Mprotect = 10) => do_mprotect(addr: usize, len: usize, prot: u32),
            (Munmap = 11) => do_munmap(addr: usize, size: usize),
            (Brk = 12) => do_brk(new_brk_addr: usize),
            (RtSigaction = 13) => do_rt_sigaction(signum_c: c_int, new_sa_c: *const sigaction_t, old_sa_c: *mut sigaction_t),
            (RtSigprocmask = 14) => do_rt_sigprocmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t, sigset_size: size_t),
            (RtSigreturn = 15) => do_rt_sigreturn(),
            (Ioctl = 16) => do_ioctl(fd: FileDesc, cmd: u32, argp: *mut u8),
            (Pread64 = 17) => do_pread(fd: FileDesc, buf: *mut u8, size: usize, offset: off_t),
            (Pwrite64 = 18) => do_pwrite(fd: FileDesc, buf: *const u8, size: usize, offset: off_t),
            (Readv = 19) => do_readv(fd: FileDesc, iov: *mut iovec_t, count: i32),
            (Writev = 20) => do_writev(fd: FileDesc, iov: *const iovec_t, count: i32),
            (Access = 21) => do_access(path: *const i8, mode: u32),
            (Pipe = 22) => do_pipe(fds_u: *mut i32),
            (Select = 23) => do_select(nfds: c_int, readfds: *mut libc::fd_set, writefds: *mut libc::fd_set, exceptfds: *mut libc::fd_set, timeout: *mut timeval_t),
            (SchedYield = 24) => do_sched_yield(),
            (Mremap = 25) => do_mremap(old_addr: usize, old_size: usize, new_size: usize, flags: i32, new_addr: usize),
            (Msync = 26) => do_msync(addr: usize, size: usize, flags: u32),
            (Mincore = 27) => handle_unsupported(),
            (Madvise = 28) => handle_unsupported(),
            (Shmget = 29) => do_shmget(key: key_t, size: size_t, shmflg: i32),
            (Shmat = 30) => do_shmat(shmid: i32, shmaddr: usize, shmflg: i32),
            (Shmctl = 31) => do_shmctl(shmid: i32, cmd: i32, buf: *mut shmids_t),
            (Dup = 32) => do_dup(old_fd: FileDesc),
            (Dup2 = 33) => do_dup2(old_fd: FileDesc, new_fd: FileDesc),
            (Pause = 34) => handle_unsupported(),
            (Nanosleep = 35) => do_nanosleep(req_u: *const timespec_t, rem_u: *mut timespec_t),
            (Getitimer = 36) => handle_unsupported(),
            (Alarm = 37) => handle_unsupported(),
            (Setitimer = 38) => handle_unsupported(),
            (Getpid = 39) => do_getpid(),
            (Sendfile = 40) => do_sendfile(out_fd: FileDesc, in_fd: FileDesc, offset_ptr: *mut off_t, count: isize),
            (Socket = 41) => do_socket(domain: c_int, socket_type: c_int, protocol: c_int),
            (Connect = 42) => do_connect(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t),
            (Accept = 43) => do_accept(fd: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t),
            (Sendto = 44) => do_sendto(fd: c_int, base: *const c_void, len: size_t, flags: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t),
            (Recvfrom = 45) => do_recvfrom(fd: c_int, base: *mut c_void, len: size_t, flags: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t),
            (Sendmsg = 46) => do_sendmsg(fd: c_int, msg_ptr: *const libc::msghdr, flags: c_int),
            (Recvmsg = 47) => do_recvmsg(fd: c_int, msg_mut_ptr: *mut libc::msghdr, flags: c_int),
            (Shutdown = 48) => do_shutdown(fd: c_int, how: c_int),
            (Bind = 49) => do_bind(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t),
            (Listen = 50) => do_listen(fd: c_int, backlog: c_int),
            (Getsockname = 51) => do_getsockname(fd: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t),
            (Getpeername = 52) => do_getpeername(fd: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t),
            (Socketpair = 53) => do_socketpair(domain: c_int, socket_type: c_int, protocol: c_int, sv: *mut c_int),
            (Setsockopt = 54) => do_setsockopt(fd: c_int, level: c_int, optname: c_int, optval: *const c_void, optlen: libc::socklen_t),
            (Getsockopt = 55) => do_getsockopt(fd: c_int, level: c_int, optname: c_int, optval: *mut c_void, optlen: *mut libc::socklen_t),
            (Clone = 56) => do_clone(flags: u32, stack_addr: usize, ptid: *mut pid_t, ctid: *mut pid_t, new_tls: usize),
            (Fork = 57) => handle_unsupported(),
            (Vfork = 58) => do_vfork(),
            (Execve = 59) => do_execve(path: *const i8, argv: *const *const i8, envp: *const *const i8),
            (Exit = 60) => do_exit(exit_status: i32),
            (Wait4 = 61) => do_wait4(pid: i32, _exit_status: *mut i32, options: u32),
            (Kill = 62) => do_kill(pid: i32, sig: c_int),
            (Uname = 63) => do_uname(name: *mut utsname_t),
            (Semget = 64) => handle_unsupported(),
            (Semop = 65) => handle_unsupported(),
            (Semctl = 66) => handle_unsupported(),
            (Shmdt = 67) => do_shmdt(shmaddr: usize),
            (Msgget = 68) => handle_unsupported(),
            (Msgsnd = 69) => handle_unsupported(),
            (Msgrcv = 70) => handle_unsupported(),
            (Msgctl = 71) => handle_unsupported(),
            (Fcntl = 72) => do_fcntl(fd: FileDesc, cmd: u32, arg: u64),
            (Flock = 73) => do_flock(fd: FileDesc, operation: i32),
            (Fsync = 74) => do_fsync(fd: FileDesc),
            (Fdatasync = 75) => do_fdatasync(fd: FileDesc),
            (Truncate = 76) => do_truncate(path: *const i8, len: usize),
            (Ftruncate = 77) => do_ftruncate(fd: FileDesc, len: usize),
            (Getdents = 78) => do_getdents(fd: FileDesc, buf: *mut u8, buf_size: usize),
            (Getcwd = 79) => do_getcwd(buf: *mut u8, size: usize),
            (Chdir = 80) => do_chdir(path: *const i8),
            (Fchdir = 81) => do_fchdir(fd: FileDesc),
            (Rename = 82) => do_rename(oldpath: *const i8, newpath: *const i8),
            (Mkdir = 83) => do_mkdir(path: *const i8, mode: u16),
            (Rmdir = 84) => do_rmdir(path: *const i8),
            (Creat = 85) => do_creat(path: *const i8, mode: u16),
            (Link = 86) => do_link(oldpath: *const i8, newpath: *const i8),
            (Unlink = 87) => do_unlink(path: *const i8),
            (Symlink = 88) => do_symlink(target: *const i8, link_path: *const i8),
            (Readlink = 89) => do_readlink(path: *const i8, buf: *mut u8, size: usize),
            (Chmod = 90) => do_chmod(path: *const i8, mode: u16),
            (Fchmod = 91) => do_fchmod(fd: FileDesc, mode: u16),
            (Chown = 92) => do_chown(path: *const i8, uid: u32, gid: u32),
            (Fchown = 93) => do_fchown(fd: FileDesc, uid: u32, gid: u32),
            (Lchown = 94) => do_lchown(path: *const i8, uid: u32, gid: u32),
            (Umask = 95) => do_umask(mask: u16),
            (Gettimeofday = 96) => do_gettimeofday(tv_u: *mut timeval_t),
            (Getrlimit = 97) => do_gettrlimit(resource: u32, rlim: *mut rlimit_t),
            (Getrusage = 98) => handle_unsupported(),
            (SysInfo = 99) => do_sysinfo(info: *mut sysinfo_t),
            (Times = 100) => handle_unsupported(),
            (Ptrace = 101) => handle_unsupported(),
            (Getuid = 102) => do_getuid(),
            (SysLog = 103) => handle_unsupported(),
            (Getgid = 104) => do_getgid(),
            (Setuid = 105) => handle_unsupported(),
            (Setgid = 106) => handle_unsupported(),
            (Geteuid = 107) => do_geteuid(),
            (Getegid = 108) => do_getegid(),
            (Setpgid = 109) => do_setpgid(pid: i32, pgid: i32),
            (Getppid = 110) => do_getppid(),
            (Getpgrp = 111) => do_getpgrp(),
            (Setsid = 112) => handle_unsupported(),
            (Setreuid = 113) => handle_unsupported(),
            (Setregid = 114) => handle_unsupported(),
            (Getgroups = 115) => do_getgroups(size: isize, buf_ptr: *mut u32),
            (Setgroups = 116) => handle_unsupported(),
            (Setresuid = 117) => handle_unsupported(),
            (Getresuid = 118) => handle_unsupported(),
            (Setresgid = 119) => handle_unsupported(),
            (Getresgid = 120) => handle_unsupported(),
            (Getpgid = 121) => do_getpgid(pid: i32),
            (Setfsuid = 122) => handle_unsupported(),
            (Setfsgid = 123) => handle_unsupported(),
            (Getsid = 124) => handle_unsupported(),
            (Capget = 125) => handle_unsupported(),
            (Capset = 126) => handle_unsupported(),
            (RtSigpending = 127) => do_rt_sigpending(buf_ptr: *mut sigset_t, buf_size: usize),
            (RtSigtimedwait = 128) => do_rt_sigtimedwait(mask_ptr: *const sigset_t, info_ptr: *mut siginfo_t, timeout_ptr: *const timespec_t, mask_size: usize),
            (RtSigqueueinfo = 129) => handle_unsupported(),
            (RtSigsuspend = 130) => handle_unsupported(),
            (Sigaltstack = 131) => do_sigaltstack(ss: *const stack_t, old_ss: *mut stack_t),
            (Utime = 132) => do_utime(path: *const i8, times: *const utimbuf_t),
            (Mknod = 133) => handle_unsupported(),
            (Uselib = 134) => handle_unsupported(),
            (Personality = 135) => handle_unsupported(),
            (Ustat = 136) => handle_unsupported(),
            (Statfs = 137) => do_statfs(path: *const i8, statfs_buf: *mut Statfs),
            (Fstatfs = 138) => do_fstatfs(fd: FileDesc, statfs_buf: *mut Statfs),
            (SysFs = 139) => handle_unsupported(),
            (Getpriority = 140) => do_get_priority(which: i32, who: i32),
            (Setpriority = 141) => do_set_priority(which: i32, who: i32, prio: i32),
            (SchedSetparam = 142) => handle_unsupported(),
            (SchedGetparam = 143) => handle_unsupported(),
            (SchedSetscheduler = 144) => handle_unsupported(),
            (SchedGetscheduler = 145) => handle_unsupported(),
            (SchedGetPriorityMax = 146) => handle_unsupported(),
            (SchedGetPriorityMin = 147) => handle_unsupported(),
            (SchedRrGetInterval = 148) => handle_unsupported(),
            (Mlock = 149) => handle_unsupported(),
            (Munlock = 150) => handle_unsupported(),
            (Mlockall = 151) => handle_unsupported(),
            (Munlockall = 152) => handle_unsupported(),
            (Vhangup = 153) => handle_unsupported(),
            (ModifyLdt = 154) => handle_unsupported(),
            (PivotRoot = 155) => handle_unsupported(),
            (SysCtl = 156) => handle_unsupported(),
            (Prctl = 157) => do_prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64),
            (ArchPrctl = 158) => do_arch_prctl(code: u32, addr: *mut usize),
            (Adjtimex = 159) => handle_unsupported(),
            (Setrlimit = 160) => do_settrlimit(resource: u32, rlim: *const rlimit_t),
            (Chroot = 161) => handle_unsupported(),
            (Sync = 162) => do_sync(),
            (Acct = 163) => handle_unsupported(),
            (Settimeofday = 164) => handle_unsupported(),
            (Mount = 165) => do_mount(source: *const i8, target: *const i8, fs_type: *const i8, flags: u32, options: *const i8),
            (Umount2 = 166) => do_umount(target: *const i8, flags: u32),
            (Swapon = 167) => handle_unsupported(),
            (Swapoff = 168) => handle_unsupported(),
            (Reboot = 169) => handle_unsupported(),
            (Sethostname = 170) => handle_unsupported(),
            (Setdomainname = 171) => handle_unsupported(),
            (Iopl = 172) => handle_unsupported(),
            (Ioperm = 173) => handle_unsupported(),
            (CreateModule = 174) => handle_unsupported(),
            (InitModule = 175) => handle_unsupported(),
            (DeleteModule = 176) => handle_unsupported(),
            (GetKernelSyms = 177) => handle_unsupported(),
            (QueryModule = 178) => handle_unsupported(),
            (Quotactl = 179) => handle_unsupported(),
            (Nfsservctl = 180) => handle_unsupported(),
            (Getpmsg = 181) => handle_unsupported(),
            (Putpmsg = 182) => handle_unsupported(),
            (AfsSysCall = 183) => handle_unsupported(),
            (Tuxcall = 184) => handle_unsupported(),
            (Security = 185) => handle_unsupported(),
            (Gettid = 186) => do_gettid(),
            (Readahead = 187) => handle_unsupported(),
            (Setxattr = 188) => handle_unsupported(),
            (Lsetxattr = 189) => handle_unsupported(),
            (Fsetxattr = 190) => handle_unsupported(),
            (Getxattr = 191) => handle_unsupported(),
            (Lgetxattr = 192) => handle_unsupported(),
            (Fgetxattr = 193) => handle_unsupported(),
            (Listxattr = 194) => handle_unsupported(),
            (Llistxattr = 195) => handle_unsupported(),
            (Flistxattr = 196) => handle_unsupported(),
            (Removexattr = 197) => handle_unsupported(),
            (Lremovexattr = 198) => handle_unsupported(),
            (Fremovexattr = 199) => handle_unsupported(),
            (Tkill = 200) => do_tkill(tid: pid_t, sig: c_int),
            (Time = 201) => do_time(tloc_u: *mut time_t),
            (Futex = 202) => do_futex(futex_addr: *const i32, futex_op: u32, futex_val: i32, timeout: u64, futex_new_addr: *const i32, bitset: u32),
            (SchedSetaffinity = 203) => do_sched_setaffinity(pid: pid_t, cpusize: size_t, buf: *const c_uchar),
            (SchedGetaffinity = 204) => do_sched_getaffinity(pid: pid_t, cpusize: size_t, buf: *mut c_uchar),
            (SetThreadArea = 205) => handle_unsupported(),
            (IoSetup = 206) => handle_unsupported(),
            (IoDestroy = 207) => handle_unsupported(),
            (IoGetevents = 208) => handle_unsupported(),
            (IoSubmit = 209) => handle_unsupported(),
            (IoCancel = 210) => handle_unsupported(),
            (GetThreadArea = 211) => handle_unsupported(),
            (LookupDcookie = 212) => handle_unsupported(),
            (EpollCreate = 213) => do_epoll_create(size: c_int),
            (EpollCtlOld = 214) => handle_unsupported(),
            (EpollWaitOld = 215) => handle_unsupported(),
            (RemapFilePages = 216) => handle_unsupported(),
            (Getdents64 = 217) => do_getdents64(fd: FileDesc, buf: *mut u8, buf_size: usize),
            (SetTidAddress = 218) => do_set_tid_address(tidptr: *mut pid_t),
            (RestartSysCall = 219) => handle_unsupported(),
            (Semtimedop = 220) => handle_unsupported(),
            (Fadvise64 = 221) => handle_unsupported(),
            (TimerCreate = 222) => handle_unsupported(),
            (TimerSettime = 223) => handle_unsupported(),
            (TimerGettime = 224) => handle_unsupported(),
            (TimerGetoverrun = 225) => handle_unsupported(),
            (TimerDelete = 226) => handle_unsupported(),
            (ClockSettime = 227) => handle_unsupported(),
            (ClockGettime = 228) => do_clock_gettime(clockid: clockid_t, ts_u: *mut timespec_t),
            (ClockGetres = 229) => do_clock_getres(clockid: clockid_t, res_u: *mut timespec_t),
            (ClockNanosleep = 230) => do_clock_nanosleep(clockid: clockid_t, flags: i32, request: *const timespec_t, remain: *mut timespec_t),
            (ExitGroup = 231) => do_exit_group(exit_status: i32),
            (EpollWait = 232) => do_epoll_wait(epfd: c_int, events: *mut libc::epoll_event, maxevents: c_int, timeout: c_int),
            (EpollCtl = 233) => do_epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *const libc::epoll_event),
            (Tgkill = 234) => do_tgkill(pid: i32, tid: pid_t, sig: c_int),
            (Utimes = 235) => do_utimes(path: *const i8, times: *const timeval_t),
            (Vserver = 236) => handle_unsupported(),
            (Mbind = 237) => handle_unsupported(),
            (SetMempolicy = 238) => handle_unsupported(),
            (GetMempolicy = 239) => handle_unsupported(),
            (MqOpen = 240) => handle_unsupported(),
            (MqUnlink = 241) => handle_unsupported(),
            (MqTimedsend = 242) => handle_unsupported(),
            (MqTimedreceive = 243) => handle_unsupported(),
            (MqNotify = 244) => handle_unsupported(),
            (MqGetsetattr = 245) => handle_unsupported(),
            (KexecLoad = 246) => handle_unsupported(),
            (Waitid = 247) => handle_unsupported(),
            (AddKey = 248) => handle_unsupported(),
            (RequestKey = 249) => handle_unsupported(),
            (Keyctl = 250) => handle_unsupported(),
            (IoprioSet = 251) => handle_unsupported(),
            (IoprioGet = 252) => handle_unsupported(),
            (InotifyInit = 253) => handle_unsupported(),
            (InotifyAddWatch = 254) => handle_unsupported(),
            (InotifyRmWatch = 255) => handle_unsupported(),
            (MigratePages = 256) => handle_unsupported(),
            (Openat = 257) => do_openat(dirfd: i32, path: *const i8, flags: u32, mode: u16),
            (Mkdirat = 258) => do_mkdirat(dirfd: i32, path: *const i8, mode: u16),
            (Mknodat = 259) => handle_unsupported(),
            (Fchownat = 260) => do_fchownat(dirfd: i32, path: *const i8, uid: u32, gid: u32, flags: i32),
            (Futimesat = 261) => do_futimesat(dirfd: i32, path: *const i8, times: *const timeval_t),
            (Fstatat = 262) => do_fstatat(dirfd: i32, path: *const i8, stat_buf: *mut StatBuf, flags: u32),
            (Unlinkat = 263) => do_unlinkat(dirfd: i32, path: *const i8, flags: i32),
            (Renameat = 264) => do_renameat(olddirfd: i32, oldpath: *const i8, newdirfd: i32, newpath: *const i8),
            (Linkat = 265) => do_linkat(olddirfd: i32, oldpath: *const i8, newdirfd: i32, newpath: *const i8, flags: i32),
            (Symlinkat = 266) => do_symlinkat(target: *const i8, new_dirfd: i32, link_path: *const i8),
            (Readlinkat = 267) => do_readlinkat(dirfd: i32, path: *const i8, buf: *mut u8, size: usize),
            (Fchmodat = 268) => do_fchmodat(dirfd: i32, path: *const i8, mode: u16),
            (Faccessat = 269) => do_faccessat(dirfd: i32, path: *const i8, mode: u32, flags: u32),
            (Pselect6 = 270) => handle_unsupported(),
            (Ppoll = 271) => do_ppoll(fds: *mut libc::pollfd, nfds: libc::nfds_t, timeout_ts: *const timespec_t, sigmask: *const sigset_t),
            (Unshare = 272) => handle_unsupported(),
            (SetRobustList = 273) => do_set_robust_list(list_head_ptr: *mut RobustListHead, len: usize),
            (GetRobustList = 274) => do_get_robust_list(tid: pid_t, list_head_ptr_ptr: *mut *mut RobustListHead, len_ptr: *mut usize),
            (Splice = 275) => handle_unsupported(),
            (Tee = 276) => handle_unsupported(),
            (SyncFileRange = 277) => handle_unsupported(),
            (Vmsplice = 278) => handle_unsupported(),
            (MovePages = 279) => handle_unsupported(),
            (Utimensat = 280) => do_utimensat(dirfd: i32, path: *const i8, times: *const timespec_t, flags: i32),
            (EpollPwait = 281) => do_epoll_pwait(epfd: c_int, events: *mut libc::epoll_event, maxevents: c_int, timeout: c_int, sigmask: *const usize),
            (Signalfd = 282) => handle_unsupported(),
            (TimerfdCreate = 283) => do_timerfd_create(clockid: clockid_t, flags: i32 ),
            (Eventfd = 284) => do_eventfd(init_val: u32),
            (Fallocate = 285) => do_fallocate(fd: FileDesc, mode: u32, offset: off_t, len: off_t),
            (TimerfdSettime = 286) => do_timerfd_settime(fd: FileDesc, flags: i32, new_value: *const itimerspec_t, old_value: *mut itimerspec_t),
            (TimerfdGettime = 287) => do_timerfd_gettime(fd: FileDesc, curr_value: *mut itimerspec_t),
            (Accept4 = 288) => do_accept4(fd: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t, flags: c_int),
            (Signalfd4 = 289) => handle_unsupported(),
            (Eventfd2 = 290) => do_eventfd2(init_val: u32, flags: i32),
            (EpollCreate1 = 291) => do_epoll_create1(flags: c_int),
            (Dup3 = 292) => do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32),
            (Pipe2 = 293) => do_pipe2(fds_u: *mut i32, flags: u32),
            (InotifyInit1 = 294) => handle_unsupported(),
            (Preadv = 295) => do_preadv(fd: FileDesc, iov: *mut iovec_t, count: i32, offset: off_t),
            (Pwritev = 296) => do_pwritev(fd: FileDesc, iov: *const iovec_t, count: i32, offset: off_t),
            (RtTgsigqueueinfo = 297) => handle_unsupported(),
            (PerfEventOpen = 298) => handle_unsupported(),
            (Recvmmsg = 299) => handle_unsupported(),
            (FanotifyInit = 300) => handle_unsupported(),
            (FanotifyMark = 301) => handle_unsupported(),
            (Prlimit64 = 302) => do_prlimit(pid: pid_t, resource: u32, new_limit: *const rlimit_t, old_limit: *mut rlimit_t),
            (NameToHandleAt = 303) => handle_unsupported(),
            (OpenByHandleAt = 304) => handle_unsupported(),
            (ClockAdjtime = 305) => handle_unsupported(),
            (Syncfs = 306) => handle_unsupported(),
            (Sendmmsg = 307) => do_sendmmsg(fd: c_int, msg_ptr: *mut mmsghdr, vlen: c_uint, flags_c: c_int),
            (Setns = 308) => handle_unsupported(),
            (Getcpu = 309) => do_getcpu(cpu_ptr: *mut u32, node_ptr: *mut u32),
            (ProcessVmReadv = 310) => handle_unsupported(),
            (ProcessVmWritev = 311) => handle_unsupported(),
            (Kcmp = 312) => handle_unsupported(),
            (FinitModule = 313) => handle_unsupported(),
            (SchedSetattr = 314) => handle_unsupported(),
            (SchedGetattr = 315) => handle_unsupported(),
            (Renameat2 = 316) => handle_unsupported(),
            (Seccomp = 317) => handle_unsupported(),
            (Getrandom = 318) => do_getrandom(buf: *mut u8, len: size_t, flags: u32),
            (MemfdCreate = 319) => handle_unsupported(),
            (KexecFileLoad = 320) => handle_unsupported(),
            (Bpf = 321) => handle_unsupported(),
            (Execveat = 322) => handle_unsupported(),
            (Userfaultfd = 323) => handle_unsupported(),
            (Membarrier = 324) => handle_unsupported(),
            (Mlock2 = 325) => handle_unsupported(),

            // Occlum-specific system calls
            (SpawnGlibc = 359) => do_spawn_for_glibc(child_pid_ptr: *mut u32, path: *const i8, argv: *const *const i8, envp: *const *const i8, fa: *const SpawnFileActions, attribute_list: *const posix_spawnattr_t),
            (SpawnMusl = 360) => do_spawn_for_musl(child_pid_ptr: *mut u32, path: *const i8, argv: *const *const i8, envp: *const *const i8, fdop_list: *const FdOp, attribute_list: *const posix_spawnattr_t),
            (MountRootFS = 363) => do_mount_rootfs(key_ptr: *const sgx_key_128bit_t, rootfs_config_ptr: *const user_rootfs_config),
        }
    };
}

/// System call numbers.
///
/// The enum is implemented with macros, which expands into the code looks like below:
/// ```
/// pub enum SyscallNum {
///     Read = 0,
///     Write = 1,
///     // ...
/// }
/// ```
/// The system call nubmers are named in a way consistent with libc.
macro_rules! impl_syscall_nums {
    ($( ( $name: ident = $num: expr ) => $_impl_fn: ident ( $($arg_name:tt : $arg_type: ty),* ) ),+,) => {
        #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
        #[repr(u32)]
        pub enum SyscallNum {
            $(
                $name = $num,
            )*
        }

        impl SyscallNum {
            pub fn as_str(&self) -> &'static str {
                use SyscallNum::*;
                match *self {
                    #![deny(unreachable_patterns)]
                    $(
                        $name => stringify!($name),
                    )*
                }
            }
        }

        impl TryFrom<u32> for SyscallNum {
            type Error = errno::Error;

            fn try_from(raw_num: u32) -> Result<Self> {
                match raw_num {
                    $(
                        $num => Ok(Self::$name),
                    )*
                    _ => return_errno!(SyscallNumError::new(raw_num)),
                }
            }
        }

        #[derive(Copy, Clone, Debug)]
        pub struct SyscallNumError {
            invalid_num: u32,
        }

        impl SyscallNumError {
            pub fn new(invalid_num: u32) -> Self {
                Self { invalid_num }
            }
        }

        impl ToErrno for SyscallNumError {
            fn errno(&self) -> Errno {
                ENOSYS
            }
        }

        impl std::fmt::Display for SyscallNumError {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "Invalid system call number ({})", self.invalid_num)
            }
        }
    }
}
/// Generate system call numbers.
process_syscall_table_with_callback!(impl_syscall_nums);

/// A struct that represents a system call
struct Syscall {
    num: SyscallNum,
    args: [isize; 6],
}

impl Syscall {
    pub fn new(
        num: u32,
        arg0: isize,
        arg1: isize,
        arg2: isize,
        arg3: isize,
        arg4: isize,
        arg5: isize,
    ) -> Result<Self> {
        let num = SyscallNum::try_from(num)?;
        let args = [arg0, arg1, arg2, arg3, arg4, arg5];
        Ok(Self { num, args })
    }
}

/// Generate the code that can format any system call.
macro_rules! impl_fmt_syscall {
    // Internal rules
    (@fmt_args $self_:ident, $f:ident, $arg_i:expr, ($(,)?)) => {};
    (@fmt_args $self_:ident, $f:ident, $arg_i:expr, ($arg_name:ident : $arg_type:ty, $($more_args:tt)*)) => {
        let arg_val = $self_.args[$arg_i] as $arg_type;
        write!($f, ", {} = {:?}", stringify!($arg_name), arg_val)?;
        impl_fmt_syscall!(@fmt_args $self_, $f, ($arg_i + 1), ($($more_args)*));
    };

    // Main rule
    ($( ( $name:ident = $num:expr ) => $fn:ident ( $($args:tt)* ) ),+,) => {
        impl std::fmt::Debug for Syscall {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Syscall {{ num = {:?}", self.num)?;
                match self.num {
                    #![deny(unreachable_patterns)]
                    $(
                        // Expands into something like below:
                        //
                        // SyscallNum::Read => {
                        //     let arg_val = self.args[0] as FileDesc;
                        //     write!(f, ", {} = {:?}", "fd", arg_val);
                        //     let arg_val = self.args[1] as *mut u8;
                        //     write!(f, ", {} = {:?}", "buf", arg_val);
                        //     let arg_val = self.args[2] as usize;
                        //     write!(f, ", {} = {:?}", "size", arg_val);
                        // }
                        SyscallNum::$name => {
                            impl_fmt_syscall!(@fmt_args self, f, 0, ($($args)*,));
                        },
                    )*
                };
                write!(f, " }}")
            }
        }
    }
}
process_syscall_table_with_callback!(impl_fmt_syscall);

/// Generate the code that can dispatch any system call to its actual implementation function.
macro_rules! impl_dispatch_syscall {
    (@do_syscall $fn:ident, $syscall:ident, $arg_i:expr, ($(,)?) -> ($($output:tt)*) ) => {
        impl_dispatch_syscall!(@as_expr $fn($($output)*))
    };
    (@do_syscall $fn:ident, $syscall:ident, $arg_i:expr, ($_arg_name:ident : $arg_type:ty, $($more_args:tt)*) -> ($($output:tt)*)) => {
        impl_dispatch_syscall!(@do_syscall $fn, $syscall, ($arg_i + 1), ($($more_args)*) -> ($($output)* ($syscall.args[$arg_i] as $arg_type),))
    };
    (@as_expr $e:expr) => { $e.await };

    ($( ( $name:ident = $num:expr ) => $fn:ident ( $($args:tt)* ) ),+,) => {
        async fn dispatch_syscall(syscall: Syscall) -> Result<isize> {
            match syscall.num {
                #![deny(unreachable_patterns)]
                $(
                    // Expands into something like below:
                    //
                    // SyscallNum::Read => {
                    //     let fd = self.args[0] as FileDesc;
                    //     let buf = self.args[1] as *mut u8;
                    //     let size = self.args[2] as usize;
                    //     do_read(fd, buf, size)
                    // }
                    SyscallNum::$name => {
                        impl_dispatch_syscall!(@do_syscall $fn, syscall, 0, ($($args)*,) -> ())
                    },
                )*
            }
        }
    }
}
process_syscall_table_with_callback!(impl_dispatch_syscall);

// TODO: move the following syscall redirction code to proper subsystems.

/*
 * This Rust-version of fdop correspond to the C-version one in Occlum.
 * See <path_to_musl_libc>/src/process/fdop.h.
 */
const FDOP_CLOSE: u32 = 1;
const FDOP_DUP2: u32 = 2;
const FDOP_OPEN: u32 = 3;

async fn do_mmap(
    addr: usize,
    size: usize,
    perms: i32,
    flags: i32,
    fd: FileDesc,
    offset: off_t,
) -> Result<isize> {
    let perms = VMPerms::from_u32(perms as u32)?;
    let flags = MMapFlags::from_u32(flags as u32)?;
    let addr = vm::do_mmap(addr, size, perms, flags, fd, offset as usize).await?;
    Ok(addr as isize)
}

async fn do_munmap(addr: usize, size: usize) -> Result<isize> {
    vm::do_munmap(addr, size).await?;
    Ok(0)
}

async fn do_mremap(
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: i32,
    new_addr: usize,
) -> Result<isize> {
    let flags = MRemapFlags::from_raw(flags as u32, new_addr)?;
    let addr = vm::do_mremap(old_addr, old_size, new_size, flags).await?;
    Ok(addr as isize)
}

async fn do_mprotect(addr: usize, len: usize, perms: u32) -> Result<isize> {
    let perms = VMPerms::from_u32(perms as u32)?;
    vm::do_mprotect(addr, len, perms).await?;
    Ok(0)
}

async fn do_brk(new_brk_addr: usize) -> Result<isize> {
    let ret_brk_addr = vm::do_brk(new_brk_addr).await?;
    Ok(ret_brk_addr as isize)
}

async fn do_msync(addr: usize, size: usize, flags: u32) -> Result<isize> {
    let flags = MSyncFlags::from_u32(flags)?;
    vm::do_msync(addr, size, flags).await?;
    Ok(0)
}

async fn do_sysinfo(info: *mut sysinfo_t) -> Result<isize> {
    check_mut_ptr(info)?;
    let info = unsafe { &mut *info };
    *info = crate::misc::do_sysinfo().await?;
    Ok(0)
}

async fn do_getrandom(buf: *mut u8, len: size_t, flags: u32) -> Result<isize> {
    check_mut_array(buf, len)?;
    let checked_len = if len > u32::MAX as usize {
        u32::MAX as usize
    } else {
        len
    };
    let rand_buf = unsafe { std::slice::from_raw_parts_mut(buf, checked_len) };
    let flags = RandFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;

    crate::misc::do_getrandom(rand_buf, flags)?;
    Ok(checked_len as isize)
}

// TODO: handle tz: timezone_t
async fn do_gettimeofday(tv_u: *mut timeval_t) -> Result<isize> {
    check_mut_ptr(tv_u)?;
    let tv = crate::time::do_gettimeofday();
    unsafe {
        *tv_u = tv;
    }
    Ok(0)
}

async fn do_clock_gettime(clockid: clockid_t, ts_u: *mut timespec_t) -> Result<isize> {
    check_mut_ptr(ts_u)?;
    let clockid = ClockId::try_from(clockid)?;
    let ts = crate::time::do_clock_gettime(clockid)?;
    unsafe {
        *ts_u = ts;
    }
    Ok(0)
}

async fn do_clock_getres(clockid: clockid_t, res_u: *mut timespec_t) -> Result<isize> {
    if res_u.is_null() {
        return Ok(0);
    }
    check_mut_ptr(res_u)?;
    let clockid = ClockId::try_from(clockid)?;
    let res = crate::time::do_clock_getres(clockid)?;
    unsafe {
        *res_u = res;
    }
    Ok(0)
}

async fn do_time(tloc_u: *mut time_t) -> Result<isize> {
    let ts = crate::time::do_clock_gettime(crate::time::ClockId::CLOCK_REALTIME)?;
    if !tloc_u.is_null() {
        check_mut_ptr(tloc_u)?;
        unsafe {
            *tloc_u = ts.sec();
        }
    }
    Ok(ts.sec() as isize)
}

// TODO: handle remainder
async fn do_nanosleep(req_u: *const timespec_t, rem_u: *mut timespec_t) -> Result<isize> {
    let req = {
        check_ptr(req_u)?;
        timespec_t::from_raw_ptr(req_u)?
    };
    let rem = if !rem_u.is_null() {
        check_mut_ptr(rem_u)?;
        Some(unsafe { &mut *rem_u })
    } else {
        None
    };
    crate::time::do_nanosleep(&req, rem).await
}

async fn do_clock_nanosleep(
    clockid: clockid_t,
    flags: i32,
    req_u: *const timespec_t,
    rem_u: *mut timespec_t,
) -> Result<isize> {
    let req = {
        check_ptr(req_u)?;
        timespec_t::from_raw_ptr(req_u)?
    };
    let rem = if !rem_u.is_null() {
        check_mut_ptr(rem_u)?;
        Some(unsafe { &mut *rem_u })
    } else {
        None
    };
    let clockid = ClockId::try_from(clockid)?;
    crate::time::do_clock_nanosleep(clockid, flags, &req, rem).await
}

async fn do_uname(name: *mut utsname_t) -> Result<isize> {
    check_mut_ptr(name)?;
    let name = unsafe { &mut *name };
    crate::misc::do_uname(name).map(|_| 0)
}

async fn do_gettrlimit(resource: u32, rlim: *mut rlimit_t) -> Result<isize> {
    do_prlimit(0, resource, ptr::null(), rlim).await
}

async fn do_settrlimit(resource: u32, rlim: *const rlimit_t) -> Result<isize> {
    do_prlimit(0, resource, rlim, ptr::null_mut()).await
}

async fn do_prlimit(
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
    crate::misc::do_prlimit(pid, resource, new_limit, old_limit).map(|_| 0)
}

async fn handle_unsupported() -> Result<isize> {
    return_errno!(ENOSYS, "Unimplemented or unknown syscall")
}

//! System call handler
//!
//! # Syscall processing flow
//!
//! 1. Libc calls `__occlum_syscall` (in `syscall_entry_x86_64.S`)
//! 2. Do user/LibOS switch and then call `occlum_syscall` (in this file)
//! 3. Preprocess the system call and then call `dispatch_syscall` (in this file)
//! 4. Call `do_*` to process the system call (in other modules)

use fs::{
    do_access, do_chdir, do_chmod, do_chown, do_close, do_dup, do_dup2, do_dup3, do_eventfd,
    do_eventfd2, do_faccessat, do_fchmod, do_fchown, do_fcntl, do_fdatasync, do_fstat, do_fstatat,
    do_fsync, do_ftruncate, do_getdents64, do_ioctl, do_lchown, do_link, do_lseek, do_lstat,
    do_mkdir, do_open, do_openat, do_pipe, do_pipe2, do_pread, do_pwrite, do_read, do_readlink,
    do_readv, do_rename, do_rmdir, do_sendfile, do_stat, do_sync, do_truncate, do_unlink, do_write,
    do_writev, iovec_t, File, FileDesc, FileRef, HostStdioFds, Stat,
};
use misc::{resource_t, rlimit_t, utsname_t};
use net::{
    do_epoll_create, do_epoll_create1, do_epoll_ctl, do_epoll_pwait, do_epoll_wait, do_poll,
    do_recvmsg, do_select, do_sendmsg, msghdr, msghdr_mut, AsSocket, AsUnixSocket, EpollEvent,
    SocketFile, UnixSocketFile,
};
use process::{pid_t, ChildProcessFilter, CloneFlags, CpuSet, FileAction, FutexFlags, FutexOp};
use std::any::Any;
use std::convert::TryFrom;
use std::ffi::{CStr, CString};
use std::io::{Read, Seek, SeekFrom, Write};
use std::ptr;
use time::{clockid_t, timespec_t, timeval_t, GLOBAL_PROFILER};
use util::log::{self, LevelFilter};
use util::mem_util::from_user::*;
use vm::{MMapFlags, VMPerms};
use {fs, process, std, vm};

use super::*;

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
                (Open = 2) => do_open(path: *const i8, flags: u32, mode: u32),
                (Close = 3) => do_close(fd: FileDesc),
                (Stat = 4) => do_stat(path: *const i8, stat_buf: *mut Stat),
                (Fstat = 5) => do_fstat(fd: FileDesc, stat_buf: *mut Stat),
                (Lstat = 6) => do_lstat(path: *const i8, stat_buf: *mut Stat),
                (Poll = 7) => do_poll(fds: *mut libc::pollfd, nfds: libc::nfds_t, timeout: c_int),
                (Lseek = 8) => do_lseek(fd: FileDesc, offset: off_t, whence: i32),
                (Mmap = 9) => do_mmap(addr: usize, size: usize, perms: i32, flags: i32, fd: FileDesc, offset: off_t),
                (Mprotect = 10) => do_mprotect(addr: usize, len: usize, prot: u32),
                (Munmap = 11) => do_munmap(addr: usize, size: usize),
                (Brk = 12) => do_brk(new_brk_addr: usize),
                (RtSigaction = 13) => do_rt_sigaction(),
                (RtSigprocmask = 14) => do_rt_sigprocmask(),
                (RtSigreturn = 15) => handle_unsupported(),
                (Ioctl = 16) => do_ioctl(fd: FileDesc, cmd: u32, argp: *mut u8),
                (Pread64 = 17) => do_pread(fd: FileDesc, buf: *mut u8, size: usize, offset: usize),
                (Pwrite64 = 18) => do_pwrite(fd: FileDesc, buf: *const u8, size: usize, offset: usize),
                (Readv = 19) => do_readv(fd: FileDesc, iov: *mut iovec_t, count: i32),
                (Writev = 20) => do_writev(fd: FileDesc, iov: *const iovec_t, count: i32),
                (Access = 21) => do_access(path: *const i8, mode: u32),
                (Pipe = 22) => do_pipe(fds_u: *mut i32),
                (Select = 23) => do_select(nfds: c_int, readfds: *mut libc::fd_set, writefds: *mut libc::fd_set, exceptfds: *mut libc::fd_set, timeout: *const libc::timeval),
                (SchedYield = 24) => do_sched_yield(),
                (Mremap = 25) => do_mremap(old_addr: usize, old_size: usize, new_size: usize, flags: i32, new_addr: usize),
                (Msync = 26) => handle_unsupported(),
                (Mincore = 27) => handle_unsupported(),
                (Madvise = 28) => handle_unsupported(),
                (Shmget = 29) => handle_unsupported(),
                (Shmat = 30) => handle_unsupported(),
                (Shmctl = 31) => handle_unsupported(),
                (Dup = 32) => do_dup(old_fd: FileDesc),
                (Dup2 = 33) => do_dup2(old_fd: FileDesc, new_fd: FileDesc),
                (Pause = 34) => handle_unsupported(),
                (Nanosleep = 35) => do_nanosleep(req_u: *const timespec_t, rem_u: *mut timespec_t),
                (Getitimer = 36) => handle_unsupported(),
                (Alarm = 37) => handle_unsupported(),
                (Setitimer = 38) => handle_unsupported(),
                (Getpid = 39) => do_getpid(),
                (Sendfile = 40) => do_sendfile(out_fd: FileDesc, in_fd: FileDesc, offset_ptr: *mut off_t, count: usize),
                (Socket = 41) => do_socket(domain: c_int, socket_type: c_int, protocol: c_int),
                (Connect = 42) => do_connect(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t),
                (Accept = 43) => do_accept(fd: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t),
                (Sendto = 44) => do_sendto(fd: c_int, base: *const c_void, len: size_t, flags: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t),
                (Recvfrom = 45) => do_recvfrom(fd: c_int, base: *mut c_void, len: size_t, flags: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t),
                (Sendmsg = 46) => do_sendmsg(fd: c_int, msg_ptr: *const msghdr, flags_c: c_int),
                (Recvmsg = 47) => do_recvmsg(fd: c_int, msg_mut_ptr: *mut msghdr_mut, flags_c: c_int),
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
                (Vfork = 58) => handle_unsupported(),
                (Execve = 59) => handle_unsupported(),
                (Exit = 60) => do_exit(exit_status: i32),
                (Wait4 = 61) => do_wait4(pid: i32, _exit_status: *mut i32),
                (Kill = 62) => handle_unsupported(),
                (Uname = 63) => do_uname(name: *mut utsname_t),
                (Semget = 64) => handle_unsupported(),
                (Semop = 65) => handle_unsupported(),
                (Semctl = 66) => handle_unsupported(),
                (Shmdt = 67) => handle_unsupported(),
                (Msgget = 68) => handle_unsupported(),
                (Msgsnd = 69) => handle_unsupported(),
                (Msgrcv = 70) => handle_unsupported(),
                (Msgctl = 71) => handle_unsupported(),
                (Fcntl = 72) => do_fcntl(fd: FileDesc, cmd: u32, arg: u64),
                (Flock = 73) => handle_unsupported(),
                (Fsync = 74) => do_fsync(fd: FileDesc),
                (Fdatasync = 75) => do_fdatasync(fd: FileDesc),
                (Truncate = 76) => do_truncate(path: *const i8, len: usize),
                (Ftruncate = 77) => do_ftruncate(fd: FileDesc, len: usize),
                (Getdents = 78) => handle_unsupported(),
                (Getcwd = 79) => do_getcwd(buf: *mut u8, size: usize),
                (Chdir = 80) => do_chdir(path: *const i8),
                (Fchdir = 81) => handle_unsupported(),
                (Rename = 82) => do_rename(oldpath: *const i8, newpath: *const i8),
                (Mkdir = 83) => do_mkdir(path: *const i8, mode: usize),
                (Rmdir = 84) => do_rmdir(path: *const i8),
                (Creat = 85) => handle_unsupported(),
                (Link = 86) => do_link(oldpath: *const i8, newpath: *const i8),
                (Unlink = 87) => do_unlink(path: *const i8),
                (Symlink = 88) => handle_unsupported(),
                (Readlink = 89) => do_readlink(path: *const i8, buf: *mut u8, size: usize),
                (Chmod = 90) => do_chmod(path: *const i8, mode: u16),
                (Fchmod = 91) => do_fchmod(fd: FileDesc, mode: u16),
                (Chown = 92) => do_chown(path: *const i8, uid: u32, gid: u32),
                (Fchown = 93) => do_fchown(fd: FileDesc, uid: u32, gid: u32),
                (Lchown = 94) => do_lchown(path: *const i8, uid: u32, gid: u32),
                (Umask = 95) => handle_unsupported(),
                (Gettimeofday = 96) => do_gettimeofday(tv_u: *mut timeval_t),
                (Getrlimit = 97) => handle_unsupported(),
                (Getrusage = 98) => handle_unsupported(),
                (SysInfo = 99) => handle_unsupported(),
                (Times = 100) => handle_unsupported(),
                (Ptrace = 101) => handle_unsupported(),
                (Getuid = 102) => do_getuid(),
                (SysLog = 103) => handle_unsupported(),
                (Getgid = 104) => do_getgid(),
                (Setuid = 105) => handle_unsupported(),
                (Setgid = 106) => handle_unsupported(),
                (Geteuid = 107) => do_geteuid(),
                (Getegid = 108) => do_getegid(),
                (Setpgid = 109) => handle_unsupported(),
                (Getppid = 110) => do_getppid(),
                (Getpgrp = 111) => handle_unsupported(),
                (Setsid = 112) => handle_unsupported(),
                (Setreuid = 113) => handle_unsupported(),
                (Setregid = 114) => handle_unsupported(),
                (Getgroups = 115) => handle_unsupported(),
                (Setgroups = 116) => handle_unsupported(),
                (Setresuid = 117) => handle_unsupported(),
                (Getresuid = 118) => handle_unsupported(),
                (Setresgid = 119) => handle_unsupported(),
                (Getresgid = 120) => handle_unsupported(),
                (Getpgid = 121) => do_getpgid(),
                (Setfsuid = 122) => handle_unsupported(),
                (Setfsgid = 123) => handle_unsupported(),
                (Getsid = 124) => handle_unsupported(),
                (Capget = 125) => handle_unsupported(),
                (Capset = 126) => handle_unsupported(),
                (RtSigpending = 127) => handle_unsupported(),
                (RtSigtimedwait = 128) => handle_unsupported(),
                (RtSigqueueinfo = 129) => handle_unsupported(),
                (RtSigsuspend = 130) => handle_unsupported(),
                (Sigaltstack = 131) => handle_unsupported(),
                (Utime = 132) => handle_unsupported(),
                (Mknod = 133) => handle_unsupported(),
                (Uselib = 134) => handle_unsupported(),
                (Personality = 135) => handle_unsupported(),
                (Ustat = 136) => handle_unsupported(),
                (Statfs = 137) => handle_unsupported(),
                (Fstatfs = 138) => handle_unsupported(),
                (SysFs = 139) => handle_unsupported(),
                (Getpriority = 140) => handle_unsupported(),
                (Setpriority = 141) => handle_unsupported(),
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
                (Prctl = 157) => handle_unsupported(),
                (ArchPrctl = 158) => do_arch_prctl(code: u32, addr: *mut usize),
                (Adjtimex = 159) => handle_unsupported(),
                (Setrlimit = 160) => handle_unsupported(),
                (Chroot = 161) => handle_unsupported(),
                (Sync = 162) => do_sync(),
                (Acct = 163) => handle_unsupported(),
                (Settimeofday = 164) => handle_unsupported(),
                (Mount = 165) => handle_unsupported(),
                (Umount2 = 166) => handle_unsupported(),
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
                (Tkill = 200) => handle_unsupported(),
                (Time = 201) => handle_unsupported(),
                (Futex = 202) => do_futex(futex_addr: *const i32, futex_op: u32, futex_val: i32, timeout: u64, futex_new_addr: *const i32),
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
                (ClockGetres = 229) => handle_unsupported(),
                (ClockNanosleep = 230) => handle_unsupported(),
                (ExitGroup = 231) => handle_unsupported(),
                (EpollWait = 232) => do_epoll_wait(epfd: c_int, events: *mut libc::epoll_event, maxevents: c_int, timeout: c_int),
                (EpollCtl = 233) => do_epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *const libc::epoll_event),
                (Tgkill = 234) => handle_unsupported(),
                (Utimes = 235) => handle_unsupported(),
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
                (Openat = 257) => do_openat(dirfd: i32, path: *const i8, flags: u32, mode: u32),
                (Mkdirat = 258) => handle_unsupported(),
                (Mknodat = 259) => handle_unsupported(),
                (Fchownat = 260) => handle_unsupported(),
                (Futimesat = 261) => handle_unsupported(),
                (Fstatat = 262) => do_fstatat(dirfd: i32, path: *const i8, stat_buf: *mut Stat, flags: u32),
                (Unlinkat = 263) => handle_unsupported(),
                (Renameat = 264) => handle_unsupported(),
                (Linkat = 265) => handle_unsupported(),
                (Symlinkat = 266) => handle_unsupported(),
                (Readlinkat = 267) => handle_unsupported(),
                (Fchmodat = 268) => handle_unsupported(),
                (Faccessat = 269) => do_faccessat(dirfd: i32, path: *const i8, mode: u32, flags: u32),
                (Pselect6 = 270) => handle_unsupported(),
                (Ppoll = 271) => handle_unsupported(),
                (Unshare = 272) => handle_unsupported(),
                (SetRobustList = 273) => handle_unsupported(),
                (GetRobustList = 274) => handle_unsupported(),
                (Splice = 275) => handle_unsupported(),
                (Tee = 276) => handle_unsupported(),
                (SyncFileRange = 277) => handle_unsupported(),
                (Vmsplice = 278) => handle_unsupported(),
                (MovePages = 279) => handle_unsupported(),
                (Utimensat = 280) => handle_unsupported(),
                (EpollPwait = 281) => do_epoll_pwait(epfd: c_int, events: *mut libc::epoll_event, maxevents: c_int, timeout: c_int, sigmask: *const usize),
                (Signalfd = 282) => handle_unsupported(),
                (TimerfdCreate = 283) => handle_unsupported(),
                (Eventfd = 284) => do_eventfd(init_val: u32),
                (Fallocate = 285) => handle_unsupported(),
                (TimerfdSettime = 286) => handle_unsupported(),
                (TimerfdGettime = 287) => handle_unsupported(),
                (Accept4 = 288) => do_accept4(fd: c_int, addr: *mut libc::sockaddr, addr_len: *mut libc::socklen_t, flags: c_int),
                (Signalfd4 = 289) => handle_unsupported(),
                (Eventfd2 = 290) => do_eventfd2(init_val: u32, flaggs: i32),
                (EpollCreate1 = 291) => do_epoll_create1(flags: c_int),
                (Dup3 = 292) => do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32),
                (Pipe2 = 293) => do_pipe2(fds_u: *mut i32, flags: u32),
                (InotifyInit1 = 294) => handle_unsupported(),
                (Preadv = 295) => handle_unsupported(),
                (Pwritev = 296) => handle_unsupported(),
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
                (Sendmmsg = 307) => handle_unsupported(),
                (Setns = 308) => handle_unsupported(),
                (Getcpu = 309) => handle_unsupported(),
                (ProcessVmReadv = 310) => handle_unsupported(),
                (ProcessVmWritev = 311) => handle_unsupported(),
                (Kcmp = 312) => handle_unsupported(),
                (FinitModule = 313) => handle_unsupported(),
                (SchedSetattr = 314) => handle_unsupported(),
                (SchedGetattr = 315) => handle_unsupported(),
                (Renameat2 = 316) => handle_unsupported(),
                (Seccomp = 317) => handle_unsupported(),
                (Getrandom = 318) => handle_unsupported(),
                (MemfdCreate = 319) => handle_unsupported(),
                (KexecFileLoad = 320) => handle_unsupported(),
                (Bpf = 321) => handle_unsupported(),
                (Execveat = 322) => handle_unsupported(),
                (Userfaultfd = 323) => handle_unsupported(),
                (Membarrier = 324) => handle_unsupported(),
                (Mlock2 = 325) => handle_unsupported(),

                // Occlum-specific sytem calls
                (Spawn = 360) => do_spawn(child_pid_ptr: *mut u32, path: *const i8, argv: *const *const i8, envp: *const *const i8, fdop_list: *const FdOp),
                // Exception handling
                (Rdtsc = 361) => do_rdtsc(low_ptr: *mut u32, high_ptr: *mut u32),
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
            type Error = error::Error;

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
                EINVAL
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
        impl_dispatch_syscall!(@as_expr $fn($($output)*));
    };
    (@do_syscall $fn:ident, $syscall:ident, $arg_i:expr, ($_arg_name:ident : $arg_type:ty, $($more_args:tt)*) -> ($($output:tt)*)) => {
        impl_dispatch_syscall!(@do_syscall $fn, $syscall, ($arg_i + 1), ($($more_args)*) -> ($($output)* ($syscall.args[$arg_i] as $arg_type),));
    };
    (@as_expr $e:expr) => { $e };

    ($( ( $name:ident = $num:expr ) => $fn:ident ( $($args:tt)* ) ),+,) => {
        fn dispatch_syscall(syscall: Syscall) -> Result<isize> {
            match syscall.num {
                #![deny(unreachable_patterns)]
                $(
                    // Expands into something like below:
                    //
                    // SyscallNum::Read => {
                    //     let fd = self.args[0] as FileDesc;
                    //     let buf = self.args[1] as *mut u8;
                    //     let size = self.args[2] as usize;
                    //     do_read(fd, buuf, size)
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

/// The system call entry point in Rust.
///
/// This function is called by __occlum_syscall.
#[no_mangle]
pub extern "C" fn occlum_syscall(
    num: u32,
    arg0: isize,
    arg1: isize,
    arg2: isize,
    arg3: isize,
    arg4: isize,
    arg5: isize,
) -> isize {
    // Start a new round of log messages for this system call. But we do not
    // set the description of this round, yet. We will do so after checking the
    // given system call number is a valid.
    log::next_round(None);

    #[cfg(feature = "syscall_timing")]
    GLOBAL_PROFILER
        .lock()
        .unwrap()
        .syscall_enter(syscall_num)
        .expect("unexpected error from profiler to enter syscall");

    let ret = Syscall::new(num, arg0, arg1, arg2, arg3, arg4, arg5).and_then(|syscall| {
        log::set_round_desc(Some(syscall.num.as_str()));
        trace!("{:?}", &syscall);

        dispatch_syscall(syscall)
    });

    #[cfg(feature = "syscall_timing")]
    GLOBAL_PROFILER
        .lock()
        .unwrap()
        .syscall_exit(syscall_num, ret.is_err())
        .expect("unexpected error from profiler to exit syscall");

    let retval = match ret {
        Ok(retval) => retval as isize,
        Err(e) => {
            let should_log_err = |errno| {
                // If the log level requires every detail, don't ignore any error
                if log::max_level() == LevelFilter::Trace {
                    return true;
                }

                // All other log levels require errors to be outputed. But
                // some errnos are usually benign and may occur in a very high
                // frequency. So we want to ignore them to keep noises at a
                // minimum level in the log.
                //
                // TODO: use a smarter, frequency-based strategy to decide whether
                // to suppress error messages.
                match errno {
                    EAGAIN | ETIMEDOUT => false,
                    _ => true,
                }
            };
            if should_log_err(e.errno()) {
                error!("Error = {}", e.backtrace());
            }

            let retval = -(e.errno() as isize);
            debug_assert!(retval != 0);
            retval
        }
    };
    trace!("Retval = {:?}", retval);
    retval
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
    debug!(
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
    timeout: u64,
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
        FutexOp::FUTEX_WAIT => {
            let timeout = {
                let timeout = timeout as *const timespec_t;
                if timeout.is_null() {
                    None
                } else {
                    let ts = timespec_t::from_raw_ptr(timeout)?;
                    ts.validate()?;
                    if futex_flags.contains(FutexFlags::FUTEX_CLOCK_REALTIME) {
                        warn!("CLOCK_REALTIME is not supported yet, use monotonic clock");
                    }
                    Some(ts)
                }
            };
            process::futex_wait(futex_addr, futex_val, &timeout).map(|_| 0)
        }
        FutexOp::FUTEX_WAKE => {
            let max_count = get_futex_val(futex_val)?;
            process::futex_wake(futex_addr, max_count).map(|count| count as isize)
        }
        FutexOp::FUTEX_REQUEUE => {
            check_ptr(futex_new_addr)?;
            let max_nwakes = get_futex_val(futex_val)?;
            let max_nrequeues = get_futex_val(timeout as i32)?;
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

fn do_rdtsc(low_ptr: *mut u32, high_ptr: *mut u32) -> Result<isize> {
    check_mut_ptr(low_ptr)?;
    check_mut_ptr(high_ptr)?;
    let (low, high) = time::do_rdtsc()?;
    debug!("do_rdtsc result {{ low: {:#x} high: {:#x}}}", low, high);
    unsafe {
        *low_ptr = low;
        *high_ptr = high;
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
    debug!("exit: {}", status);
    extern "C" {
        fn do_exit_task() -> !;
    }
    process::do_exit(status);
    unsafe {
        do_exit_task();
    }
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
    debug!(
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

    let fd = process::put_file(file_ref, false)?;
    Ok(fd as isize)
}

fn do_connect(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t) -> Result<isize> {
    debug!(
        "connect: fd: {}, addr: {:?}, addr_len: {}",
        fd, addr, addr_len
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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

fn do_accept(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    do_accept4(fd, addr, addr_len, 0)
}

fn do_accept4(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
    flags: c_int,
) -> Result<isize> {
    debug!(
        "accept4: fd: {}, addr: {:?}, addr_len: {:?}, flags: {:#x}",
        fd, addr, addr_len, flags
    );
    let file_ref = process::get_file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let socket = file_ref.as_socket()?;

        let new_socket = socket.accept(addr, addr_len, flags)?;
        let new_file_ref: Arc<Box<dyn File>> = Arc::new(Box::new(new_socket));
        let new_fd = process::put_file(new_file_ref, false)?;

        Ok(new_fd as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr = addr as *mut libc::sockaddr_un;
        check_mut_ptr(addr)?; // TODO: check addr_len

        let new_socket = unix_socket.accept()?;
        let new_file_ref: Arc<Box<dyn File>> = Arc::new(Box::new(new_socket));
        let new_fd = process::put_file(new_file_ref, false)?;

        Ok(new_fd as isize)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_shutdown(fd: c_int, how: c_int) -> Result<isize> {
    debug!("shutdown: fd: {}, how: {}", fd, how);
    let file_ref = process::get_file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let ret = try_libc!(libc::ocall::shutdown(socket.fd(), how));
        Ok(ret as isize)
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

fn do_bind(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t) -> Result<isize> {
    debug!("bind: fd: {}, addr: {:?}, addr_len: {}", fd, addr, addr_len);
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!("listen: fd: {}, backlog: {}", fd, backlog);
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
        "setsockopt: fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd, level, optname, optval, optlen
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
        "getsockopt: fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd, level, optname, optval, optlen
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
        "getpeername: fd: {}, addr: {:?}, addr_len: {:?}",
        fd, addr, addr_len
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
        "getsockname: fd: {}, addr: {:?}, addr_len: {:?}",
        fd, addr, addr_len
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
        "sendto: fd: {}, base: {:?}, len: {}, addr: {:?}, addr_len: {}",
        fd, base, len, addr, addr_len
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
        "recvfrom: fd: {}, base: {:?}, len: {}, flags: {}, addr: {:?}, addr_len: {:?}",
        fd, base, len, flags, addr, addr_len
    );
    let file_ref = process::get_file(fd as FileDesc)?;
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
    debug!(
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

        debug!("socketpair: ({}, {})", sock_pair[0], sock_pair[1]);
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

fn handle_unsupported() -> Result<isize> {
    return_errno!(ENOSYS, "Unimplemented or unknown syscall")
}

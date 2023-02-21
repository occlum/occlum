use super::event_file::EventCreationFlags;
use super::file_ops;
use super::file_ops::{
    get_abs_path_by_fd, get_utimes, AccessibilityCheckFlags, AccessibilityCheckMode, ChownFlags,
    FcntlCmd, FsPath, LinkFlags, StatFlags, UnlinkFlags, Utime, UtimeFlags, AT_FDCWD, UTIME_OMIT,
};
use super::fs_ops;
use super::fs_ops::{MountFlags, MountOptions, UmountFlags};
use super::time::{clockid_t, itimerspec_t, timespec_t, timeval_t, ClockID};
use super::timer_file::{TimerCreationFlags, TimerSetFlags};
use super::*;
use config::ConfigMountFsType;
use util::mem_util::from_user;

#[allow(non_camel_case_types)]
pub struct iovec_t {
    base: *const c_void,
    len: size_t,
}

pub fn do_eventfd(init_val: u32) -> Result<isize> {
    do_eventfd2(init_val, 0)
}

pub fn do_eventfd2(init_val: u32, flags: i32) -> Result<isize> {
    info!("eventfd: initval {}, flags {} ", init_val, flags);

    let inner_flags =
        EventCreationFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let file_ref: Arc<dyn File> = {
        let event = EventFile::new(init_val, inner_flags)?;
        Arc::new(event)
    };

    let fd = current!().add_file(
        file_ref,
        inner_flags.contains(EventCreationFlags::EFD_CLOEXEC),
    );
    Ok(fd as isize)
}

pub fn do_timerfd_create(clockid: clockid_t, flags: i32) -> Result<isize> {
    debug!("timerfd: clockid {}, flags {} ", clockid, flags);

    let clockid = ClockID::from_raw(clockid)?;
    match clockid {
        ClockID::CLOCK_REALTIME | ClockID::CLOCK_MONOTONIC => {}
        _ => {
            return_errno!(EINVAL, "invalid clockid");
        }
    }
    let timer_create_flags =
        TimerCreationFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let file_ref: Arc<dyn File> = {
        let timer = TimerFile::new(clockid, timer_create_flags)?;
        Arc::new(timer)
    };

    let fd = current!().add_file(
        file_ref,
        timer_create_flags.contains(TimerCreationFlags::TFD_CLOEXEC),
    );
    Ok(fd as isize)
}

pub fn do_timerfd_settime(
    fd: FileDesc,
    flags: i32,
    new_value_ptr: *const itimerspec_t,
    old_value_ptr: *mut itimerspec_t,
) -> Result<isize> {
    from_user::check_ptr(new_value_ptr)?;
    let new_value = itimerspec_t::from_raw_ptr(new_value_ptr)?;
    let timer_set_flags =
        TimerSetFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;

    let current = current!();
    let file = current.file(fd)?;
    let timerfile = file.as_timer()?;
    let old_value = timerfile.set_time(timer_set_flags, &new_value)?;
    if !old_value_ptr.is_null() {
        from_user::check_mut_ptr(old_value_ptr)?;
        unsafe {
            old_value_ptr.write(old_value);
        }
    }
    Ok(0)
}

pub fn do_timerfd_gettime(fd: FileDesc, curr_value_ptr: *mut itimerspec_t) -> Result<isize> {
    from_user::check_mut_ptr(curr_value_ptr)?;
    let current = current!();
    let file = current.file(fd)?;
    let timerfile = file.as_timer()?;
    let curr_value = timerfile.time()?;
    unsafe {
        curr_value_ptr.write(curr_value);
    }
    Ok(0)
}

pub fn do_creat(path: *const i8, mode: u16) -> Result<isize> {
    let flags =
        AccessMode::O_WRONLY as u32 | (CreationFlags::O_CREAT | CreationFlags::O_TRUNC).bits();
    self::do_open(path, flags, mode)
}

pub fn do_open(path: *const i8, flags: u32, mode: u16) -> Result<isize> {
    self::do_openat(AT_FDCWD, path, flags, mode)
}

pub fn do_openat(dirfd: i32, path: *const i8, flags: u32, mode: u16) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let fs_path = FsPath::new(&path, dirfd, false)?;
    let mode = FileMode::from_bits_truncate(mode);
    let fd = file_ops::do_openat(&fs_path, flags, mode)?;
    Ok(fd as isize)
}

pub fn do_umask(mask: u16) -> Result<isize> {
    let new_mask = FileMode::from_bits_truncate(mask).to_umask();
    let old_mask = current!().process().set_umask(new_mask);
    Ok(old_mask.bits() as isize)
}

pub fn do_close(fd: FileDesc) -> Result<isize> {
    file_ops::do_close(fd)?;
    Ok(0)
}

pub fn do_read(fd: FileDesc, buf: *mut u8, size: usize) -> Result<isize> {
    let safe_buf = {
        from_user::check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = file_ops::do_read(fd, safe_buf)?;
    Ok(len as isize)
}

pub fn do_write(fd: FileDesc, buf: *const u8, size: usize) -> Result<isize> {
    let safe_buf = {
        from_user::check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    let len = file_ops::do_write(fd, safe_buf)?;
    Ok(len as isize)
}

fn do_writev_offset(
    fd: FileDesc,
    iov: *const iovec_t,
    count: i32,
    offset: Option<off_t>,
) -> Result<isize> {
    let count = {
        if count < 0 {
            return_errno!(EINVAL, "Invalid count of iovec");
        }
        count as usize
    };

    from_user::check_array(iov, count)?;
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

    let len = if let Some(offset) = offset {
        file_ops::do_pwritev(fd, bufs, offset)?
    } else {
        file_ops::do_writev(fd, bufs)?
    };

    Ok(len as isize)
}

pub fn do_writev(fd: FileDesc, iov: *const iovec_t, count: i32) -> Result<isize> {
    do_writev_offset(fd, iov, count, None)
}

fn do_readv_offset(
    fd: FileDesc,
    iov: *mut iovec_t,
    count: i32,
    offset: Option<off_t>,
) -> Result<isize> {
    let count = {
        if count < 0 {
            return_errno!(EINVAL, "Invalid count of iovec");
        }
        count as usize
    };

    from_user::check_array(iov, count)?;
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

    let len = if let Some(offset) = offset {
        file_ops::do_preadv(fd, bufs, offset)?
    } else {
        file_ops::do_readv(fd, bufs)?
    };

    Ok(len as isize)
}

pub fn do_readv(fd: FileDesc, iov: *mut iovec_t, count: i32) -> Result<isize> {
    do_readv_offset(fd, iov, count, None)
}

pub fn do_pread(fd: FileDesc, buf: *mut u8, size: usize, offset: off_t) -> Result<isize> {
    let safe_buf = {
        from_user::check_mut_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let len = file_ops::do_pread(fd, safe_buf, offset)?;
    Ok(len as isize)
}

pub fn do_preadv(fd: FileDesc, iov: *mut iovec_t, count: i32, offset: off_t) -> Result<isize> {
    if offset < 0 {
        return_errno!(EINVAL, "Invalid offset");
    }

    do_readv_offset(fd, iov, count, Some(offset))
}

pub fn do_pwrite(fd: FileDesc, buf: *const u8, size: usize, offset: off_t) -> Result<isize> {
    let safe_buf = {
        from_user::check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts(buf, size) }
    };
    let len = file_ops::do_pwrite(fd, safe_buf, offset)?;
    Ok(len as isize)
}

pub fn do_pwritev(fd: FileDesc, iov: *const iovec_t, count: i32, offset: off_t) -> Result<isize> {
    if offset < 0 {
        return_errno!(EINVAL, "Invalid offset");
    }

    do_writev_offset(fd, iov, count, Some(offset))
}

pub fn do_fstat(fd: FileDesc, stat_buf: *mut Stat) -> Result<isize> {
    from_user::check_mut_ptr(stat_buf)?;

    let stat = file_ops::do_fstat(fd)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

pub fn do_stat(path: *const i8, stat_buf: *mut Stat) -> Result<isize> {
    self::do_fstatat(AT_FDCWD, path, stat_buf, 0)
}

pub fn do_lstat(path: *const i8, stat_buf: *mut Stat) -> Result<isize> {
    self::do_fstatat(
        AT_FDCWD,
        path,
        stat_buf,
        StatFlags::AT_SYMLINK_NOFOLLOW.bits(),
    )
}

pub fn do_fstatat(dirfd: i32, path: *const i8, stat_buf: *mut Stat, flags: u32) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let flags = StatFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let fs_path = FsPath::new(&path, dirfd, flags.contains(StatFlags::AT_EMPTY_PATH))?;
    from_user::check_mut_ptr(stat_buf)?;
    let stat = file_ops::do_fstatat(&fs_path, flags)?;
    unsafe {
        stat_buf.write(stat);
    }
    Ok(0)
}

pub fn do_access(path: *const i8, mode: u32) -> Result<isize> {
    self::do_faccessat(AT_FDCWD, path, mode, 0)
}

pub fn do_faccessat(dirfd: i32, path: *const i8, mode: u32, flags: u32) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let fs_path = FsPath::new(&path, dirfd, false)?;
    let mode = AccessibilityCheckMode::from_u32(mode)?;
    let flags = AccessibilityCheckFlags::from_u32(flags)?;
    file_ops::do_faccessat(&fs_path, mode, flags).map(|_| 0)
}

pub fn do_lseek(fd: FileDesc, offset: off_t, whence: i32) -> Result<isize> {
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

    let offset = file_ops::do_lseek(fd, seek_from)?;
    Ok(offset as isize)
}

pub fn do_fsync(fd: FileDesc) -> Result<isize> {
    file_ops::do_fsync(fd)?;
    Ok(0)
}

pub fn do_fdatasync(fd: FileDesc) -> Result<isize> {
    file_ops::do_fdatasync(fd)?;
    Ok(0)
}

pub fn do_truncate(path: *const i8, len: usize) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    file_ops::do_truncate(&path, len)?;
    Ok(0)
}

pub fn do_ftruncate(fd: FileDesc, len: usize) -> Result<isize> {
    file_ops::do_ftruncate(fd, len)?;
    Ok(0)
}

pub fn do_getdents64(fd: FileDesc, buf: *mut u8, buf_size: usize) -> Result<isize> {
    let safe_buf = {
        from_user::check_mut_array(buf, buf_size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, buf_size) }
    };
    let len = file_ops::do_getdents64(fd, safe_buf)?;
    Ok(len as isize)
}

pub fn do_getdents(fd: FileDesc, buf: *mut u8, buf_size: usize) -> Result<isize> {
    let safe_buf = {
        from_user::check_mut_array(buf, buf_size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, buf_size) }
    };
    let len = file_ops::do_getdents(fd, safe_buf)?;
    Ok(len as isize)
}

pub fn do_sync() -> Result<isize> {
    fs_ops::do_sync()?;
    Ok(0)
}

pub fn do_pipe(fds_u: *mut i32) -> Result<isize> {
    do_pipe2(fds_u, 0)
}

pub fn do_pipe2(fds_u: *mut i32, flags: u32) -> Result<isize> {
    from_user::check_mut_array(fds_u, 2)?;
    // TODO: how to deal with open flags???
    let fds = pipe::do_pipe2(flags as u32)?;
    unsafe {
        *fds_u.offset(0) = fds[0] as c_int;
        *fds_u.offset(1) = fds[1] as c_int;
    }
    Ok(0)
}

pub fn do_dup(old_fd: FileDesc) -> Result<isize> {
    let new_fd = file_ops::do_dup(old_fd)?;
    Ok(new_fd as isize)
}

pub fn do_dup2(old_fd: FileDesc, new_fd: FileDesc) -> Result<isize> {
    let new_fd = file_ops::do_dup2(old_fd, new_fd)?;
    Ok(new_fd as isize)
}

pub fn do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32) -> Result<isize> {
    let new_fd = file_ops::do_dup3(old_fd, new_fd, flags)?;
    Ok(new_fd as isize)
}

pub fn do_chdir(path: *const i8) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    if path.is_empty() {
        return_errno!(ENOENT, "path is an empty string");
    } else if path.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "path name too long");
    }
    fs_ops::do_chdir(&path)?;
    Ok(0)
}

pub fn do_fchdir(fd: FileDesc) -> Result<isize> {
    let path = get_abs_path_by_fd(fd)?;
    fs_ops::do_chdir(&path)?;
    Ok(0)
}

pub fn do_getcwd(buf_ptr: *mut u8, size: usize) -> Result<isize> {
    let buf = {
        from_user::check_mut_array(buf_ptr, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf_ptr, size) }
    };

    let cwd = fs_ops::do_getcwd()?;
    if cwd.len() + 1 > buf.len() {
        return_errno!(ERANGE, "buf is not long enough");
    }
    buf[..cwd.len()].copy_from_slice(cwd.as_bytes());
    buf[cwd.len()] = b'\0';

    // The user-level library returns the pointer of buffer, the kernel just returns
    // the length of the buffer filled (which includes the ending '\0' character).
    Ok((cwd.len() + 1) as isize)
}

pub fn do_rename(oldpath: *const i8, newpath: *const i8) -> Result<isize> {
    self::do_renameat(AT_FDCWD, oldpath, AT_FDCWD, newpath)
}

pub fn do_renameat(
    olddirfd: i32,
    oldpath: *const i8,
    newdirfd: i32,
    newpath: *const i8,
) -> Result<isize> {
    let oldpath = from_user::clone_cstring_safely(oldpath)?
        .to_string_lossy()
        .into_owned();
    let newpath = from_user::clone_cstring_safely(newpath)?
        .to_string_lossy()
        .into_owned();
    let old_fs_path = FsPath::new(&oldpath, olddirfd, false)?;
    let new_fs_path = FsPath::new(&newpath, newdirfd, false)?;
    file_ops::do_renameat(&old_fs_path, &new_fs_path)?;
    Ok(0)
}

pub fn do_mkdir(path: *const i8, mode: u16) -> Result<isize> {
    self::do_mkdirat(AT_FDCWD, path, mode)
}

pub fn do_mkdirat(dirfd: i32, path: *const i8, mode: u16) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let fs_path = FsPath::new(&path, dirfd, false)?;
    let mode = FileMode::from_bits_truncate(mode);
    file_ops::do_mkdirat(&fs_path, mode)?;
    Ok(0)
}

pub fn do_rmdir(path: *const i8) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    if path.is_empty() {
        return_errno!(ENOENT, "path is an empty string");
    } else if path.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "path name too long");
    }
    file_ops::do_rmdir(&path)?;
    Ok(0)
}

pub fn do_link(oldpath: *const i8, newpath: *const i8) -> Result<isize> {
    self::do_linkat(AT_FDCWD, oldpath, AT_FDCWD, newpath, 0)
}

pub fn do_linkat(
    olddirfd: i32,
    oldpath: *const i8,
    newdirfd: i32,
    newpath: *const i8,
    flags: i32,
) -> Result<isize> {
    let oldpath = from_user::clone_cstring_safely(oldpath)?
        .to_string_lossy()
        .into_owned();
    let newpath = from_user::clone_cstring_safely(newpath)?
        .to_string_lossy()
        .into_owned();
    let flags = LinkFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let old_fs_path = FsPath::new(&oldpath, olddirfd, flags.contains(LinkFlags::AT_EMPTY_PATH))?;
    let new_fs_path = FsPath::new(&newpath, newdirfd, false)?;
    file_ops::do_linkat(&old_fs_path, &new_fs_path, flags)?;
    Ok(0)
}

pub fn do_unlink(path: *const i8) -> Result<isize> {
    self::do_unlinkat(AT_FDCWD, path, 0)
}

pub fn do_unlinkat(dirfd: i32, path: *const i8, flags: i32) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let fs_path = FsPath::new(&path, dirfd, false)?;
    let flags =
        UnlinkFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flag value"))?;
    file_ops::do_unlinkat(&fs_path, flags)?;
    Ok(0)
}

pub fn do_readlink(path: *const i8, buf: *mut u8, size: usize) -> Result<isize> {
    self::do_readlinkat(AT_FDCWD, path, buf, size)
}

pub fn do_readlinkat(dirfd: i32, path: *const i8, buf: *mut u8, size: usize) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let buf = {
        from_user::check_array(buf, size)?;
        unsafe { std::slice::from_raw_parts_mut(buf, size) }
    };
    let fs_path = FsPath::new(&path, dirfd, false)?;
    let len = file_ops::do_readlinkat(&fs_path, buf)?;
    Ok(len as isize)
}

pub fn do_symlink(target: *const i8, link_path: *const i8) -> Result<isize> {
    self::do_symlinkat(target, AT_FDCWD, link_path)
}

pub fn do_symlinkat(target: *const i8, new_dirfd: i32, link_path: *const i8) -> Result<isize> {
    let target = from_user::clone_cstring_safely(target)?
        .to_string_lossy()
        .into_owned();
    let link_path = from_user::clone_cstring_safely(link_path)?
        .to_string_lossy()
        .into_owned();
    let fs_path = FsPath::new(&link_path, new_dirfd, false)?;
    file_ops::do_symlinkat(&target, &fs_path)?;
    Ok(0)
}

pub fn do_chmod(path: *const i8, mode: u16) -> Result<isize> {
    self::do_fchmodat(AT_FDCWD, path, mode)
}

pub fn do_fchmod(fd: FileDesc, mode: u16) -> Result<isize> {
    let mode = FileMode::from_bits_truncate(mode);
    file_ops::do_fchmod(fd, mode)?;
    Ok(0)
}

pub fn do_fchmodat(dirfd: i32, path: *const i8, mode: u16) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let mode = FileMode::from_bits_truncate(mode);
    let fs_path = FsPath::new(&path, dirfd, false)?;
    file_ops::do_fchmodat(&fs_path, mode)?;
    Ok(0)
}

pub fn do_chown(path: *const i8, uid: u32, gid: u32) -> Result<isize> {
    self::do_fchownat(AT_FDCWD, path, uid, gid, 0)
}

pub fn do_fchown(fd: FileDesc, uid: u32, gid: u32) -> Result<isize> {
    file_ops::do_fchown(fd, uid, gid)?;
    Ok(0)
}

pub fn do_fchownat(dirfd: i32, path: *const i8, uid: u32, gid: u32, flags: i32) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    let flags = ChownFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let fs_path = FsPath::new(&path, dirfd, flags.contains(ChownFlags::AT_EMPTY_PATH))?;
    file_ops::do_fchownat(&fs_path, uid, gid, flags)?;
    Ok(0)
}

pub fn do_lchown(path: *const i8, uid: u32, gid: u32) -> Result<isize> {
    self::do_fchownat(
        AT_FDCWD,
        path,
        uid,
        gid,
        ChownFlags::AT_SYMLINK_NOFOLLOW.bits(),
    )
}

pub fn do_sendfile(
    out_fd: FileDesc,
    in_fd: FileDesc,
    offset_ptr: *mut off_t,
    count: usize,
) -> Result<isize> {
    let offset = if offset_ptr.is_null() {
        None
    } else {
        from_user::check_mut_ptr(offset_ptr)?;
        Some(unsafe { offset_ptr.read() })
    };

    let (len, offset) = file_ops::do_sendfile(out_fd, in_fd, offset, count)?;
    if !offset_ptr.is_null() {
        unsafe {
            offset_ptr.write(offset as off_t);
        }
    }
    Ok(len as isize)
}

pub fn do_fcntl(fd: FileDesc, cmd: u32, arg: u64) -> Result<isize> {
    let mut cmd = FcntlCmd::from_raw(cmd, arg)?;
    file_ops::do_fcntl(fd, &mut cmd)
}

pub fn do_ioctl(fd: FileDesc, cmd: u32, argp: *mut u8) -> Result<isize> {
    let mut ioctl_cmd = unsafe {
        if argp.is_null() == false {
            from_user::check_mut_ptr(argp)?;
        }
        IoctlCmd::new(cmd, argp)?
    };
    file_ops::do_ioctl(fd, &mut ioctl_cmd)?;
    Ok(0)
}

pub fn do_mount_rootfs(
    key_ptr: *const sgx_key_128bit_t,
    occlum_json_mac_ptr: *const sgx_aes_gcm_128bit_tag_t,
) -> Result<isize> {
    let key = if key_ptr.is_null() {
        None
    } else {
        Some(unsafe { key_ptr.read() })
    };
    if occlum_json_mac_ptr.is_null() {
        return_errno!(EINVAL, "occlum_json_mac_ptr cannot be null");
    }
    let expected_occlum_json_mac = unsafe { occlum_json_mac_ptr.read() };
    let user_config_path = unsafe { format!("{}{}", INSTANCE_DIR, "/build/Occlum.json.protected") };
    let user_config = config::load_config(&user_config_path, &expected_occlum_json_mac)?;
    fs_ops::do_mount_rootfs(&user_config, &key)?;
    Ok(0)
}

pub fn do_fallocate(fd: FileDesc, mode: u32, offset: off_t, len: off_t) -> Result<isize> {
    if offset < 0 || len <= 0 {
        return_errno!(
            EINVAL,
            "offset was less than 0, or len was less than or equal to 0"
        );
    }
    let flags = FallocateFlags::from_u32(mode)?;
    file_ops::do_fallocate(fd, flags, offset as usize, len as usize)?;
    Ok(0)
}

pub fn do_fstatfs(fd: FileDesc, statfs_buf: *mut Statfs) -> Result<isize> {
    from_user::check_mut_ptr(statfs_buf)?;

    let statfs = fs_ops::do_fstatfs(fd)?;
    unsafe {
        statfs_buf.write(statfs);
    }
    Ok(0)
}

pub fn do_statfs(path: *const i8, statfs_buf: *mut Statfs) -> Result<isize> {
    let path = from_user::clone_cstring_safely(path)?
        .to_string_lossy()
        .into_owned();
    if path.is_empty() {
        return_errno!(ENOENT, "path is an empty string");
    } else if path.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "path name too long");
    }
    from_user::check_mut_ptr(statfs_buf)?;

    let statfs = fs_ops::do_statfs(&path)?;
    unsafe {
        statfs_buf.write(statfs);
    }
    Ok(0)
}

pub fn do_mount(
    source: *const i8,
    target: *const i8,
    fs_type: *const i8,
    flags: u32,
    options: *const i8,
) -> Result<isize> {
    let source = from_user::clone_cstring_safely(source)?
        .to_string_lossy()
        .into_owned();
    let target = from_user::clone_cstring_safely(target)?
        .to_string_lossy()
        .into_owned();
    if target.is_empty() {
        return_errno!(ENOENT, "target is an empty string");
    } else if target.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "target name too long");
    }
    let flags = MountFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let mount_options = {
        let fs_type = {
            let fs_type = from_user::clone_cstring_safely(fs_type)?
                .to_string_lossy()
                .into_owned();
            ConfigMountFsType::from_input(fs_type.as_str())?
        };
        MountOptions::from_fs_type_and_options(&fs_type, options)?
    };

    fs_ops::do_mount(&source, &target, flags, mount_options)?;
    Ok(0)
}

pub fn do_umount(target: *const i8, flags: u32) -> Result<isize> {
    let target = from_user::clone_cstring_safely(target)?
        .to_string_lossy()
        .into_owned();
    let flags = UmountFlags::from_u32(flags)?;

    fs_ops::do_umount(&target, flags)?;
    Ok(0)
}

pub fn do_flock(fd: FileDesc, operation: i32) -> Result<isize> {
    let flock_ops = FlockOps::from_i32(operation)?;

    file_ops::do_flock(fd, flock_ops)?;
    Ok(0)
}

pub fn do_utime(path: *const i8, times_u: *const utimbuf_t) -> Result<isize> {
    let times = if !times_u.is_null() {
        from_user::check_ptr(times_u)?;
        let utimbuf = unsafe { *times_u };
        let atime = timespec_t::from(utimbuf.atime());
        atime.validate()?;
        let mtime = timespec_t::from(utimbuf.mtime());
        mtime.validate()?;
        Some((atime, mtime))
    } else {
        None
    };

    let (atime, mtime) = file_ops::get_utimes(times)?;
    self::do_utimes_wrapper(AT_FDCWD, path, atime, mtime, 0)
}

pub fn do_utimes(path: *const i8, times: *const timeval_t) -> Result<isize> {
    self::do_futimesat(AT_FDCWD, path, times)
}

pub fn do_futimesat(dirfd: i32, path: *const i8, times_u: *const timeval_t) -> Result<isize> {
    let times = if !times_u.is_null() {
        from_user::check_array(times_u, 2)?;
        let atime_ptr = unsafe { times_u.offset(0) };
        let atime = unsafe { *atime_ptr };
        let atime = timespec_t::from(atime);
        atime.validate()?;
        let mtime_ptr = unsafe { times_u.offset(1) };
        let mtime = unsafe { *mtime_ptr };
        let mtime = timespec_t::from(mtime);
        mtime.validate()?;
        Some((atime, mtime))
    } else {
        None
    };

    let (atime, mtime) = file_ops::get_utimes(times)?;
    self::do_utimes_wrapper(dirfd, path, atime, mtime, 0)
}

pub fn do_utimensat(
    dirfd: i32,
    path: *const i8,
    times_u: *const timespec_t,
    flags: i32,
) -> Result<isize> {
    let times = if !times_u.is_null() {
        from_user::check_array(times_u, 2)?;
        let atime_ptr = unsafe { times_u.offset(0) };
        let atime = unsafe { *atime_ptr };
        let mtime_ptr = unsafe { times_u.offset(1) };
        let mtime = unsafe { *mtime_ptr };
        if atime.nsec() == UTIME_OMIT && mtime.nsec() == UTIME_OMIT {
            return Ok(0);
        }
        Some((atime, mtime))
    } else {
        None
    };

    let (atime, mtime) = file_ops::get_utimes(times)?;
    self::do_utimes_wrapper(dirfd, path, atime, mtime, flags)
}

fn do_utimes_wrapper(
    dirfd: i32,
    path: *const i8,
    atime: Utime,
    mtime: Utime,
    flags: i32,
) -> Result<isize> {
    if path.is_null() && dirfd != AT_FDCWD {
        file_ops::do_utimes_fd(dirfd as FileDesc, atime, mtime, flags)
    } else {
        let path = from_user::clone_cstring_safely(path)?
            .to_string_lossy()
            .into_owned();
        let flags = UtimeFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
        let fs_path = FsPath::new(&path, dirfd, false)?;
        file_ops::do_utimes_path(&fs_path, atime, mtime, flags)
    }
}

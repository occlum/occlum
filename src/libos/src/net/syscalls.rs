use super::*;

use super::io_multiplexing::{AsEpollFile, EpollCtlCmd, EpollEventFlags, EpollFile};
use fs::{CreationFlags, File, FileDesc, FileRef};
use process::Process;
use std::convert::TryFrom;
use util::mem_util::from_user;

pub fn do_sendmsg(fd: c_int, msg_ptr: *const msghdr, flags_c: c_int) -> Result<isize> {
    debug!(
        "sendmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_ptr, flags_c
    );

    let file_ref = process::get_file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let msg_c = {
            from_user::check_ptr(msg_ptr)?;
            let msg_c = unsafe { &*msg_ptr };
            msg_c.check_member_ptrs()?;
            msg_c
        };
        let msg = unsafe { MsgHdr::from_c(&msg_c)? };

        let flags = SendFlags::from_bits_truncate(flags_c);

        socket
            .sendmsg(&msg, flags)
            .map(|bytes_sent| bytes_sent as isize)
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        return_errno!(EBADF, "does not support unix socket")
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

pub fn do_recvmsg(fd: c_int, msg_mut_ptr: *mut msghdr_mut, flags_c: c_int) -> Result<isize> {
    debug!(
        "recvmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_mut_ptr, flags_c
    );

    let file_ref = process::get_file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_socket() {
        let msg_mut_c = {
            from_user::check_mut_ptr(msg_mut_ptr)?;
            let msg_mut_c = unsafe { &mut *msg_mut_ptr };
            msg_mut_c.check_member_ptrs()?;
            msg_mut_c
        };
        let mut msg_mut = unsafe { MsgHdrMut::from_c(msg_mut_c)? };

        let flags = RecvFlags::from_bits_truncate(flags_c);

        socket
            .recvmsg(&mut msg_mut, flags)
            .map(|bytes_recvd| bytes_recvd as isize)
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        return_errno!(EBADF, "does not support unix socket")
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

#[allow(non_camel_case_types)]
trait c_msghdr_ext {
    fn check_member_ptrs(&self) -> Result<()>;
}

impl c_msghdr_ext for msghdr {
    // TODO: implement this!
    fn check_member_ptrs(&self) -> Result<()> {
        Ok(())
    }
    /*
            ///user space check
            pub unsafe fn check_from_user(user_hdr: *const msghdr) -> Result<()> {
                Self::check_pointer(user_hdr, from_user::check_ptr)
            }

            ///Check msghdr ptr
            pub unsafe fn check_pointer(
                user_hdr: *const msghdr,
                check_ptr: fn(*const u8) -> Result<()>,
            ) -> Result<()> {
                check_ptr(user_hdr as *const u8)?;

                if (*user_hdr).msg_name.is_null() ^ ((*user_hdr).msg_namelen == 0) {
                    return_errno!(EINVAL, "name length is invalid");
                }

                if (*user_hdr).msg_iov.is_null() ^ ((*user_hdr).msg_iovlen == 0) {
                    return_errno!(EINVAL, "iov length is invalid");
                }

                if (*user_hdr).msg_control.is_null() ^ ((*user_hdr).msg_controllen == 0) {
                    return_errno!(EINVAL, "control length is invalid");
                }

                if !(*user_hdr).msg_name.is_null() {
                    check_ptr((*user_hdr).msg_name as *const u8)?;
                }

                if !(*user_hdr).msg_iov.is_null() {
                    check_ptr((*user_hdr).msg_iov as *const u8)?;
                    let iov_slice = slice::from_raw_parts((*user_hdr).msg_iov, (*user_hdr).msg_iovlen);
                    for iov in iov_slice {
                        check_ptr(iov.iov_base as *const u8)?;
                    }
                }

                if !(*user_hdr).msg_control.is_null() {
                    check_ptr((*user_hdr).msg_control as *const u8)?;
                }
                Ok(())
            }
    */
}

impl c_msghdr_ext for msghdr_mut {
    fn check_member_ptrs(&self) -> Result<()> {
        Ok(())
    }
}

pub fn do_select(
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
        from_user::check_mut_ptr(readfds)?;
        unsafe { &mut *readfds }
    } else {
        &mut zero_fds0
    };
    let writefds = if !writefds.is_null() {
        from_user::check_mut_ptr(writefds)?;
        unsafe { &mut *writefds }
    } else {
        &mut zero_fds1
    };
    let exceptfds = if !exceptfds.is_null() {
        from_user::check_mut_ptr(exceptfds)?;
        unsafe { &mut *exceptfds }
    } else {
        &mut zero_fds2
    };
    let timeout = if !timeout.is_null() {
        from_user::check_ptr(timeout)?;
        Some(unsafe { timeout.read() })
    } else {
        None
    };

    let n = io_multiplexing::do_select(nfds, readfds, writefds, exceptfds, timeout)?;
    Ok(n as isize)
}

pub fn do_poll(fds: *mut libc::pollfd, nfds: libc::nfds_t, timeout: c_int) -> Result<isize> {
    from_user::check_mut_array(fds, nfds as usize)?;
    let polls = unsafe { std::slice::from_raw_parts_mut(fds, nfds as usize) };

    let n = io_multiplexing::do_poll(polls, timeout)?;
    Ok(n as isize)
}

pub fn do_epoll_create(size: c_int) -> Result<isize> {
    if size <= 0 {
        return_errno!(EINVAL, "size is not positive");
    }
    do_epoll_create1(0)
}

pub fn do_epoll_create1(raw_flags: c_int) -> Result<isize> {
    // Only O_CLOEXEC is valid
    let flags = CreationFlags::from_bits(raw_flags as u32)
        .ok_or_else(|| errno!(EINVAL, "invalid flags"))?
        & CreationFlags::O_CLOEXEC;
    let epoll_file = io_multiplexing::EpollFile::new(flags)?;
    let file_ref: Arc<Box<dyn File>> = Arc::new(Box::new(epoll_file));
    let close_on_spawn = flags.contains(CreationFlags::O_CLOEXEC);
    let fd = process::put_file(file_ref, close_on_spawn)?;

    Ok(fd as isize)
}

pub fn do_epoll_ctl(
    epfd: c_int,
    op: c_int,
    fd: c_int,
    event: *const libc::epoll_event,
) -> Result<isize> {
    debug!("epoll_ctl: epfd: {}, op: {:?}, fd: {}", epfd, op, fd);
    let inner_event = if !event.is_null() {
        from_user::check_ptr(event)?;
        Some(EpollEvent::from_raw(unsafe { &*event })?)
    } else {
        None
    };

    let epfile_ref = process::get_file(epfd as FileDesc)?;
    let epoll_file = epfile_ref.as_epfile()?;

    epoll_file.control(
        EpollCtlCmd::try_from(op)?,
        fd as FileDesc,
        inner_event.as_ref(),
    )?;
    Ok(0)
}

pub fn do_epoll_wait(
    epfd: c_int,
    events: *mut libc::epoll_event,
    max_events: c_int,
    timeout: c_int,
) -> Result<isize> {
    let max_events = {
        if max_events <= 0 {
            return_errno!(EINVAL, "maxevents <= 0");
        }
        max_events as usize
    };
    let raw_events = {
        from_user::check_mut_array(events, max_events)?;
        unsafe { std::slice::from_raw_parts_mut(events, max_events) }
    };

    // A new vector to store EpollEvent, which may degrade the performance due to extra copy.
    let mut inner_events: Vec<EpollEvent> =
        vec![EpollEvent::new(EpollEventFlags::empty(), 0); max_events];

    debug!(
        "epoll_wait: epfd: {}, len: {:?}, timeout: {}",
        epfd,
        raw_events.len(),
        timeout
    );

    let epfile_ref = process::get_file(epfd as FileDesc)?;
    let epoll_file = epfile_ref.as_epfile()?;

    let count = epoll_file.wait(&mut inner_events, timeout)?;

    for i in 0..count {
        raw_events[i] = inner_events[i].to_raw();
    }

    Ok(count as isize)
}

pub fn do_epoll_pwait(
    epfd: c_int,
    events: *mut libc::epoll_event,
    maxevents: c_int,
    timeout: c_int,
    sigmask: *const usize, //TODO:add sigset_t
) -> Result<isize> {
    if !sigmask.is_null() {
        warn!("epoll_pwait cannot handle signal mask, yet");
    } else {
        info!("epoll_wait");
    }
    do_epoll_wait(epfd, events, maxevents, timeout)
}

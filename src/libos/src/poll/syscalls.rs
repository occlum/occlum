use std::time::Duration;

use super::do_epoll::{EpollCtl, EpollEvent, EpollFile, EpollFlags};
use super::do_poll::PollFd;
use crate::fs::CreationFlags;
use crate::misc::resource_t;
use crate::prelude::*;
use crate::util::mem_util::from_user;

pub async fn do_epoll_create(size: c_int) -> Result<isize> {
    if size <= 0 {
        return_errno!(EINVAL, "size is not positive");
    }
    do_epoll_create1(0).await
}

pub async fn do_epoll_create1(raw_flags: c_int) -> Result<isize> {
    debug!("epoll_create: raw_flags: {:?}", raw_flags);

    // Only O_CLOEXEC is valid
    let flags = CreationFlags::from_bits(raw_flags as u32)
        .ok_or_else(|| errno!(EINVAL, "invalid flags"))?
        & CreationFlags::O_CLOEXEC;
    let epoll_file: Arc<EpollFile> = EpollFile::new();
    let file_ref = FileRef::new_epoll(epoll_file);
    let close_on_spawn = flags.contains(CreationFlags::O_CLOEXEC);
    let epfd = current!().add_file(file_ref, close_on_spawn);
    Ok(epfd as isize)
}

pub async fn do_epoll_ctl(
    epfd: c_int,
    op: c_int,
    fd: c_int,
    event_ptr: *const libc::epoll_event,
) -> Result<isize> {
    debug!("epoll_ctl: epfd: {}, op: {:?}, fd: {}", epfd, op, fd);

    let get_c_event = |event_ptr| -> Result<&libc::epoll_event> {
        from_user::check_ptr(event_ptr)?;
        Ok(unsafe { &*event_ptr })
    };

    let fd = fd as FileDesc;
    let ctl_cmd = match op {
        libc::EPOLL_CTL_ADD => {
            let c_event = get_c_event(event_ptr)?;
            let event = EpollEvent::from(c_event);
            let flags = EpollFlags::from_bits_truncate(c_event.events);
            EpollCtl::Add(fd, event, flags)
        }
        libc::EPOLL_CTL_DEL => EpollCtl::Del(fd),
        libc::EPOLL_CTL_MOD => {
            let c_event = get_c_event(event_ptr)?;
            let event = EpollEvent::from(c_event);
            let flags = EpollFlags::from_bits_truncate(c_event.events);
            EpollCtl::Mod(fd, event, flags)
        }
        _ => return_errno!(EINVAL, "invalid op"),
    };

    let file_ref = current!().file(epfd as FileDesc)?;
    let epoll_file = file_ref
        .as_epoll_file()
        .ok_or_else(|| errno!(EINVAL, "not an epoll file"))?;

    epoll_file.control(&ctl_cmd)?;
    Ok(0)
}

pub async fn do_epoll_wait(
    epfd: c_int,
    events: *mut libc::epoll_event,
    max_events: c_int,
    timeout_ms: c_int,
) -> Result<isize> {
    debug!(
        "epoll_wait: epfd: {}, max_events: {:?}, timeout_ms: {}",
        epfd, max_events, timeout_ms
    );

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

    debug!(
        "epoll_wait: epfd: {}, len: {:?}, timeout: {}",
        epfd,
        raw_events.len(),
        timeout_ms,
    );

    let file_ref = current!().file(epfd as FileDesc)?;
    let epoll_file = file_ref
        .as_epoll_file()
        .ok_or_else(|| errno!(EINVAL, "not an epoll file"))?;
    let timeout = if timeout_ms >= 0 {
        Some(Duration::from_millis(timeout_ms as u64))
    } else {
        None
    };
    let ep_events = epoll_file.wait(max_events /*, timeout.as_ref()*/).await?;

    for (i, ep_event) in ep_events.iter().enumerate() {
        raw_events[i] = ep_event.into();
    }
    Ok(ep_events.len() as isize)
}

pub async fn do_epoll_pwait(
    epfd: c_int,
    events: *mut libc::epoll_event,
    maxevents: c_int,
    timeout: c_int,
    sigmask: *const usize, //TODO:add sigset_t
) -> Result<isize> {
    if !sigmask.is_null() {
        warn!("epoll_pwait cannot handle signal mask, yet");
    } else {
        debug!("epoll_wait");
    }
    do_epoll_wait(epfd, events, maxevents, timeout).await
}

pub async fn do_poll(
    fds: *mut libc::pollfd,
    nfds: libc::nfds_t,
    timeout_ms: c_int,
) -> Result<isize> {
    // It behaves like sleep when fds is null and nfds is zero.
    if !fds.is_null() || nfds != 0 {
        from_user::check_mut_array(fds, nfds as usize)?;
    }

    let soft_rlimit_nofile = current!()
        .rlimits()
        .lock()
        .unwrap()
        .get(resource_t::RLIMIT_NOFILE)
        .get_cur();
    // TODO: Check nfds against the size of the stack used in ocall to prevent stack overflow
    if nfds > soft_rlimit_nofile {
        return_errno!(EINVAL, "The nfds value exceeds the RLIMIT_NOFILE value.");
    }

    let raw_poll_fds = unsafe { std::slice::from_raw_parts_mut(fds, nfds as usize) };
    let poll_fds: Vec<PollFd> = raw_poll_fds.iter().map(|raw| PollFd::from(raw)).collect();

    let mut timeout = if timeout_ms >= 0 {
        Some(Duration::from_millis(timeout_ms as u64))
    } else {
        None
    };

    let count = super::do_poll::do_poll(&poll_fds, timeout.as_mut()).await?;

    for (raw_poll_fd, poll_fd) in raw_poll_fds.iter_mut().zip(poll_fds.iter()) {
        raw_poll_fd.revents = poll_fd.revents().get().bits() as i16;
    }
    Ok(count as isize)
}

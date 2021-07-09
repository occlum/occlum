use std::time::Duration;

use super::do_poll::PollFd;
use crate::misc::resource_t;
use crate::prelude::*;
use crate::util::mem_util::from_user;

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

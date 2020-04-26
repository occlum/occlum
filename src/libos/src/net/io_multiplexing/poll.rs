use super::*;

pub fn do_poll(pollfds: &mut [libc::pollfd], timeout: c_int) -> Result<usize> {
    debug!(
        "poll: {:?}, timeout: {}",
        pollfds.iter().map(|p| p.fd).collect::<Vec<_>>(),
        timeout
    );

    // Untrusted pollfd's that will be modified by OCall
    let mut u_pollfds: Vec<libc::pollfd> = pollfds.to_vec();

    let current = current!();
    for (i, pollfd) in pollfds.iter_mut().enumerate() {
        // Poll should just ignore negative fds
        if pollfd.fd < 0 {
            u_pollfds[i].fd = -1;
            u_pollfds[i].revents = 0;
            continue;
        }

        let file_ref = current.file(pollfd.fd as FileDesc)?;
        if let Ok(socket) = file_ref.as_socket() {
            // convert libos fd to host fd in the copy to keep pollfds unchanged
            u_pollfds[i].fd = socket.fd();
            u_pollfds[i].revents = 0;
        } else if let Ok(eventfd) = file_ref.as_event() {
            u_pollfds[i].fd = eventfd.get_host_fd();
            u_pollfds[i].revents = 0;
        } else if let Ok(socket) = file_ref.as_unix_socket() {
            // FIXME: spin poll until can read (hack for php)
            while (pollfd.events & libc::POLLIN) != 0 && socket.poll()?.0 == false {
                spin_loop_hint();
            }

            let (r, w, e) = socket.poll()?;
            if r {
                pollfd.revents |= libc::POLLIN;
            }
            if w {
                pollfd.revents |= libc::POLLOUT;
            }
            pollfd.revents &= pollfd.events;
            if e {
                pollfd.revents |= libc::POLLERR;
            }
            warn!("poll unix socket is unimplemented, spin for read");
            return Ok(1);
        } else if let Ok(dev_random) = file_ref.as_dev_random() {
            return Ok(dev_random.poll(pollfd)?);
        } else {
            return_errno!(EBADF, "not a supported file type");
        }
    }

    let (u_pollfds_ptr, u_pollfds_len) = u_pollfds.as_mut_slice().as_mut_ptr_and_len();

    let num_events = try_libc!(libc::ocall::poll(
        u_pollfds_ptr,
        u_pollfds_len as u64,
        timeout
    )) as usize;
    assert!(num_events <= pollfds.len());

    // Copy back revents from the untrusted pollfds
    let mut num_nonzero_revents = 0;
    for (i, pollfd) in pollfds.iter_mut().enumerate() {
        if u_pollfds[i].revents == 0 {
            continue;
        }
        pollfd.revents = u_pollfds[i].revents;
        num_nonzero_revents += 1;
    }
    assert!(num_nonzero_revents == num_events);
    Ok(num_events as usize)
}

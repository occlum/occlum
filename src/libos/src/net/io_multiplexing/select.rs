use super::*;

/// Forward to host `poll`
/// (sgx_libc doesn't have `select`)
pub fn do_select(
    nfds: usize,
    readfds: &mut libc::fd_set,
    writefds: &mut libc::fd_set,
    exceptfds: &mut libc::fd_set,
    timeout: Option<libc::timeval>,
) -> Result<usize> {
    debug!("select: nfds: {}", nfds);
    // convert libos fd to Linux fd
    let mut host_to_libos_fd = [0; libc::FD_SETSIZE];
    let mut polls = Vec::<libc::pollfd>::new();

    let current = current!();
    let file_table = current.files().lock().unwrap();

    for fd in 0..nfds {
        let fd_ref = file_table.get(fd as FileDesc)?;
        let (r, w, e) = (
            readfds.is_set(fd),
            writefds.is_set(fd),
            exceptfds.is_set(fd),
        );
        if !(r || w || e) {
            continue;
        }
        if let Ok(socket) = fd_ref.as_unix_socket() {
            warn!("select unix socket is unimplemented, spin for read");
            readfds.clear();
            writefds.clear();
            exceptfds.clear();

            // FIXME: spin poll until can read (hack for php)
            while r && socket.poll()?.0 == false {
                spin_loop_hint();
            }

            let (rr, ww, ee) = socket.poll()?;
            if r && rr {
                readfds.set(fd);
            }
            if w && ww {
                writefds.set(fd);
            }
            if e && ee {
                writefds.set(fd);
            }
            return Ok(1);
        }
        let host_fd = if let Ok(socket) = fd_ref.as_socket() {
            socket.fd()
        } else if let Ok(eventfd) = fd_ref.as_event() {
            eventfd.get_host_fd()
        } else {
            return_errno!(EBADF, "unsupported file type");
        };

        host_to_libos_fd[host_fd as usize] = fd;
        let mut events = 0;
        if r {
            events |= libc::POLLIN;
        }
        if w {
            events |= libc::POLLOUT;
        }
        if e {
            events |= libc::POLLERR;
        }

        polls.push(libc::pollfd {
            fd: host_fd as c_int,
            events,
            revents: 0,
        });
    }

    // Unlock the file table as early as possible
    drop(file_table);

    let timeout = match timeout {
        None => -1,
        Some(tv) => (tv.tv_sec * 1000 + tv.tv_usec / 1000) as i32,
    };

    let (polls_ptr, polls_len) = polls.as_mut_slice().as_mut_ptr_and_len();
    let ret = try_libc!(libc::ocall::poll(polls_ptr, polls_len as u64, timeout));

    // convert fd back and write fdset
    readfds.clear();
    writefds.clear();
    exceptfds.clear();

    for poll in polls.iter() {
        let fd = host_to_libos_fd[poll.fd as usize];
        if poll.revents & libc::POLLIN != 0 {
            readfds.set(fd);
        }
        if poll.revents & libc::POLLOUT != 0 {
            writefds.set(fd);
        }
        if poll.revents & libc::POLLERR != 0 {
            exceptfds.set(fd);
        }
    }

    Ok(ret as usize)
}

/// Safe methods for `libc::fd_set`
trait FdSetExt {
    fn set(&mut self, fd: usize);
    fn clear(&mut self);
    fn is_set(&mut self, fd: usize) -> bool;
}

impl FdSetExt for libc::fd_set {
    fn set(&mut self, fd: usize) {
        assert!(fd < libc::FD_SETSIZE);
        unsafe {
            libc::FD_SET(fd as c_int, self);
        }
    }

    fn clear(&mut self) {
        unsafe {
            libc::FD_ZERO(self);
        }
    }

    fn is_set(&mut self, fd: usize) -> bool {
        assert!(fd < libc::FD_SETSIZE);
        unsafe { libc::FD_ISSET(fd as c_int, self) }
    }
}

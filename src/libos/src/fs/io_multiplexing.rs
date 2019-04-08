use super::*;
use crate::syscall::AsSocket;
use std::vec::Vec;

/// Forward to host `poll`
/// (sgx_libc doesn't have `select`)
pub fn do_select(
    nfds: usize,
    readfds: &mut libc::fd_set,
    writefds: &mut libc::fd_set,
    exceptfds: &mut libc::fd_set,
    timeout: Option<libc::timeval>,
) -> Result<usize, Error> {
    // convert libos fd to Linux fd
    let mut host_to_libos_fd = [0; libc::FD_SETSIZE];
    let mut polls = Vec::<libc::pollfd>::new();

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_table_ref = proc.get_files().lock().unwrap();

    for fd in 0..nfds {
        let (r, w, e) = (readfds.is_set(fd), writefds.is_set(fd), exceptfds.is_set(fd));
        if !(r || w || e) {
            continue;
        }
        let host_fd = file_table_ref.get(fd as FileDesc)?.as_socket()?.fd();

        host_to_libos_fd[host_fd as usize] = fd;
        let mut events = 0;
        if r { events |= libc::POLLIN; }
        if w { events |= libc::POLLOUT; }
        if e { events |= libc::POLLERR; }

        polls.push(libc::pollfd {
            fd: host_fd as c_int,
            events,
            revents: 0,
        });
    }

    let timeout = match timeout {
        None => -1,
        Some(tv) => (tv.tv_sec * 1000 + tv.tv_usec / 1000) as i32,
    };

    let ret = unsafe {
        libc::ocall::poll(polls.as_mut_ptr(), polls.len() as u64, timeout)
    };

    if ret < 0 {
        return Err(Error::new(Errno::from_retval(ret as i32), ""));
    }

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

pub fn do_poll(
    polls: &mut [libc::pollfd],
    timeout: c_int,
) -> Result<usize, Error> {
    info!("poll: [..], timeout: {}", timeout);

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();

    // convert libos fd to Linux fd
    for poll in polls.iter_mut() {
        let file_ref = proc.get_files().lock().unwrap().get(poll.fd as FileDesc)?;
        let socket = file_ref.as_socket()?;
        poll.fd = socket.fd();
    }
    let ret = unsafe {
        libc::ocall::poll(polls.as_mut_ptr(), polls.len() as u64, timeout)
    };
    // recover fd ?

    if ret < 0 {
        Err(Error::new(Errno::from_retval(ret as i32), ""))
    } else {
        Ok(ret as usize)
    }
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

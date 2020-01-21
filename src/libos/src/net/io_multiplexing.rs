use super::*;
use fs::{File, FileDesc, FileRef};
use std::any::Any;
use std::collections::btree_map::BTreeMap;
use std::fmt;
use std::sync::atomic::spin_loop_hint;
use std::vec::Vec;

/// Forward to host `poll`
/// (sgx_libc doesn't have `select`)
pub fn do_select(
    nfds: usize,
    readfds: &mut libc::fd_set,
    writefds: &mut libc::fd_set,
    exceptfds: &mut libc::fd_set,
    timeout: Option<libc::timeval>,
) -> Result<usize> {
    info!("select: nfds: {}", nfds);
    // convert libos fd to Linux fd
    let mut host_to_libos_fd = [0; libc::FD_SETSIZE];
    let mut polls = Vec::<libc::pollfd>::new();

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_table_ref = proc.get_files().lock().unwrap();

    for fd in 0..nfds {
        let (r, w, e) = (
            readfds.is_set(fd),
            writefds.is_set(fd),
            exceptfds.is_set(fd),
        );
        if !(r || w || e) {
            continue;
        }
        if let Ok(socket) = file_table_ref.get(fd as FileDesc)?.as_unix_socket() {
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
        let host_fd = file_table_ref.get(fd as FileDesc)?.as_socket()?.fd();

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

    let timeout = match timeout {
        None => -1,
        Some(tv) => (tv.tv_sec * 1000 + tv.tv_usec / 1000) as i32,
    };

    let ret = try_libc!(libc::ocall::poll(
        polls.as_mut_ptr(),
        polls.len() as u64,
        timeout
    ));

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

pub fn do_poll(polls: &mut [libc::pollfd], timeout: c_int) -> Result<usize> {
    info!(
        "poll: {:?}, timeout: {}",
        polls.iter().map(|p| p.fd).collect::<Vec<_>>(),
        timeout
    );

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();

    // convert libos fd to Linux fd
    for poll in polls.iter_mut() {
        let file_ref = proc.get_files().lock().unwrap().get(poll.fd as FileDesc)?;
        if let Ok(socket) = file_ref.as_socket() {
            poll.fd = socket.fd();
        } else if let Ok(socket) = file_ref.as_unix_socket() {
            // FIXME: spin poll until can read (hack for php)
            while (poll.events & libc::POLLIN) != 0 && socket.poll()?.0 == false {
                spin_loop_hint();
            }

            let (r, w, e) = socket.poll()?;
            if r {
                poll.revents |= libc::POLLIN;
            }
            if w {
                poll.revents |= libc::POLLOUT;
            }
            if e {
                poll.revents |= libc::POLLERR;
            }
            poll.revents &= poll.events;
            warn!("poll unix socket is unimplemented, spin for read");
            return Ok(1);
        } else {
            return_errno!(EBADF, "not a socket");
        }
    }
    let ret = try_libc!(libc::ocall::poll(
        polls.as_mut_ptr(),
        polls.len() as u64,
        timeout
    ));
    // recover fd ?
    Ok(ret as usize)
}

pub fn do_epoll_create1(flags: c_int) -> Result<FileDesc> {
    info!("epoll_create1: flags: {}", flags);

    let epoll = EpollFile::new()?;
    let file_ref: Arc<Box<dyn File>> = Arc::new(Box::new(epoll));
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let fd = {
        let close_on_spawn = flags & libc::EPOLL_CLOEXEC != 0;
        proc.get_files()
            .lock()
            .unwrap()
            .put(file_ref, close_on_spawn)
    };
    Ok(fd)
}

pub fn do_epoll_ctl(
    epfd: FileDesc,
    op: c_int,
    fd: FileDesc,
    event: *const libc::epoll_event,
) -> Result<()> {
    info!("epoll_ctl: epfd: {}, op: {:?}, fd: {}", epfd, op, fd);

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let mut file_table_ref = proc.get_files().lock().unwrap();
    let mut file_ref = file_table_ref.get(epfd)?;
    let mut epoll = file_ref.as_epoll()?.inner.lock().unwrap();

    let fd_ref = file_table_ref.get(fd)?;
    let sock_result = fd_ref.as_socket();
    if sock_result.is_err() {
        //FIXME: workaround for grpc, other fd types including pipe should be supported
        return Ok(());
    }

    let host_fd = sock_result.unwrap().fd() as FileDesc;
    epoll.ctl(op, host_fd, event)?;

    Ok(())
}

pub fn do_epoll_wait(
    epfd: FileDesc,
    events: &mut [libc::epoll_event],
    timeout: c_int,
) -> Result<usize> {
    info!(
        "epoll_wait: epfd: {}, len: {:?}, timeout: {}",
        epfd,
        events.len(),
        timeout
    );

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let mut file_ref = proc.get_files().lock().unwrap().get(epfd)?;
    let mut epoll = file_ref.as_epoll()?.inner.lock().unwrap();

    let count = epoll.wait(events, timeout)?;
    Ok(count)
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

pub struct EpollFile {
    inner: SgxMutex<EpollFileInner>,
}

impl EpollFile {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: SgxMutex::new(EpollFileInner::new()?),
        })
    }
}

struct EpollFileInner {
    epoll_fd: c_int,
}

// FIXME: What if a Linux fd is closed but still in an epoll?
impl EpollFileInner {
    /// Create a new Linux epoll file descriptor
    pub fn new() -> Result<Self> {
        let ret = try_libc!(libc::ocall::epoll_create1(0));
        Ok(EpollFileInner { epoll_fd: ret })
    }

    pub fn ctl(
        &mut self,
        op: c_int,
        host_fd: FileDesc,
        event: *const libc::epoll_event,
    ) -> Result<()> {
        let ret = try_libc!(libc::ocall::epoll_ctl(
            self.epoll_fd,
            op,
            host_fd as c_int,
            event as *mut _
        ));
        Ok(())
    }

    /// Wait for an I/O event on the epoll.
    /// Returns the number of file descriptors ready for the requested I/O.
    pub fn wait(&mut self, events: &mut [libc::epoll_event], timeout: c_int) -> Result<usize> {
        let ret = try_libc!(libc::ocall::epoll_wait(
            self.epoll_fd,
            events.as_mut_ptr(),
            events.len() as c_int,
            timeout,
        ));
        Ok(ret as usize)
    }
}

impl Drop for EpollFileInner {
    fn drop(&mut self) {
        unsafe {
            libc::ocall::close(self.epoll_fd);
        }
    }
}

impl File for EpollFile {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Debug for EpollFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let inner = self.inner.lock().unwrap();
        f.debug_struct("EpollFile")
            .field("epoll_fd", &inner.epoll_fd)
            .finish()
    }
}

pub trait AsEpoll {
    fn as_epoll(&self) -> Result<&EpollFile>;
}

impl AsEpoll for FileRef {
    fn as_epoll(&self) -> Result<&EpollFile> {
        self.as_any()
            .downcast_ref::<EpollFile>()
            .ok_or_else(|| errno!(EBADF, "not a epoll"))
    }
}

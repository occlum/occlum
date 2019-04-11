use super::*;
use crate::syscall::AsSocket;
use std::vec::Vec;
use std::collections::btree_map::BTreeMap;
use std::fmt;
use std::any::Any;

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

pub fn do_epoll_create1(flags: c_int) -> Result<FileDesc, Error> {
    info!("epoll_create1: flags: {}", flags);

    let epoll = EpollFile::new()?;
    let file_ref: Arc<Box<File>> = Arc::new(Box::new(epoll));
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let fd = {
        let close_on_spawn = flags & libc::EPOLL_CLOEXEC != 0;
        proc.get_files().lock().unwrap().put(file_ref, close_on_spawn)
    };
    Ok(fd)
}

pub fn do_epoll_ctl(
    epfd: FileDesc,
    op: EpollOp,
    fd: FileDesc,
) -> Result<(), Error> {
    info!("epoll_ctl: epfd: {}, op: {:?}, fd: {}", epfd, op, fd);

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let mut file_table_ref = proc.get_files().lock().unwrap();
    let mut file_ref = file_table_ref.get(epfd)?;
    let mut epoll = file_ref.as_epoll()?.inner.lock().unwrap();

    match op {
        EpollOp::Add(event) => {
            let host_fd = file_table_ref.get(fd)?.as_socket()?.fd() as FileDesc;
            epoll.add(fd, host_fd, event)?;
        },
        EpollOp::Modify(event) => epoll.modify(fd, event)?,
        EpollOp::Delete => epoll.remove(fd)?,
    }
    Ok(())
}

pub fn do_epoll_wait(
    epfd: FileDesc,
    events: &mut [libc::epoll_event],
    timeout: c_int,
) -> Result<usize, Error> {
    info!("epoll_wait: epfd: {}, len: {:?}, timeout: {}", epfd, events.len(), timeout);

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

pub enum EpollOp {
    Add(libc::epoll_event),
    Modify(libc::epoll_event),
    Delete
}

impl Debug for EpollOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            EpollOp::Add(_) => "Add",
            EpollOp::Modify(_) => "Modify",
            EpollOp::Delete => "Delete",
        };
        write!(f, "{}", s)
    }
}

pub struct EpollFile {
    inner: SgxMutex<EpollFileInner>,
}

impl EpollFile {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            inner: SgxMutex::new(EpollFileInner::new()?)
        })
    }
}

struct EpollFileInner {
    epoll_fd: c_int,
    fd_to_host: BTreeMap<FileDesc, FileDesc>,
    fd_to_libos: BTreeMap<FileDesc, FileDesc>,
}

// FIXME: What if a Linux fd is closed but still in an epoll?
impl EpollFileInner {
    /// Create a new Linux epoll file descriptor
    pub fn new() -> Result<Self, Error> {
        let ret = unsafe { libc::ocall::epoll_create1(0) };
        if ret < 0 {
            return Err(Error::new(Errno::from_retval(ret as i32), ""));
        }
        Ok(EpollFileInner {
            epoll_fd: ret,
            fd_to_host: BTreeMap::new(),
            fd_to_libos: BTreeMap::new(),
        })
    }

    /// Add `fd` to the interest list and associate the settings
    /// specified in `event` with the internal file linked to `fd`.
    pub fn add(&mut self, fd: FileDesc, host_fd: FileDesc, mut event: libc::epoll_event) -> Result<(), Error> {
        if self.fd_to_host.contains_key(&fd) {
            return Err(Error::new(EEXIST, "fd is exist in epoll"));
        }
        let ret = unsafe {
            libc::ocall::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_ADD,
                host_fd as c_int,
                &mut event,
            )
        };
        if ret < 0 {
            return Err(Error::new(Errno::from_retval(ret as i32), ""));
        }
        self.fd_to_host.insert(fd, host_fd);
        self.fd_to_libos.insert(host_fd, fd);
        Ok(())
    }

    /// Change the settings associated with `fd` in the interest list to
    /// the new settings specified in `event`.
    pub fn modify(&mut self, fd: FileDesc, mut event: libc::epoll_event) -> Result<(), Error> {
        let host_fd = *self.fd_to_host.get(&fd)
            .ok_or(Error::new(EINVAL, "fd is not exist in epoll"))?;
        let ret = unsafe {
            libc::ocall::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_MOD,
                host_fd as c_int,
                &mut event,
            )
        };
        if ret < 0 {
            return Err(Error::new(Errno::from_retval(ret as i32), ""));
        }
        Ok(())
    }

    /// Remove the target file descriptor `fd` from the interest list.
    pub fn remove(&mut self, fd: FileDesc) -> Result<(), Error> {
        let host_fd = *self.fd_to_host.get(&fd)
            .ok_or(Error::new(EINVAL, "fd is not exist in epoll"))?;
        let ret = unsafe {
            libc::ocall::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_DEL,
                host_fd as c_int,
                core::ptr::null_mut(),
            )
        };
        if ret < 0 {
            return Err(Error::new(Errno::from_retval(ret as i32), ""));
        }
        self.fd_to_host.remove(&fd);
        self.fd_to_libos.remove(&host_fd);
        Ok(())
    }

    /// Wait for an I/O event on the epoll.
    /// Returns the number of file descriptors ready for the requested I/O.
    pub fn wait(
        &mut self,
        events: &mut [libc::epoll_event],
        timeout: c_int,
    ) -> Result<usize, Error> {
        let ret = unsafe {
            libc::ocall::epoll_wait(
                self.epoll_fd,
                events.as_mut_ptr(),
                events.len() as c_int,
                timeout,
            )
        };
        if ret < 0 {
            return Err(Error::new(Errno::from_retval(ret as i32), ""));
        }
        // convert host fd to libos
        let count = ret as usize;
        for event in events[0..count].iter_mut() {
            let host_fd = event.u64 as FileDesc;
            let fd = self.fd_to_libos[&host_fd];
            event.u64 = fd as u64;
        }
        Ok(count)
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
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        Err(Error::new(Errno::ESPIPE, "Epoll does not support seek"))
    }

    fn metadata(&self) -> Result<Metadata, Error> {
        unimplemented!()
    }

    fn set_len(&self, len: u64) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_all(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_data(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn read_entry(&self) -> Result<String, Error> {
        unimplemented!()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl Debug for EpollFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let inner = self.inner.lock().unwrap();
        f.debug_struct("EpollFile")
            .field("epoll_fd", &inner.epoll_fd)
            .field("fds", &inner.fd_to_host.keys())
            .field("host_fds", &inner.fd_to_host.values())
            .finish()
    }
}

pub trait AsEpoll {
    fn as_epoll(&self) -> Result<&EpollFile, Error>;
}

impl AsEpoll for FileRef {
    fn as_epoll(&self) -> Result<&EpollFile, Error> {
        self.as_any()
            .downcast_ref::<EpollFile>()
            .ok_or(Error::new(Errno::EBADF, "not a epoll"))
    }
}

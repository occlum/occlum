use super::*;

pub fn select(
    nfds: c_int,
    readfds: &mut libc::fd_set,
    writefds: &mut libc::fd_set,
    exceptfds: &mut libc::fd_set,
    timeout: Option<&mut timeval_t>,
) -> Result<isize> {
    debug!("select: nfds: {} timeout: {:?}", nfds, timeout);

    let current = current!();
    let file_table = current.files().lock().unwrap();

    let mut max_host_fd = None;
    let mut host_to_libos_fd = [None; libc::FD_SETSIZE];
    let mut unsafe_readfds = libc::fd_set::new_empty();
    let mut unsafe_writefds = libc::fd_set::new_empty();
    let mut unsafe_exceptfds = libc::fd_set::new_empty();

    for fd in 0..(nfds as FileDesc) {
        let (r, w, e) = (
            readfds.is_set(fd),
            writefds.is_set(fd),
            exceptfds.is_set(fd),
        );
        if !(r || w || e) {
            continue;
        }

        let fd_ref = file_table.get(fd)?;

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
            let mut ready_num = 0;
            if r && rr {
                readfds.set(fd)?;
                ready_num += 1;
            }
            if w && ww {
                writefds.set(fd)?;
                ready_num += 1;
            }
            if e && ee {
                exceptfds.set(fd)?;
                ready_num += 1;
            }
            return Ok(ready_num);
        }

        let host_fd = if let Ok(socket) = fd_ref.as_socket() {
            socket.fd()
        } else if let Ok(eventfd) = fd_ref.as_event() {
            eventfd.get_host_fd()
        } else {
            return_errno!(EBADF, "unsupported file type");
        } as FileDesc;

        if host_fd as usize >= libc::FD_SETSIZE {
            return_errno!(EBADF, "host fd exceeds FD_SETSIZE");
        }

        // convert libos fd to host fd
        host_to_libos_fd[host_fd as usize] = Some(fd);
        max_host_fd = Some(max(max_host_fd.unwrap_or(0), host_fd as c_int));
        if r {
            unsafe_readfds.set(host_fd)?;
        }
        if w {
            unsafe_writefds.set(host_fd)?;
        }
        if e {
            unsafe_exceptfds.set(host_fd)?;
        }
    }

    // Unlock the file table as early as possible
    drop(file_table);

    let host_nfds = if let Some(fd) = max_host_fd {
        fd + 1
    } else {
        // Set nfds to zero if no fd is monitored
        0
    };

    let ret = do_select_in_host(
        host_nfds,
        &mut unsafe_readfds,
        &mut unsafe_writefds,
        &mut unsafe_exceptfds,
        timeout,
    )?;

    // convert fd back and write fdset and do ocall check
    let mut ready_num = 0;
    for host_fd in 0..host_nfds as FileDesc {
        let fd_option = host_to_libos_fd[host_fd as usize];
        let (r, w, e) = (
            unsafe_readfds.is_set(host_fd),
            unsafe_writefds.is_set(host_fd),
            unsafe_exceptfds.is_set(host_fd),
        );
        if !(r || w || e) {
            if let Some(fd) = fd_option {
                readfds.unset(fd)?;
                writefds.unset(fd)?;
                exceptfds.unset(fd)?;
            }
            continue;
        }

        let fd = fd_option.expect("host_fd with events must have a responding libos fd");

        if r {
            assert!(readfds.is_set(fd));
            ready_num += 1;
        } else {
            readfds.unset(fd)?;
        }
        if w {
            assert!(writefds.is_set(fd));
            ready_num += 1;
        } else {
            writefds.unset(fd)?;
        }
        if e {
            assert!(exceptfds.is_set(fd));
            ready_num += 1;
        } else {
            exceptfds.unset(fd)?;
        }
    }

    assert!(ready_num == ret);
    Ok(ret)
}

fn do_select_in_host(
    host_nfds: c_int,
    readfds: &mut libc::fd_set,
    writefds: &mut libc::fd_set,
    exceptfds: &mut libc::fd_set,
    timeout: Option<&mut timeval_t>,
) -> Result<isize> {
    let readfds_ptr = readfds.as_raw_ptr_mut();
    let writefds_ptr = writefds.as_raw_ptr_mut();
    let exceptfds_ptr = exceptfds.as_raw_ptr_mut();

    let mut origin_timeout: timeval_t = Default::default();
    let timeout_ptr = if let Some(to) = timeout {
        origin_timeout = *to;
        to
    } else {
        std::ptr::null_mut()
    } as *mut timeval_t;

    let ret = try_libc!({
        let mut retval: c_int = 0;
        let status = occlum_ocall_select(
            &mut retval,
            host_nfds,
            readfds_ptr,
            writefds_ptr,
            exceptfds_ptr,
            timeout_ptr,
        );
        assert!(status == sgx_status_t::SGX_SUCCESS);

        retval
    }) as isize;

    if !timeout_ptr.is_null() {
        let time_left = unsafe { *(timeout_ptr) };
        time_left.validate()?;
        assert!(time_left.as_duration() <= origin_timeout.as_duration());
    }

    Ok(ret)
}

/// Safe methods for `libc::fd_set`
pub trait FdSetExt {
    fn new_empty() -> Self;
    fn unset(&mut self, fd: FileDesc) -> Result<()>;
    fn is_set(&self, fd: FileDesc) -> bool;
    fn set(&mut self, fd: FileDesc) -> Result<()>;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn as_raw_ptr_mut(&mut self) -> *mut Self;
}

impl FdSetExt for libc::fd_set {
    fn new_empty() -> Self {
        unsafe { core::mem::zeroed() }
    }

    fn unset(&mut self, fd: FileDesc) -> Result<()> {
        if fd as usize >= libc::FD_SETSIZE {
            return_errno!(EINVAL, "fd exceeds FD_SETSIZE");
        }
        unsafe {
            libc::FD_CLR(fd as c_int, self);
        }
        Ok(())
    }

    fn set(&mut self, fd: FileDesc) -> Result<()> {
        if fd as usize >= libc::FD_SETSIZE {
            return_errno!(EINVAL, "fd exceeds FD_SETSIZE");
        }
        unsafe {
            libc::FD_SET(fd as c_int, self);
        }
        Ok(())
    }

    fn clear(&mut self) {
        unsafe {
            libc::FD_ZERO(self);
        }
    }

    fn is_set(&self, fd: FileDesc) -> bool {
        if fd as usize >= libc::FD_SETSIZE {
            return false;
        }
        unsafe { libc::FD_ISSET(fd as c_int, self as *const Self as *mut Self) }
    }

    fn is_empty(&self) -> bool {
        let set = unsafe {
            std::slice::from_raw_parts(self as *const Self as *const u64, libc::FD_SETSIZE / 64)
        };
        set.iter().all(|&x| x == 0)
    }

    fn as_raw_ptr_mut(&mut self) -> *mut Self {
        if self.is_empty() {
            std::ptr::null_mut()
        } else {
            self as *mut libc::fd_set
        }
    }
}

extern "C" {
    fn occlum_ocall_select(
        ret: *mut c_int,
        nfds: c_int,
        readfds: *mut libc::fd_set,
        writefds: *mut libc::fd_set,
        exceptfds: *mut libc::fd_set,
        timeout: *mut timeval_t,
    ) -> sgx_status_t;
}

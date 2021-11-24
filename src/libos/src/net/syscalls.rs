use super::*;

use std::mem::MaybeUninit;
use std::time::Duration;

use super::io_multiplexing::{AsEpollFile, EpollCtl, EpollFile, EpollFlags, FdSetExt, PollFd};
use fs::{CreationFlags, File, FileDesc, FileRef};
use misc::resource_t;
use process::Process;
use signal::{sigset_t, SigSet};
use std::convert::TryFrom;
use time::{timespec_t, timeval_t};
use util::mem_util::from_user;

pub fn do_socket(domain: c_int, socket_type: c_int, protocol: c_int) -> Result<isize> {
    let sock_domain = AddressFamily::try_from(domain as u16)?;
    let file_flags = FileFlags::from_bits_truncate(socket_type);
    let sock_type = SocketType::try_from(socket_type & (!file_flags.bits()))?;

    let file_ref: Arc<dyn File> = match sock_domain {
        AddressFamily::LOCAL => {
            let unix_socket = unix_socket(sock_type, file_flags, protocol)?;
            Arc::new(unix_socket)
        }
        _ => {
            let socket = HostSocket::new(sock_domain, sock_type, file_flags, protocol)?;
            Arc::new(socket)
        }
    };

    let close_on_spawn = file_flags.contains(FileFlags::SOCK_CLOEXEC);
    let fd = current!().add_file(file_ref, close_on_spawn);
    Ok(fd as isize)
}

pub fn do_bind(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t) -> Result<isize> {
    if addr.is_null() || addr_len == 0 {
        return_errno!(EINVAL, "no address is specified");
    }
    from_user::check_array(addr as *const u8, addr_len as usize)?;

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let sock_addr = unsafe { SockAddr::try_from_raw(addr, addr_len)? };
        trace!("bind to addr: {:?}", sock_addr);
        socket.bind(&sock_addr)?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let mut unix_addr = unsafe { UnixAddr::try_from_raw(addr, addr_len)? };
        trace!("bind to addr: {:?}", unix_addr);
        unix_socket.bind(&mut unix_addr)?;
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    }

    Ok(0)
}

pub fn do_listen(fd: c_int, backlog: c_int) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        socket.listen(backlog)?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket.listen(backlog)?;
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    }

    Ok(0)
}

pub fn do_connect(
    fd: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    // For SOCK_DGRAM sockets not initiated in connection-mode,
    // if address is a null address for the protocol,
    // the socket's peer address shall be reset.
    let addr_set: bool = !addr.is_null();
    if addr_set {
        from_user::check_array(addr as *const u8, addr_len as usize)?;
    }

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let addr_option = if addr_set {
            Some(unsafe { SockAddr::try_from_raw(addr, addr_len)? })
        } else {
            None
        };

        socket.connect(&addr_option)?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        // TODO: support AF_UNSPEC address for datagram socket use
        let addr = if addr_set {
            unsafe { UnixAddr::try_from_raw(addr, addr_len)? }
        } else {
            return_errno!(EINVAL, "invalid address");
        };

        unix_socket.connect(&addr)?;
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    }

    Ok(0)
}

pub fn do_accept(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    do_accept4(fd, addr, addr_len, 0)
}

pub fn do_accept4(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
    flags: c_int,
) -> Result<isize> {
    let addr_set: bool = !addr.is_null();
    if addr_set {
        from_user::check_ptr(addr_len)?;
        from_user::check_mut_array(addr as *mut u8, unsafe { *addr_len } as usize)?;
    }

    let file_flags = FileFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let close_on_spawn = file_flags.contains(FileFlags::SOCK_CLOEXEC);

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let (new_socket_file, sock_addr_option) = socket.accept(file_flags)?;
        let new_file_ref: Arc<dyn File> = Arc::new(new_socket_file);
        let new_fd = current!().add_file(new_file_ref, close_on_spawn);

        if addr_set {
            if let Some(sock_addr) = sock_addr_option {
                let mut buf =
                    unsafe { std::slice::from_raw_parts_mut(addr as *mut u8, *addr_len as usize) };
                sock_addr.copy_to_slice(&mut buf);
                unsafe {
                    *addr_len = sock_addr.len() as u32;
                }
            } else {
                unsafe {
                    *addr_len = 0;
                }
            }
        }
        Ok(new_fd as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let (new_socket_file, sock_addr_option) = unix_socket.accept(file_flags)?;
        let new_file_ref: Arc<dyn File> = Arc::new(new_socket_file);
        let new_fd = current!().add_file(new_file_ref, close_on_spawn);

        if addr_set {
            if let Some(sock_addr) = sock_addr_option {
                let mut buf =
                    unsafe { std::slice::from_raw_parts_mut(addr as *mut u8, *addr_len as usize) };
                sock_addr.copy_to_slice(&mut buf);
                unsafe {
                    *addr_len = sock_addr.raw_len() as u32;
                }
            } else {
                unsafe {
                    *addr_len = 0;
                }
            }
        }
        Ok(new_fd as isize)
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    }
}

pub fn do_shutdown(fd: c_int, how: c_int) -> Result<isize> {
    debug!("shutdown: fd: {}, how: {}", fd, how);
    let how = HowToShut::try_from_raw(how)?;

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        socket.shutdown(how)?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket.shutdown(how)?;
    } else {
        return_errno!(EBADF, "not a host socket")
    }

    Ok(0)
}

pub fn do_setsockopt(
    fd: c_int,
    level: c_int,
    optname: c_int,
    optval: *const c_void,
    optlen: libc::socklen_t,
) -> Result<isize> {
    debug!(
        "setsockopt: fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd, level, optname, optval, optlen
    );
    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let ret = try_libc!(libc::ocall::setsockopt(
            socket.raw_host_fd() as i32,
            level,
            optname,
            optval,
            optlen
        ));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("setsockopt for unix socket is unimplemented");
        Ok(0)
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
}

pub fn do_getsockopt(
    fd: c_int,
    level: c_int,
    optname: c_int,
    optval: *mut c_void,
    optlen: *mut libc::socklen_t,
) -> Result<isize> {
    debug!(
        "getsockopt: fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd, level, optname, optval, optlen
    );
    let file_ref = current!().file(fd as FileDesc)?;
    let socket = file_ref.as_host_socket();

    if let Ok(socket) = socket {
        let ret = try_libc!(libc::ocall::getsockopt(
            socket.raw_host_fd() as i32,
            level,
            optname,
            optval,
            optlen
        ));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("getsockopt for unix socket is unimplemented");
        Ok(0)
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
}

pub fn do_getpeername(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let addr_set: bool = !addr.is_null();
    if addr_set {
        from_user::check_ptr(addr_len)?;
        from_user::check_mut_array(addr as *mut u8, unsafe { *addr_len } as usize)?;
    } else {
        return Ok(0);
    }

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let ret = try_libc!(libc::ocall::getpeername(
            socket.raw_host_fd() as i32,
            addr,
            addr_len
        ));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let name = unix_socket.peer_addr()?;
        let mut dst = unsafe {
            std::slice::from_raw_parts_mut(addr as *mut _ as *mut u8, *addr_len as usize)
        };
        name.copy_to_slice(dst);
        unsafe {
            *addr_len = name.raw_len() as u32;
        }
        Ok(0)
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
}

pub fn do_getsockname(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let addr_set: bool = !addr.is_null();
    if addr_set {
        from_user::check_ptr(addr_len)?;
        from_user::check_mut_array(addr as *mut u8, unsafe { *addr_len } as usize)?;
    } else {
        return Ok(0);
    }

    if unsafe { *addr_len } < std::mem::size_of::<libc::sa_family_t>() as u32 {
        return_errno!(EINVAL, "input length is too short");
    }

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let ret = try_libc!(libc::ocall::getsockname(
            socket.raw_host_fd() as i32,
            addr,
            addr_len
        ));
        Ok(ret as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let name_opt = unix_socket.addr();
        if let Some(name) = name_opt {
            let mut dst = unsafe {
                std::slice::from_raw_parts_mut(addr as *mut _ as *mut u8, *addr_len as usize)
            };
            name.copy_to_slice(dst);
            unsafe {
                *addr_len = name.raw_len() as u32;
            }
        } else {
            unsafe {
                (*addr).sa_family = AddressFamily::LOCAL as u16;
                *addr_len = 2;
            }
        }
        Ok(0)
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    }
}

pub fn do_sendto(
    fd: c_int,
    base: *const c_void,
    len: size_t,
    flags: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    if len == 0 {
        return Ok(0);
    }

    if addr.is_null() ^ (addr_len == 0) {
        return_errno!(EINVAL, "addr and ddr_len should be both null");
    }

    from_user::check_array(base as *const u8, len)?;
    let buf = unsafe { std::slice::from_raw_parts(base as *const u8, len as usize) };

    let addr_set: bool = !addr.is_null();
    if addr_set {
        from_user::check_mut_array(addr as *mut u8, addr_len as usize)?;
    }

    let send_flags = SendFlags::from_bits(flags).unwrap();

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let addr_option = if addr_set {
            Some(unsafe { SockAddr::try_from_raw(addr, addr_len)? })
        } else {
            None
        };

        socket
            .sendto(buf, send_flags, &addr_option)
            .map(|u| u as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr_option = if addr_set {
            Some(unsafe { UnixAddr::try_from_raw(addr, addr_len)? })
        } else {
            None
        };

        unix_socket
            .sendto(buf, send_flags, &addr_option)
            .map(|u| u as isize)
    } else {
        return_errno!(EBADF, "unsupported file type");
    }
}

pub fn do_recvfrom(
    fd: c_int,
    base: *mut c_void,
    len: size_t,
    flags: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    if addr.is_null() ^ addr_len.is_null() {
        return_errno!(EINVAL, "addr and ddr_len should be both null");
    }

    from_user::check_array(base as *mut u8, len)?;
    let mut buf = unsafe { std::slice::from_raw_parts_mut(base as *mut u8, len as usize) };

    // MSG_CTRUNC is a return flag but linux allows it to be set on input flags.
    // We just ignore it.
    let recv_flags = RecvFlags::from_bits(flags & !(MsgHdrFlags::MSG_CTRUNC.bits()))
        .ok_or_else(|| errno!(EINVAL, "invalid flags"))?;

    let addr_set: bool = !addr.is_null();
    if addr_set {
        from_user::check_ptr(addr_len)?;
        from_user::check_mut_array(addr as *mut u8, unsafe { *addr_len } as usize)?;
    }

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let (data_len, sock_addr_option) = socket.recvfrom(buf, recv_flags)?;
        if addr_set {
            if let Some(sock_addr) = sock_addr_option {
                let mut buf =
                    unsafe { std::slice::from_raw_parts_mut(addr as *mut u8, *addr_len as usize) };
                sock_addr.copy_to_slice(&mut buf);
                unsafe {
                    *addr_len = sock_addr.len() as u32;
                }
            } else {
                unsafe {
                    *addr_len = 0;
                }
            }
        }
        Ok(data_len as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let (data_len, sock_addr_option) = unix_socket.recvfrom(buf, recv_flags)?;
        if addr_set {
            if let Some(sock_addr) = sock_addr_option {
                let mut buf =
                    unsafe { std::slice::from_raw_parts_mut(addr as *mut u8, *addr_len as usize) };
                sock_addr.copy_to_slice(&mut buf);
                unsafe {
                    *addr_len = sock_addr.raw_len() as u32;
                }
            } else {
                unsafe {
                    *addr_len = 0;
                }
            }
        }
        Ok(data_len as isize)
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    }
}

pub fn do_socketpair(
    domain: c_int,
    socket_type: c_int,
    protocol: c_int,
    sv: *mut c_int,
) -> Result<isize> {
    let mut sock_pair = unsafe {
        from_user::check_mut_array(sv, 2)?;
        std::slice::from_raw_parts_mut(sv as *mut u32, 2)
    };

    let file_flags = FileFlags::from_bits_truncate(socket_type);
    let close_on_spawn = file_flags.contains(FileFlags::SOCK_CLOEXEC);
    let sock_type = SocketType::try_from(socket_type & (!file_flags.bits()))?;

    let domain = AddressFamily::try_from(domain as u16)?;
    if (domain == AddressFamily::LOCAL) {
        let (client_socket, server_socket) = socketpair(sock_type, file_flags, protocol as i32)?;

        let current = current!();
        let mut files = current.files().lock().unwrap();
        sock_pair[0] = files.put(Arc::new(client_socket), close_on_spawn);
        sock_pair[1] = files.put(Arc::new(server_socket), close_on_spawn);

        debug!("socketpair: ({}, {})", sock_pair[0], sock_pair[1]);
        Ok(0)
    } else {
        return_errno!(EAFNOSUPPORT, "domain not supported")
    }
}

pub fn do_sendmsg(fd: c_int, msg_ptr: *const msghdr, flags_c: c_int) -> Result<isize> {
    debug!(
        "sendmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_ptr, flags_c
    );

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
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
        return_errno!(EOPNOTSUPP, "does not support unix socket")
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
}

pub fn do_recvmsg(fd: c_int, msg_mut_ptr: *mut msghdr_mut, flags_c: c_int) -> Result<isize> {
    debug!(
        "recvmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_mut_ptr, flags_c
    );

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
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
        return_errno!(EOPNOTSUPP, "does not support unix socket")
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
}

pub fn do_sendmmsg(
    fd: c_int,
    msgvec_ptr: *mut mmsghdr,
    vlen: c_uint,
    flags_c: c_int,
) -> Result<isize> {
    debug!(
        "sendmmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msgvec_ptr, flags_c
    );

    from_user::check_ptr(msgvec_ptr)?;

    let mut msgvec = unsafe { std::slice::from_raw_parts_mut(msgvec_ptr, vlen as usize) };
    let flags = SendFlags::from_bits_truncate(flags_c);
    let file_ref = current!().file(fd as FileDesc)?;

    if let Ok(socket) = file_ref.as_host_socket() {
        let mut send_count = 0;
        for mmsg in (msgvec) {
            if !mmsg.msg_hdr.check_member_ptrs().is_ok() {
                break;
            }

            let msg = unsafe {
                if let Ok(msg) = MsgHdr::from_c({ &mmsg.msg_hdr }) {
                    msg
                } else {
                    break;
                }
            };

            if socket
                .sendmsg(&msg, flags)
                .map(|bytes_sent| {
                    mmsg.msg_len = bytes_sent as u32;
                    mmsg.msg_len
                })
                .is_ok()
            {
                send_count += 1;
            } else {
                break;
            }
        }

        Ok(send_count as isize)
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        return_errno!(EOPNOTSUPP, "does not support unix socket")
    } else {
        return_errno!(ENOTSOCK, "not a socket")
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
    timeout: *mut timeval_t,
) -> Result<isize> {
    let nfds = {
        let soft_rlimit_nofile = current!()
            .rlimits()
            .lock()
            .unwrap()
            .get(resource_t::RLIMIT_NOFILE)
            .get_cur();
        if nfds < 0 || nfds > libc::FD_SETSIZE as i32 || nfds as u64 > soft_rlimit_nofile {
            return_errno!(
                EINVAL,
                "nfds is negative or exceeds the resource limit or FD_SETSIZE"
            );
        }
        nfds as FileDesc
    };

    let mut timeout_c = if !timeout.is_null() {
        from_user::check_ptr(timeout)?;
        let timeval = unsafe { &mut *timeout };
        timeval.validate()?;
        Some(timeval)
    } else {
        None
    };
    let mut timeout = timeout_c.as_ref().map(|timeout_c| timeout_c.as_duration());

    let readfds = if !readfds.is_null() {
        from_user::check_mut_ptr(readfds)?;
        Some(unsafe { &mut *readfds })
    } else {
        None
    };
    let writefds = if !writefds.is_null() {
        from_user::check_mut_ptr(writefds)?;
        Some(unsafe { &mut *writefds })
    } else {
        None
    };
    let exceptfds = if !exceptfds.is_null() {
        from_user::check_mut_ptr(exceptfds)?;
        Some(unsafe { &mut *exceptfds })
    } else {
        None
    };

    let ret = io_multiplexing::do_select(nfds, readfds, writefds, exceptfds, timeout.as_mut());

    if let Some(timeout_c) = timeout_c {
        *timeout_c = timeout.unwrap().into();
    }

    ret
}

pub fn do_ppoll(
    fds: *mut libc::pollfd,
    nfds: libc::nfds_t,
    timeout_ts: *const timespec_t,
    sigmask: *const sigset_t,
) -> Result<isize> {
    let mut timeout = if timeout_ts.is_null() {
        None
    } else {
        from_user::check_ptr(timeout_ts)?;
        let timeout_ts = unsafe { &*timeout_ts };
        Some(timeout_ts.as_duration())
    };
    if !sigmask.is_null() {
        warn!("ppoll sigmask is not supported!");
    }
    do_poll_common(fds, nfds, timeout.as_mut(), None)
}

pub fn do_poll(fds: *mut libc::pollfd, nfds: libc::nfds_t, timeout_ms: c_int) -> Result<isize> {
    let mut timeout = if timeout_ms >= 0 {
        Some(Duration::from_millis(timeout_ms as u64))
    } else {
        None
    };
    do_poll_common(fds, nfds, timeout.as_mut(), None)
}

fn do_poll_common(
    fds: *mut libc::pollfd,
    nfds: libc::nfds_t,
    timeout: Option<&mut Duration>,
    sigmask: Option<SigSet>,
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
    let poll_fds: Vec<PollFd> = raw_poll_fds
        .iter()
        .map(|raw| PollFd::from_raw(raw))
        .collect();

    let count = io_multiplexing::do_poll_new(&poll_fds, timeout)?;

    for (raw_poll_fd, poll_fd) in raw_poll_fds.iter_mut().zip(poll_fds.iter()) {
        raw_poll_fd.revents = poll_fd.revents().get().to_raw() as i16;
    }
    Ok(count as isize)
}

pub fn do_epoll_create(size: c_int) -> Result<isize> {
    if size <= 0 {
        return_errno!(EINVAL, "size is not positive");
    }
    do_epoll_create1(0)
}

pub fn do_epoll_create1(raw_flags: c_int) -> Result<isize> {
    debug!("epoll_create: raw_flags: {:?}", raw_flags);

    // Only O_CLOEXEC is valid
    let flags = CreationFlags::from_bits(raw_flags as u32)
        .ok_or_else(|| errno!(EINVAL, "invalid flags"))?
        & CreationFlags::O_CLOEXEC;
    let epoll_file: Arc<EpollFile> = EpollFile::new();
    let close_on_spawn = flags.contains(CreationFlags::O_CLOEXEC);
    let epfd = current!().add_file(epoll_file, close_on_spawn);
    Ok(epfd as isize)
}

pub fn do_epoll_ctl(
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
            let event = EpollEvent::from_c(c_event);
            let flags = EpollFlags::from_c(c_event);
            EpollCtl::Add(fd, event, flags)
        }
        libc::EPOLL_CTL_DEL => EpollCtl::Del(fd),
        libc::EPOLL_CTL_MOD => {
            let c_event = get_c_event(event_ptr)?;
            let event = EpollEvent::from_c(c_event);
            let flags = EpollFlags::from_c(c_event);
            EpollCtl::Mod(fd, event, flags)
        }
        _ => return_errno!(EINVAL, "invalid op"),
    };

    let epfile_ref = current!().file(epfd as FileDesc)?;
    let epoll_file = epfile_ref.as_epoll_file()?;

    epoll_file.control(&ctl_cmd)?;
    Ok(0)
}

pub fn do_epoll_wait(
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

    // A new vector to store EpollEvent, which may degrade the performance due to extra copy.
    let mut inner_events: Vec<MaybeUninit<EpollEvent>> = vec![MaybeUninit::uninit(); max_events];

    debug!(
        "epoll_wait: epfd: {}, len: {:?}, timeout: {}",
        epfd,
        raw_events.len(),
        timeout_ms,
    );

    let epfile_ref = current!().file(epfd as FileDesc)?;
    let epoll_file = epfile_ref.as_epoll_file()?;
    let timeout = if timeout_ms >= 0 {
        Some(Duration::from_millis(timeout_ms as u64))
    } else {
        None
    };
    let count = epoll_file.wait(&mut inner_events, timeout.as_ref())?;

    for i in 0..count {
        raw_events[i] = unsafe { inner_events[i].assume_init() }.to_c();
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
        debug!("epoll_wait");
    }
    do_epoll_wait(epfd, events, maxevents, timeout)
}

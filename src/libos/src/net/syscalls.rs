use super::socket::{mmsghdr, MsgFlags, SocketFlags, SocketProtocol};

use atomic::Ordering;
use core::f32::consts::E;
use num_enum::TryFromPrimitive;
use std::mem::MaybeUninit;
use std::ptr;
use std::time::Duration;

use super::io_multiplexing::{AsEpollFile, EpollCtl, EpollFile, EpollFlags, FdSetExt, PollFd};
use fs::{CreationFlags, File, FileDesc, FileRef, IoctlCmd};
use misc::resource_t;
use process::Process;
use signal::{sigset_t, MaskOp, SigSet, SIGKILL, SIGSTOP};
use std::convert::TryFrom;
use time::{timespec_t, timeval_t};
use util::mem_util::from_user;

use super::*;

use crate::fs::StatusFlags;
use crate::io_uring::ENABLE_URING;
use crate::prelude::*;

// 4096 is default max socket connection value in Ubuntu 20.04
const SOMAXCONN: u32 = 4096;
const SOCONN_DEFAULT: u32 = 16;

pub fn do_socket(domain: c_int, socket_type: c_int, protocol: c_int) -> Result<isize> {
    let domain = Domain::try_from(domain as u16)?;
    let flags = SocketFlags::from_bits_truncate(socket_type);

    let type_bits = socket_type & !flags.bits();
    let socket_type =
        SocketType::try_from(type_bits).map_err(|_| errno!(EINVAL, "invalid socket type"))?;

    debug!(
        "socket domain: {:?}, type: {:?}, protocol: {:?}",
        domain, socket_type, protocol
    );

    let mut file_ref: Option<Arc<dyn File>> = None;

    // Only support INET and INET6 domain with uring feature
    if ENABLE_URING.load(Ordering::Relaxed) && (domain == Domain::INET || domain == Domain::INET6) {
        let protocol = SocketProtocol::try_from(protocol)
            .map_err(|_| errno!(EINVAL, "Invalid or unsupported network protocol"))?;

        // Determine if type and domain match uring supported socket
        let match_uring = move || {
            let is_uring_type =
                (socket_type == SocketType::DGRAM || socket_type == SocketType::STREAM);
            let is_uring_protocol = (protocol == SocketProtocol::IPPROTO_IP
                || protocol == SocketProtocol::IPPROTO_TCP
                || protocol == SocketProtocol::IPPROTO_UDP);

            is_uring_type && is_uring_protocol
        };

        if match_uring() {
            let nonblocking = flags.contains(SocketFlags::SOCK_NONBLOCK);
            let socket_file = SocketFile::new(domain, protocol, socket_type, nonblocking)?;
            file_ref = Some(Arc::new(socket_file));
        }
    };

    // Dispatch unsupported uring domain and flags to ocall
    if file_ref.is_none() {
        match domain {
            Domain::LOCAL => {
                let unix_socket = unix_socket(socket_type, flags, protocol)?;
                file_ref = Some(Arc::new(unix_socket));
            }
            _ => {
                let socket = HostSocket::new(domain, socket_type, flags, protocol)?;
                file_ref = Some(Arc::new(socket));
            }
        }
    };

    let close_on_spawn = flags.contains(SocketFlags::SOCK_CLOEXEC);
    let fd = current!().add_file(file_ref.unwrap(), close_on_spawn);
    Ok(fd as isize)
}

pub fn do_bind(fd: c_int, addr: *const libc::sockaddr, addr_len: libc::socklen_t) -> Result<isize> {
    let addr = {
        let addr_len = addr_len as usize;
        let sockaddr_storage = copy_sock_addr_from_user(addr, addr_len)?;
        let addr = AnyAddr::from_c_storage(&sockaddr_storage, addr_len)?;
        addr
    };

    trace!("bind to addr: {:?}", addr);

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        let raw_addr = addr.to_raw();
        socket.bind(&raw_addr)?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let unix_addr = addr.to_unix()?;
        unix_socket.bind(unix_addr)?;
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket.bind(&addr)?;
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
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        let backlog: u32 = if backlog as u32 > SOMAXCONN {
            SOMAXCONN
        } else if backlog == 0 {
            SOCONN_DEFAULT
        } else {
            backlog as u32
        };
        uring_socket.listen(backlog)?;
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
            Some(unsafe { SockAddr::try_from_raw(addr, addr_len as u32)? })
        } else {
            None
        };

        socket.connect(addr_option.as_ref())?;
        return Ok(0);
    };

    let addr = {
        let addr_len = addr_len as usize;
        let sockaddr_storage = copy_sock_addr_from_user(addr, addr_len)?;
        let addr = AnyAddr::from_c_storage(&sockaddr_storage, addr_len)?;
        addr
    };

    if let Ok(unix_socket) = file_ref.as_unix_socket() {
        // TODO: support AF_UNSPEC address for datagram socket use
        unix_socket.connect(addr.to_unix()?)?;
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket.connect(&addr)?;
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
    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;
    let sock_flags =
        SocketFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let close_on_spawn = sock_flags.contains(SocketFlags::SOCK_CLOEXEC);

    let file_ref = current!().file(fd as FileDesc)?;

    // Accept the socket
    let (new_file_ref, sock_addr_option): (Arc<dyn File>, Option<AnyAddr>) =
        if let Ok(socket) = file_ref.as_host_socket() {
            let (new_socket_file, sock_addr_option) = socket.accept(sock_flags)?;
            (
                Arc::new(new_socket_file),
                sock_addr_option.map(|raw_addr| AnyAddr::Raw(raw_addr)),
            )
        } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
            let (new_socket_file, sock_addr_option) = unix_socket.accept(sock_flags)?;
            (
                Arc::new(new_socket_file),
                sock_addr_option.map(|unix_addr| AnyAddr::Unix(unix_addr)),
            )
        } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
            let nonblocking = sock_flags.contains(SocketFlags::SOCK_NONBLOCK);
            let accepted_socket = uring_socket.accept(nonblocking)?;
            let sock_addr = accepted_socket.peer_addr()?;
            (Arc::new(accepted_socket), Some(sock_addr))
        } else {
            return_errno!(ENOTSOCK, "not a socket");
        };

    let new_fd = current!().add_file(new_file_ref, close_on_spawn);

    // Output the address
    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        if let Some(sock_addr) = sock_addr_option {
            let (src_addr, src_addr_len) = sock_addr.to_c_storage();
            copy_sock_addr_to_user(src_addr, src_addr_len, addr_mut, addr_len_mut);
        } else {
            *addr_len_mut = 0;
        }
    }

    Ok(new_fd as isize)
}

pub fn do_shutdown(fd: c_int, how: c_int) -> Result<isize> {
    debug!("shutdown: fd: {}, how: {}", fd, how);
    let how = Shutdown::from_c(how as _)?;

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(socket) = file_ref.as_host_socket() {
        socket.shutdown(how)?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket.shutdown(how)?;
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket.shutdown(how)?;
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

    let optval = from_user::make_slice(optval as *const u8, optlen as usize)?;

    if let Ok(host_socket) = file_ref.as_host_socket() {
        let mut cmd = new_host_setsockopt_cmd(level, optname, optval)?;
        host_socket.ioctl(cmd.as_mut())?;
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("setsockopt for unix socket is unimplemented");
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        let mut cmd = new_uring_setsockopt_cmd(level, optname, optval, uring_socket.get_type())?;
        uring_socket.ioctl(cmd.as_mut())?;
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
    Ok(0)
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
    let optlen_mut = from_user::make_mut_ref(optlen)?;
    let optlen = *optlen_mut;
    let optval_mut = from_user::make_mut_slice(optval as *mut u8, optlen as usize)?;

    // Man getsockopt:
    // If the size of the option value is greater than option_len, the value stored in the object pointed to by the option_value argument will be silently truncated.
    // Thus if the optlen is 0, nothing is returned to optval. We can just return here.
    if optlen == 0 {
        return Ok(0);
    }

    let file_ref = current!().file(fd as FileDesc)?;

    if let Ok(host_socket) = file_ref.as_host_socket() {
        let mut cmd = new_host_getsockopt_cmd(level, optname, optlen)?;
        host_socket.ioctl(cmd.as_mut())?;
        let src_optval = get_optval(cmd.as_ref())?;
        copy_bytes_to_user(src_optval, optval_mut, optlen_mut);
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        warn!("getsockopt for unix socket is unimplemented");
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        let mut cmd = new_uring_getsockopt_cmd(level, optname, optlen, uring_socket.get_type())?;
        uring_socket.ioctl(cmd.as_mut())?;
        let src_optval = get_optval(cmd.as_ref())?;
        copy_bytes_to_user(src_optval, optval_mut, optlen_mut);
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
    Ok(0)
}

pub fn do_getpeername(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;
    if addr_and_addr_len.is_none() {
        return Ok(0);
    }

    let file_ref = current!().file(fd as FileDesc)?;
    let (src_addr, src_addr_len) = if let Ok(host_socket) = file_ref.as_host_socket() {
        host_socket.peer_addr()?.to_c_storage()
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket.peer_addr()?.to_c_storage()
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket.peer_addr()?.to_c_storage()
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    };

    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        copy_sock_addr_to_user(src_addr, src_addr_len, addr_mut, addr_len_mut);
    }

    Ok(0)
}

pub fn do_getsockname(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;
    if addr_and_addr_len.is_none() {
        return Ok(0);
    }

    let file_ref = current!().file(fd as FileDesc)?;
    let (src_addr, src_addr_len) = if let Ok(host_socket) = file_ref.as_host_socket() {
        host_socket.addr()?.to_c_storage()
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket.addr().to_c_storage()
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket.addr()?.to_c_storage()
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    };

    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        copy_sock_addr_to_user(src_addr, src_addr_len, addr_mut, addr_len_mut);
    }

    Ok(0)
}

pub fn do_sendto(
    fd: c_int,
    base: *const c_void,
    len: size_t,
    flags: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    if addr.is_null() ^ (addr_len == 0) {
        return_errno!(EINVAL, "addr and addr_len should be both null and 0 or not");
    }
    let addr = {
        if addr.is_null() {
            None
        } else {
            let addr_storage = copy_sock_addr_from_user(addr, addr_len as _)?;
            Some(AnyAddr::from_c_storage(&addr_storage, addr_len as _)?)
        }
    };

    from_user::check_array(base as *const u8, len)?;
    let buf = unsafe { std::slice::from_raw_parts(base as *const u8, len as usize) };

    let send_flags = SendFlags::from_bits_truncate(flags);

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(host_socket) = file_ref.as_host_socket() {
        host_socket
            .sendto(buf, send_flags, addr)
            .map(|u| u as isize)
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        let addr = match addr {
            Some(ref any_addr) => Some(any_addr.to_unix()?),
            None => None,
        };

        unix_socket
            .sendto(buf, send_flags, addr)
            .map(|u| u as isize)
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket
            .sendto(&buf, addr, send_flags)
            .map(|bytes_send| bytes_send as isize)
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
    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;

    from_user::check_array(base as *mut u8, len)?;
    let mut buf = unsafe { std::slice::from_raw_parts_mut(base as *mut u8, len as usize) };

    // MSG_CTRUNC is a return flag but linux allows it to be set on input flags.
    // We just ignore it.
    let recv_flags = RecvFlags::from_bits(flags & !(MsgFlags::MSG_CTRUNC.bits()))
        .ok_or_else(|| errno!(EINVAL, "invalid flags"))?;

    let file_ref = current!().file(fd as FileDesc)?;
    let (data_len, addr_recv) = if let Ok(socket) = file_ref.as_host_socket() {
        socket.recvfrom(buf, recv_flags)?
    } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
        unix_socket
            .recvfrom(buf, recv_flags)
            .map(|(len, addr_recv)| (len, addr_recv.map(|unix_addr| AnyAddr::Unix(unix_addr))))?
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket.recvfrom(&mut buf, recv_flags)?
    } else {
        return_errno!(ENOTSOCK, "not a socket");
    };

    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        if let Some(addr_recv) = addr_recv {
            let (c_addr_storage, c_addr_len) = addr_recv.to_c_storage();
            copy_sock_addr_to_user(c_addr_storage, c_addr_len, addr_mut, addr_len_mut);
        } else {
            // If addr_recv is not filled, set addr_len to 0
            *addr_len_mut = 0;
        }
    }
    Ok(data_len as isize)
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

    let file_flags = SocketFlags::from_bits_truncate(socket_type);
    let close_on_spawn = file_flags.contains(SocketFlags::SOCK_CLOEXEC);
    let sock_type = SocketType::try_from(socket_type & (!file_flags.bits()))
        .map_err(|_| errno!(EINVAL, "invalid socket type"))?;

    let domain = Domain::try_from(domain as u16)?;
    if (domain == Domain::LOCAL) {
        let (client_socket, server_socket) = socketpair(sock_type, file_flags, protocol)?;

        let current = current!();
        let mut files = current.files().lock();
        sock_pair[0] = files.put(Arc::new(client_socket), close_on_spawn);
        sock_pair[1] = files.put(Arc::new(server_socket), close_on_spawn);

        debug!("socketpair: ({}, {})", sock_pair[0], sock_pair[1]);
        Ok(0)
    } else {
        return_errno!(EAFNOSUPPORT, "domain not supported")
    }
}

pub fn do_sendmsg(fd: c_int, msg_ptr: *const libc::msghdr, flags_c: c_int) -> Result<isize> {
    debug!(
        "sendmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_ptr, flags_c
    );

    let (addr, bufs, control) = extract_msghdr_from_user(msg_ptr)?;
    let flags = SendFlags::from_bits_truncate(flags_c);

    let file_ref = current!().file(fd as FileDesc)?;
    if let Ok(host_socket) = file_ref.as_host_socket() {
        host_socket
            .sendmsg(&bufs[..], flags, addr, control)
            .map(|bytes_send| bytes_send as isize)
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        socket
            .sendmsg(&bufs[..], flags, control)
            .map(|bytes_sent| bytes_sent as isize)
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        uring_socket
            .sendmsg(&bufs[..], addr, flags, control)
            .map(|bytes_send| bytes_send as isize)
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }
}

pub fn do_recvmsg(fd: c_int, msg_mut_ptr: *mut libc::msghdr, flags_c: c_int) -> Result<isize> {
    debug!(
        "recvmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_mut_ptr, flags_c
    );
    let (mut msg, mut addr, mut control, mut bufs) = extract_msghdr_mut_from_user(msg_mut_ptr)?;
    let flags = RecvFlags::from_bits_truncate(flags_c);

    let file_ref = current!().file(fd as FileDesc)?;
    let (bytes_recv, recv_addr, msg_flags, msg_controllen) =
        if let Ok(host_socket) = file_ref.as_host_socket() {
            host_socket.recvmsg(&mut bufs[..], flags, control)?
        } else if let Ok(unix_socket) = file_ref.as_unix_socket() {
            unix_socket.recvmsg(&mut bufs[..], flags, control)?
        } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
            uring_socket.recvmsg(&mut bufs[..], flags, control)?
        } else {
            return_errno!(ENOTSOCK, "not a socket")
        };

    if let Some(addr) = addr {
        if let Some(recv_addr) = recv_addr {
            let (c_addr_storage, c_addr_len) = recv_addr.to_c_storage();
            copy_sock_addr_to_user(c_addr_storage, c_addr_len, addr, &mut msg.msg_namelen);
        }
    }

    msg.msg_flags = msg_flags.bits();
    msg.msg_controllen = msg_controllen;
    if msg_controllen == 0 {
        msg.msg_control = ptr::null_mut();
    }

    Ok(bytes_recv as isize)
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
    let mut send_count = 0;

    if let Ok(host_socket) = file_ref.as_host_socket() {
        for mmsg in (msgvec) {
            let (addr, bufs, control) = extract_msghdr_from_user(&mmsg.msg_hdr)?;

            if host_socket
                .sendmsg(&bufs[..], flags, addr, control)
                .map(|bytes_send| {
                    mmsg.msg_len = bytes_send as c_uint;
                    bytes_send as isize
                })
                .is_ok()
            {
                send_count += 1;
            } else {
                break;
            }
        }
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        return_errno!(EOPNOTSUPP, "does not support unix socket")
    } else if let Ok(uring_socket) = file_ref.as_uring_socket() {
        for mmsg in (msgvec) {
            let (addr, bufs, control) = extract_msghdr_from_user(&mmsg.msg_hdr)?;

            if uring_socket
                .sendmsg(&bufs[..], addr, flags, control)
                .map(|bytes_send| {
                    mmsg.msg_len = bytes_send as c_uint;
                    bytes_send as isize
                })
                .is_ok()
            {
                send_count += 1;
            } else {
                break;
            }
        }
    } else {
        return_errno!(ENOTSOCK, "not a socket")
    }

    Ok(send_count as isize)
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

#[derive(Debug)]
#[repr(C)]
pub struct sigset_argpack {
    ss: *const sigset_t,
    ss_len: size_t,
}

pub fn do_pselect6(
    nfds: c_int,
    readfds: *mut libc::fd_set,
    writefds: *mut libc::fd_set,
    exceptfds: *mut libc::fd_set,
    timeout: *mut timespec_t,
    sig_data: *const sigset_argpack,
) -> Result<isize> {
    let mut is_set_sig = false;
    let mut prev_mask = SigSet::default();
    let thread = current!();

    // Set signal mask
    if !sig_data.is_null() {
        from_user::check_ptr(sig_data)?;
        let user_sig_data = unsafe { &*(sig_data) };

        is_set_sig = !user_sig_data.ss.is_null();
        if is_set_sig {
            if user_sig_data.ss_len != std::mem::size_of::<sigset_t>() {
                return_errno!(EINVAL, "unexpected sigset size");
            }
            let update_mask = {
                let sigset = user_sig_data.ss;
                from_user::check_ptr(sigset)?;
                let set = unsafe { &*sigset };
                let mut set = SigSet::from_c(unsafe { *sigset });
                // According to man pages, "it is not possible to block SIGKILL or SIGSTOP.
                // Attempts to do so are silently ignored."
                set -= SIGKILL;
                set -= SIGSTOP;
                set
            };
            let mut curr_mask = thread.sig_mask().write().unwrap();
            prev_mask = *curr_mask;
            *curr_mask = update_mask;
        }
    }

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

    // Restore the original signal mask
    if is_set_sig {
        let mut curr_mask = thread.sig_mask().write().unwrap();
        *curr_mask = prev_mask;
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

fn copy_sock_addr_from_user(
    addr: *const libc::sockaddr,
    addr_len: usize,
) -> Result<libc::sockaddr_storage> {
    // Check the address pointer and length
    if addr.is_null() || addr_len == 0 {
        return_errno!(EINVAL, "no address is specified");
    }
    if addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
        return_errno!(
            EINVAL,
            "addr len cannot be greater than sockaddr_storage's size"
        );
    }
    let sockaddr_src_buf = from_user::make_slice(addr as *const u8, addr_len)?;

    let sockaddr_storage = {
        let mut sockaddr_storage = MaybeUninit::<libc::sockaddr_storage>::uninit();
        // Safety. The dst slice is the only mutable reference to the sockaddr_storage
        let sockaddr_dst_buf = unsafe {
            let ptr = sockaddr_storage.as_mut_ptr() as *mut u8;
            let len = addr_len;
            std::slice::from_raw_parts_mut(ptr, len)
        };
        sockaddr_dst_buf.copy_from_slice(sockaddr_src_buf);
        unsafe { sockaddr_storage.assume_init() }
    };

    Ok(sockaddr_storage)
}

fn get_slice_from_sock_addr_ptr_mut<'a>(
    addr_ptr: *mut libc::sockaddr,
    addr_len_ptr: *mut libc::socklen_t,
) -> Result<Option<(&'a mut [u8], &'a mut u32)>> {
    if addr_ptr.is_null() ^ addr_len_ptr.is_null() {
        return_errno!(EINVAL, "addr and addr_len should be both null or not null");
    }
    if addr_ptr.is_null() {
        return Ok(None);
    }

    let addr_len_mut = from_user::make_mut_ref(addr_len_ptr)?;
    let addr_len = *addr_len_mut;
    let addr_mut = from_user::make_mut_slice(addr_ptr as *mut u8, addr_len as usize)?;
    Ok(Some((addr_mut, addr_len_mut)))
}

fn copy_sock_addr_to_user(
    src_addr: libc::sockaddr_storage,
    src_addr_len: usize,
    dst_addr: &mut [u8],
    dst_addr_len: &mut u32,
) {
    let len = std::cmp::min(src_addr_len, *dst_addr_len as usize);
    let sockaddr_src_buf = unsafe {
        let ptr = &src_addr as *const _ as *const u8;
        std::slice::from_raw_parts(ptr, len)
    };
    dst_addr[..len].copy_from_slice(sockaddr_src_buf);
    *dst_addr_len = src_addr_len as u32;
}

/// Create a new ioctl command for host socket getsockopt syscall
fn new_host_getsockopt_cmd(level: i32, optname: i32, optlen: u32) -> Result<Box<dyn IoctlCmd>> {
    if level != libc::SOL_SOCKET {
        return Ok(Box::new(GetSockOptRawCmd::new(level, optname, optlen)));
    }

    let opt =
        SockOptName::try_from(optname).map_err(|_| errno!(ENOPROTOOPT, "Not a valid optname"))?;

    Ok(match opt {
        SockOptName::SO_CNX_ADVICE => return_errno!(ENOPROTOOPT, "it's a write-only option"),
        _ => Box::new(GetSockOptRawCmd::new(level, optname, optlen)),
    })
}

/// Create a new ioctl command for uring socket getsockopt syscall
fn new_uring_getsockopt_cmd(
    level: i32,
    optname: i32,
    optlen: u32,
    socket_type: SocketType,
) -> Result<Box<dyn IoctlCmd>> {
    if level != libc::SOL_SOCKET {
        return Ok(Box::new(GetSockOptRawCmd::new(level, optname, optlen)));
    }

    let opt =
        SockOptName::try_from(optname).map_err(|_| errno!(ENOPROTOOPT, "Not a valid optname"))?;

    Ok(match opt {
        SockOptName::SO_ACCEPTCONN => Box::new(GetAcceptConnCmd::new(())),
        SockOptName::SO_DOMAIN => Box::new(GetDomainCmd::new(())),
        SockOptName::SO_ERROR => Box::new(GetErrorCmd::new(())),
        SockOptName::SO_PEERNAME => Box::new(GetPeerNameCmd::new(())),
        SockOptName::SO_TYPE => Box::new(GetTypeCmd::new(())),
        SockOptName::SO_RCVTIMEO_OLD => Box::new(GetRecvTimeoutCmd::new(())),
        SockOptName::SO_SNDTIMEO_OLD => Box::new(GetSendTimeoutCmd::new(())),
        SockOptName::SO_SNDBUF => Box::new(GetSockOptRawCmd::new(level, optname, optlen)),
        SockOptName::SO_RCVBUF => Box::new(GetSockOptRawCmd::new(level, optname, optlen)),

        SockOptName::SO_CNX_ADVICE => return_errno!(ENOPROTOOPT, "it's a write-only option"),
        _ => Box::new(GetSockOptRawCmd::new(level, optname, optlen)),
    })
}

/// Create a new ioctl command for host socket setsockopt syscall
fn new_host_setsockopt_cmd(
    level: i32,
    optname: i32,
    optval: &'static [u8],
) -> Result<Box<dyn IoctlCmd>> {
    if level != libc::SOL_SOCKET {
        return Ok(Box::new(SetSockOptRawCmd::new(level, optname, optval)));
    }

    let opt =
        SockOptName::try_from(optname).map_err(|_| errno!(ENOPROTOOPT, "Not a valid optname"))?;

    Ok(match opt {
        SockOptName::SO_ACCEPTCONN
        | SockOptName::SO_DOMAIN
        | SockOptName::SO_PEERNAME
        | SockOptName::SO_TYPE
        | SockOptName::SO_ERROR
        | SockOptName::SO_PEERCRED
        | SockOptName::SO_SNDLOWAT
        | SockOptName::SO_PEERSEC
        | SockOptName::SO_PROTOCOL
        | SockOptName::SO_MEMINFO
        | SockOptName::SO_INCOMING_NAPI_ID
        | SockOptName::SO_COOKIE
        | SockOptName::SO_PEERGROUPS => return_errno!(ENOPROTOOPT, "it's a read-only option"),
        _ => Box::new(SetSockOptRawCmd::new(level, optname, optval)),
    })
}

/// Create a new ioctl command for uring socket setsockopt syscall
fn new_uring_setsockopt_cmd(
    level: i32,
    optname: i32,
    optval: &'static [u8],
    socket_type: SocketType,
) -> Result<Box<dyn IoctlCmd>> {
    if level != libc::SOL_SOCKET {
        return Ok(Box::new(SetSockOptRawCmd::new(level, optname, optval)));
    }

    if optval.len() == 0 {
        return_errno!(EINVAL, "Not a valid optval length");
    }

    let opt =
        SockOptName::try_from(optname).map_err(|_| errno!(ENOPROTOOPT, "Not a valid optname"))?;

    Ok(match opt {
        SockOptName::SO_ACCEPTCONN
        | SockOptName::SO_DOMAIN
        | SockOptName::SO_PEERNAME
        | SockOptName::SO_TYPE
        | SockOptName::SO_ERROR
        | SockOptName::SO_PEERCRED
        | SockOptName::SO_SNDLOWAT
        | SockOptName::SO_PEERSEC
        | SockOptName::SO_PROTOCOL
        | SockOptName::SO_MEMINFO
        | SockOptName::SO_INCOMING_NAPI_ID
        | SockOptName::SO_COOKIE
        | SockOptName::SO_PEERGROUPS => return_errno!(ENOPROTOOPT, "it's a read-only option"),
        SockOptName::SO_RCVTIMEO_OLD => {
            let mut timeout: *const libc::timeval = std::ptr::null();
            if optval.len() >= std::mem::size_of::<libc::timeval>() {
                timeout = optval as *const _ as *const libc::timeval;
            } else {
                return_errno!(EINVAL, "invalid timeout option");
            }
            let timeout = unsafe {
                let secs = if (*timeout).tv_sec < 0 {
                    0
                } else {
                    (*timeout).tv_sec
                };

                let usec = (*timeout).tv_usec;
                if usec < 0 || usec > 1000000 || (usec as u32).checked_mul(1000).is_none() {
                    return_errno!(EDOM, "time struct value is invalid");
                }
                Duration::new(secs as u64, (*timeout).tv_usec as u32 * 1000)
            };
            trace!("recv timeout = {:?}", timeout);
            Box::new(SetRecvTimeoutCmd::new(timeout))
        }
        SockOptName::SO_SNDTIMEO_OLD => {
            let mut timeout: *const libc::timeval = std::ptr::null();
            if optval.len() >= std::mem::size_of::<libc::timeval>() {
                timeout = optval as *const _ as *const libc::timeval;
            } else {
                return_errno!(EINVAL, "invalid timeout option");
            }
            let timeout = unsafe {
                let secs = if (*timeout).tv_sec < 0 {
                    0
                } else {
                    (*timeout).tv_sec
                };

                let usec = (*timeout).tv_usec;
                if usec < 0 || usec > 1000000 || (usec as u32).checked_mul(1000).is_none() {
                    return_errno!(EDOM, "time struct value is invalid");
                }
                Duration::new(secs as u64, usec as u32 * 1000)
            };
            trace!("send timeout = {:?}", timeout);
            Box::new(SetSendTimeoutCmd::new(timeout))
        }
        SockOptName::SO_SNDBUF => {
            // Implement dynamic buf size for stream socket only.
            if socket_type != SocketType::STREAM {
                Box::new(SetSockOptRawCmd::new(level, optname, optval))
            } else {
                // For the max value, we choose 4MB (doubled) to assure the libos kernel buf won't be the bottleneck.
                let max_size = 2 * 1024 * 1024;

                if optval.len() > 8 {
                    return_errno!(EINVAL, "optval size is invalid");
                }

                let mut send_buf_size = {
                    let mut size = [0 as u8; std::mem::size_of::<usize>()];
                    let start_offset = size.len() - optval.len();
                    size[start_offset..].copy_from_slice(optval);
                    usize::from_ne_bytes(size)
                };
                trace!("set SO_SNDBUF size = {:?}", send_buf_size);
                if send_buf_size > max_size {
                    send_buf_size = max_size;
                }
                // Based on man page: The kernel doubles this value (to allow space for bookkeeping overhead)
                // when it is set using setsockopt(2), and this doubled value is returned by getsockopt(2).
                send_buf_size *= 2;
                Box::new(SetSendBufSizeCmd::new(send_buf_size))
            }
        }
        SockOptName::SO_RCVBUF => {
            if socket_type != SocketType::STREAM {
                Box::new(SetSockOptRawCmd::new(level, optname, optval))
            } else {
                // Implement dynamic buf size for stream socket only.
                info!("optval = {:?}", optval);
                // For the max value, we choose 4MB (doubled) to assure the libos kernel buf won't be the bottleneck.
                let max_size = 2 * 1024 * 1024;

                if optval.len() > 8 {
                    return_errno!(EINVAL, "optval size is invalid");
                }

                let mut recv_buf_size = {
                    let mut size = [0 as u8; std::mem::size_of::<usize>()];
                    let start_offset = size.len() - optval.len();
                    size[start_offset..].copy_from_slice(optval);
                    usize::from_ne_bytes(size)
                };
                trace!("set SO_RCVBUF size = {:?}", recv_buf_size);
                if recv_buf_size > max_size {
                    recv_buf_size = max_size
                }
                // Based on man page: The kernel doubles this value (to allow space for bookkeeping overhead)
                // when it is set using setsockopt(2), and this doubled value is returned by getsockopt(2).
                recv_buf_size *= 2;
                Box::new(SetRecvBufSizeCmd::new(recv_buf_size))
            }
        }
        _ => Box::new(SetSockOptRawCmd::new(level, optname, optval)),
    })
}

fn get_optval(cmd: &dyn IoctlCmd) -> Result<&[u8]> {
    crate::match_ioctl_cmd_ref!(cmd, {
        cmd : GetAcceptConnCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetDomainCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetPeerNameCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetTypeCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetSockOptRawCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetErrorCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetRecvTimeoutCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetSendTimeoutCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetSendBufSizeCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetRecvBufSizeCmd => {
            cmd.get_output_as_bytes()
        },
        _ => {
            return_errno!(EINVAL, "invalid sockopt command");
        }
    })
    .ok_or_else(|| errno!(EINVAL, "no available output"))
}

fn copy_bytes_to_user(src_buf: &[u8], dst_buf: &mut [u8], dst_len: &mut u32) {
    let copy_len = dst_buf.len().min(src_buf.len());
    dst_buf[..copy_len].copy_from_slice(&src_buf[..copy_len]);
    *dst_len = copy_len as _;
}

fn extract_msghdr_from_user<'a>(
    msg_ptr: *const libc::msghdr,
) -> Result<(Option<AnyAddr>, Vec<&'a [u8]>, Option<&'a [u8]>)> {
    let msg = from_user::make_ref(msg_ptr)?;

    let msg_name = msg.msg_name;
    let msg_namelen = msg.msg_namelen;
    if msg_name.is_null() ^ (msg_namelen == 0) {
        return_errno!(EINVAL, "name and namelen should be both null and 0 or not");
    }
    let name = if msg_name.is_null() {
        None
    } else {
        let sockaddr_storage = copy_sock_addr_from_user(msg_name as *const _, msg_namelen as _)?;
        Some(AnyAddr::from_c_storage(
            &sockaddr_storage,
            msg_namelen as _,
        )?)
    };

    let msg_control = msg.msg_control;
    let msg_controllen = msg.msg_controllen;

    if msg_control.is_null() ^ (msg_controllen == 0) {
        return_errno!(
            EINVAL,
            "message control and controllen should be both null and 0 or not"
        );
    }

    let control = if msg_control.is_null() {
        None
    } else {
        Some(from_user::make_slice(
            msg_control as *const u8,
            msg_controllen as _,
        )?)
    };

    let msg_iov = msg.msg_iov;
    let msg_iovlen = msg.msg_iovlen;
    if msg_iov.is_null() ^ (msg_iovlen == 0) {
        return_errno!(EINVAL, "iov and iovlen should be both null and 0 or not");
    }
    let bufs = if msg_iov.is_null() {
        Vec::new()
    } else {
        let iovs = from_user::make_slice(msg_iov, msg_iovlen)?;
        let mut bufs = Vec::with_capacity(msg_iovlen);
        for iov in iovs {
            let buf = from_user::make_slice(iov.iov_base as *const u8, iov.iov_len)?;
            bufs.push(buf);
        }
        bufs
    };

    Ok((name, bufs, control))
}

fn extract_msghdr_mut_from_user<'a>(
    msg_mut_ptr: *mut libc::msghdr,
) -> Result<(
    &'a mut libc::msghdr,
    Option<&'a mut [u8]>,
    Option<&'a mut [u8]>,
    Vec<&'a mut [u8]>,
)> {
    let msg_mut = from_user::make_mut_ref(msg_mut_ptr)?;

    let msg_name = msg_mut.msg_name;
    let msg_namelen = msg_mut.msg_namelen;
    if msg_name.is_null() ^ (msg_namelen == 0) {
        return_errno!(EINVAL, "name and namelen should be both null and 0 or not");
    }
    let name = if msg_name.is_null() {
        None
    } else {
        Some(from_user::make_mut_slice(
            msg_name as *mut u8,
            msg_namelen as usize,
        )?)
    };

    let msg_control = msg_mut.msg_control;
    let msg_controllen = msg_mut.msg_controllen;

    if msg_control.is_null() ^ (msg_controllen == 0) {
        return_errno!(
            EINVAL,
            "message control and controllen should be both null and 0 or not"
        );
    }

    let control = if msg_control.is_null() {
        None
    } else {
        Some(from_user::make_mut_slice(
            msg_control as *mut u8,
            msg_controllen as usize,
        )?)
    };

    let msg_iov = msg_mut.msg_iov;
    let msg_iovlen = msg_mut.msg_iovlen;
    if msg_iov.is_null() ^ (msg_iovlen == 0) {
        return_errno!(EINVAL, "iov and iovlen should be both null and 0 or not");
    }
    let bufs = if msg_iov.is_null() {
        Vec::new()
    } else {
        let iovs = from_user::make_mut_slice(msg_iov, msg_iovlen)?;
        let mut bufs = Vec::with_capacity(msg_iovlen);
        for iov in iovs {
            // In some situation using MSG_ERRQUEUE, users just require control buffers,
            // they may left iovec buffer all zero. It works in Linux.
            if iov.iov_base.is_null() {
                break;
            }
            let buf = from_user::make_mut_slice(iov.iov_base as *mut u8, iov.iov_len)?;
            bufs.push(buf);
        }
        bufs
    };

    Ok((msg_mut, name, control, bufs))
}

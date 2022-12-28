use std::convert::TryFrom;
use std::mem::MaybeUninit;
use std::ptr;
use std::time::Duration;

use async_io::ioctl::IoctlCmd;
use async_io::socket::{
    GetRecvTimeoutCmd, GetSendTimeoutCmd, SetRecvTimeoutCmd, SetSendTimeoutCmd,
};
use async_io::socket::{RecvFlags, SendFlags, Shutdown, Type};
use async_socket::sockopt::{
    GetAcceptConnCmd, GetDomainCmd, GetPeerNameCmd, GetSockOptRawCmd, GetTypeCmd, SetSockOptRawCmd,
    SockOptName,
};
use num_enum::TryFromPrimitive;

use super::*;
use crate::fs::StatusFlags;
use crate::prelude::*;
use crate::util::mem_util::from_user;

// 4096 is default max socket connection value in Ubuntu 20.04
const SOMAXCONN: u32 = 4096;
const SOCONN_DEFAULT: u32 = 16;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct mmsghdr {
    pub msg_hdr: libc::msghdr,
    pub msg_len: c_uint,
}

pub async fn do_socket(domain: c_int, type_and_flags: c_int, protocol: c_int) -> Result<isize> {
    // Check arguments
    let domain = Domain::try_from(domain)
        .map_err(|_| errno!(EINVAL, "invalid or unsupported network domain"))?;
    let flags = SocketFlags::from_bits_truncate(type_and_flags);
    let type_bits = type_and_flags & !flags.bits();
    let socket_type =
        Type::try_from(type_bits).map_err(|_| errno!(EINVAL, "invalid socket type"))?;

    trace!(
        "create new socket. domain: {:?}, flags: {:?}, protocol: {:?}",
        domain,
        flags,
        protocol
    );

    // Create the socket
    let nonblocking = flags.contains(SocketFlags::SOCK_NONBLOCK);
    let socket_file = SocketFile::new(domain, protocol, socket_type, nonblocking)?;
    let file_ref = FileRef::new_socket(socket_file);

    let close_on_spawn = flags.contains(SocketFlags::SOCK_CLOEXEC);
    let fd = current!().add_file(file_ref, close_on_spawn);
    Ok(fd as isize)
}

pub async fn do_socketpair(
    domain: c_int,
    type_and_flags: c_int,
    protocol: c_int,
    sv: *mut c_int,
) -> Result<isize> {
    let domain = Domain::try_from(domain)
        .map_err(|_| errno!(EAFNOSUPPORT, "invalid or unsupported network domain"))?;
    if domain != Domain::Unix {
        return_errno!(EAFNOSUPPORT, "unsupported network domain");
    }
    let flags = SocketFlags::from_bits_truncate(type_and_flags);
    let is_stream = {
        let type_bits = type_and_flags & !flags.bits();
        let type_ = Type::try_from(type_bits).map_err(|_| errno!(EINVAL, "invalid socket type"))?;
        match type_ {
            Type::STREAM => true,
            Type::DGRAM => false,
            _ => return_errno!(EINVAL, "invalid type"),
        }
    };
    let _protocol = {
        // Only the default protocol is supported for now
        if protocol != 0 {
            return_errno!(EOPNOTSUPP, "invalid protocol");
        }
        protocol
    };

    let mut sock_pair = from_user::make_mut_slice(sv as *mut u32, 2)?;

    let nonblocking = flags.contains(SocketFlags::SOCK_NONBLOCK);
    let (socket_file1, socket_file2) = SocketFile::new_pair(is_stream, nonblocking)?;

    let file_ref1 = FileRef::new_socket(socket_file1);
    let file_ref2 = FileRef::new_socket(socket_file2);
    let close_on_spawn = flags.contains(SocketFlags::SOCK_CLOEXEC);
    sock_pair[0] = current!().add_file(file_ref1, close_on_spawn);
    sock_pair[1] = current!().add_file(file_ref2, close_on_spawn);
    debug!("socketpair: ({}, {})", sock_pair[0], sock_pair[1]);
    Ok(0)
}

pub async fn do_bind(
    fd: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let mut addr = {
        let addr_len = addr_len as usize;
        let sockaddr_storage = copy_sock_addr_from_user(addr, addr_len)?;
        let addr = AnyAddr::from_c_storage(&sockaddr_storage, addr_len)?;
        addr
    };

    socket_file.bind(&mut addr).await?;
    Ok(0)
}

pub async fn do_listen(fd: c_int, backlog: c_int) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let backlog: u32 = if backlog as u32 > SOMAXCONN {
        SOMAXCONN
    } else if backlog == 0 {
        SOCONN_DEFAULT
    } else {
        backlog as u32
    };

    socket_file.listen(backlog)?;
    Ok(0)
}

pub async fn do_connect(
    fd: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let addr = {
        let addr_len = addr_len as usize;
        let sockaddr_storage = copy_sock_addr_from_user(addr, addr_len)?;
        AnyAddr::from_c_storage(&sockaddr_storage, addr_len)?
    };

    socket_file.connect(&addr).await?;
    Ok(0)
}

pub async fn do_accept(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    do_accept4(fd, addr, addr_len, 0).await
}

pub async fn do_accept4(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
    flags: c_int,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    // Process other input arguments
    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;
    let flags = SocketFlags::from_bits_truncate(flags);

    // Do accept
    let nonblocking = flags.contains(SocketFlags::SOCK_NONBLOCK);
    let accepted_socket = socket_file.accept(nonblocking).await?;

    // Output the address
    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        let (src_addr, src_addr_len) = accepted_socket.peer_addr()?.to_c_storage();
        copy_sock_addr_to_user(src_addr, src_addr_len, addr_mut, addr_len_mut);
    }
    // Update the file table
    let new_fd = {
        let new_file_ref = FileRef::new_socket(accepted_socket);
        let close_on_spawn = flags.contains(SocketFlags::SOCK_CLOEXEC);
        current!().add_file(new_file_ref, close_on_spawn)
    };
    Ok(new_fd as isize)
}

pub async fn do_sendto(
    fd: c_int,
    base: *const c_void,
    len: size_t,
    flags: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let buf = from_user::make_slice(base as *const u8, len as usize)?;
    let flags = SendFlags::from_bits_truncate(flags);

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

    socket_file
        .sendto(&buf, addr, flags)
        .await
        .map(|bytes_send| bytes_send as isize)
}

pub async fn do_recvfrom(
    fd: c_int,
    base: *mut c_void,
    len: size_t,
    flags: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let mut buf = from_user::make_mut_slice(base as *mut u8, len as usize)?;
    let flags = RecvFlags::from_bits_truncate(flags);
    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;

    let (bytes_recv, addr_recv) = socket_file.recvfrom(&mut buf, flags).await?;

    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        if let Some(addr_recv) = addr_recv {
            let (c_addr_storage, c_addr_len) = addr_recv.to_c_storage();
            copy_sock_addr_to_user(c_addr_storage, c_addr_len, addr_mut, addr_len_mut);
        } else {
            // If addr_recv is not filled, set addr_len to 0
            *addr_len_mut = 0;
        }
    }
    Ok(bytes_recv as _)
}

pub async fn do_sendmsg(fd: c_int, msg_ptr: *const libc::msghdr, flags: c_int) -> Result<isize> {
    debug!(
        "sendmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_ptr, flags
    );

    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let (addr, bufs, control) = extract_msghdr_from_user(msg_ptr)?;
    let flags = SendFlags::from_bits_truncate(flags);

    socket_file
        .sendmsg(&bufs[..], addr, flags, control)
        .await
        .map(|bytes_send| bytes_send as isize)
}

pub async fn do_recvmsg(fd: c_int, msg_mut_ptr: *mut libc::msghdr, flags: c_int) -> Result<isize> {
    debug!(
        "recvmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_mut_ptr, flags
    );

    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let (mut msg, mut addr, mut control, mut bufs) = extract_msghdr_mut_from_user(msg_mut_ptr)?;
    let flags = RecvFlags::from_bits_truncate(flags);

    let (bytes_recv, recv_addr, msg_flags, msg_controllen) =
        socket_file.recvmsg(&mut bufs[..], flags, control).await?;

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

pub async fn do_sendmmsg(
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
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    let mut send_count = 0;
    for mmsg in (msgvec) {
        let (addr, bufs, control) = extract_msghdr_from_user(&mmsg.msg_hdr)?;

        if socket_file
            .sendmsg(&bufs[..], addr, flags, control)
            .await
            .map(|bytes_send| {
                mmsg.msg_len += bytes_send as c_uint;
                bytes_send as isize
            })
            .is_ok()
        {
            send_count += 1;
        } else {
            break;
        }
    }

    Ok(send_count as isize)
}

pub async fn do_getpeername(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;

    let (src_addr, src_addr_len) = socket_file.peer_addr()?.to_c_storage();
    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        copy_sock_addr_to_user(src_addr, src_addr_len, addr_mut, addr_len_mut);
    }
    Ok(0)
}

pub async fn do_getsockname(
    fd: c_int,
    addr: *mut libc::sockaddr,
    addr_len: *mut libc::socklen_t,
) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    let addr_and_addr_len = get_slice_from_sock_addr_ptr_mut(addr, addr_len)?;

    let (src_addr, src_addr_len) = socket_file.addr()?.to_c_storage();
    if let Some((addr_mut, addr_len_mut)) = addr_and_addr_len {
        copy_sock_addr_to_user(src_addr, src_addr_len, addr_mut, addr_len_mut);
    }
    Ok(0)
}

pub async fn do_getsockopt(
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
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    let optlen_mut = from_user::make_mut_ref(optlen)?;
    let optlen = *optlen_mut;
    let optval_mut = from_user::make_mut_slice(optval as *mut u8, optlen as usize)?;

    // Man getsockopt:
    // If the size of the option value is greater than option_len, the value stored in the object pointed to by the option_value argument will be silently truncated.
    // Thus if the optlen is 0, nothing is returned to optval. We can just return here.
    if optlen == 0 {
        return Ok(0);
    }

    let mut cmd = new_getsockopt_cmd(level, optname, optlen)?;
    socket_file.ioctl(cmd.as_mut()).await?;
    let src_optval = get_optval(cmd.as_ref())?;
    copy_bytes_to_user(src_optval, optval_mut, optlen_mut);
    Ok(0)
}

pub async fn do_setsockopt(
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

    if optval as usize != 0 && optlen == 0 {
        return_errno!(EINVAL, "the optlen size is 0");
    }

    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;

    let optval = from_user::make_slice(optval as *const u8, optlen as usize)?;

    let mut cmd = new_setsockopt_cmd(level, optname, optval)?;
    socket_file.ioctl(cmd.as_mut()).await?;
    Ok(0)
}

pub async fn do_shutdown(fd: c_int, how: c_int) -> Result<isize> {
    debug!("shutdown: fd: {}, how: {}", fd, how);

    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    let how = Shutdown::from_c(how as _)?;

    socket_file.shutdown(how).await?;
    Ok(0)
}

// Flags to use when creating a new socket
bitflags! {
    struct SocketFlags: i32 {
        const SOCK_NONBLOCK = 0x800;
        const SOCK_CLOEXEC  = 0x80000;
    }
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
        // Safety. The content will be initialized before function returns.
        let mut sockaddr_storage =
            unsafe { MaybeUninit::<libc::sockaddr_storage>::uninit().assume_init() };
        // Safety. The dst slice is the only mutable reference to the sockaddr_storage
        let sockaddr_dst_buf = unsafe {
            let ptr = &mut sockaddr_storage as *mut _ as *mut u8;
            let len = addr_len;
            std::slice::from_raw_parts_mut(ptr, len)
        };
        sockaddr_dst_buf.copy_from_slice(sockaddr_src_buf);
        sockaddr_storage
    };
    Ok(sockaddr_storage)
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

/// Create a new ioctl command for getsockopt syscall
fn new_getsockopt_cmd(level: i32, optname: i32, optlen: u32) -> Result<Box<dyn IoctlCmd>> {
    if level != libc::SOL_SOCKET {
        return Ok(Box::new(GetSockOptRawCmd::new(level, optname, optlen)));
    }

    let opt =
        SockOptName::try_from(optname).map_err(|_| errno!(ENOPROTOOPT, "Not a valid optname"))?;
    Ok(match opt {
        SockOptName::SO_ACCEPTCONN => Box::new(GetAcceptConnCmd::new(())),
        SockOptName::SO_DOMAIN => Box::new(GetDomainCmd::new(())),
        SockOptName::SO_PEERNAME => Box::new(GetPeerNameCmd::new(())),
        SockOptName::SO_TYPE => Box::new(GetTypeCmd::new(())),
        SockOptName::SO_RCVTIMEO_OLD => Box::new(GetRecvTimeoutCmd::new(())),
        SockOptName::SO_SNDTIMEO_OLD => Box::new(GetSendTimeoutCmd::new(())),
        SockOptName::SO_CNX_ADVICE => return_errno!(ENOPROTOOPT, "it's a write-only option"),
        _ => Box::new(GetSockOptRawCmd::new(level, optname, optlen)),
    })
}

/// Create a new ioctl command for setsockopt syscall
fn new_setsockopt_cmd(level: i32, optname: i32, optval: &[u8]) -> Result<Box<dyn IoctlCmd>> {
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
        _ => Box::new(SetSockOptRawCmd::new(level, optname, optval)),
    })
}

fn get_optval(cmd: &dyn IoctlCmd) -> Result<&[u8]> {
    async_io::match_ioctl_cmd_ref!(cmd, {
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
        cmd : GetRecvTimeoutCmd => {
            cmd.get_output_as_bytes()
        },
        cmd : GetSendTimeoutCmd => {
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

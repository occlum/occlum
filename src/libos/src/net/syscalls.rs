use std::convert::TryFrom;
use std::mem::MaybeUninit;

use async_io::ioctl::IoctlCmd;
use async_io::socket::Type;
use host_socket::sockopt::{
    GetAcceptConnCmd, GetDomainCmd, GetPeerNameCmd, GetSockOptRawCmd, GetTypeCmd, SetSockOptRawCmd,
    SockOptName,
};
use num_enum::TryFromPrimitive;

use super::*;
use crate::fs::StatusFlags;
use crate::prelude::*;
use crate::util::mem_util::from_user;

pub async fn do_socket(domain: c_int, type_and_flags: c_int, protocol: c_int) -> Result<isize> {
    // Check arguments
    let domain = Domain::try_from(domain)
        .map_err(|_| errno!(EINVAL, "invalid or unsupported network domain"))?;
    let flags = SocketFlags::from_bits_truncate(type_and_flags);
    let is_stream = {
        let type_bits = type_and_flags & !flags.bits();
        let type_ = Type::try_from(type_bits).map_err(|_| errno!(EINVAL, "invalid socket type"))?;
        // Only the two most commonn stream types are supported for now
        match type_ {
            Type::STREAM => true,
            Type::DGRAM => false,
            _ => return_errno!(EINVAL, "invalid type"),
        }
    };
    let _protocol = {
        // Only the default protocol is supported for now
        if protocol != 0 {
            return_errno!(EINVAL, "invalid protocol");
        }
        protocol
    };

    // Create the socket
    let socket_file = SocketFile::new(domain, is_stream)?;
    let file_ref = FileRef::new_socket(socket_file);

    let close_on_spawn = flags.contains(SocketFlags::SOCK_CLOEXEC);
    let fd = current!().add_file(file_ref, close_on_spawn);
    Ok(fd as isize)
}

pub async fn do_bind(
    fd: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    let addr = {
        let addr_len = addr_len as usize;
        let sockaddr_storage = copy_sock_addr_from_user(addr, addr_len)?;
        AnyAddr::from_c_storage(&sockaddr_storage, addr_len as usize)?
    };
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(ENOTSOCK, "not a socket"))?;
    socket_file.bind(&addr)?;
    Ok(0)
}

pub async fn do_listen(fd: c_int, backlog: c_int) -> Result<isize> {
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;
    socket_file.listen(backlog as u32)?;
    Ok(0)
}

pub async fn do_connect(
    fd: c_int,
    addr: *const libc::sockaddr,
    addr_len: libc::socklen_t,
) -> Result<isize> {
    // TODO: allow addr be null.
    // In case of datagram sockets (UDP), the addr can be null, which means forgetting
    // about the destination address, i.e., making the UDP sockets _unconnected_.

    let addr = {
        let addr_len = addr_len as usize;
        let sockaddr_storage = copy_sock_addr_from_user(addr, addr_len)?;
        AnyAddr::from_c_storage(&sockaddr_storage, addr_len as usize)?
    };
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;
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
    // Set output vars for the accepted address and its length, if needed
    let output_addr_buf_and_len: Option<(&mut [u8], &mut libc::socklen_t)> = {
        if !addr.is_null() {
            let output_len = {
                from_user::check_ptr(addr_len)?;
                unsafe { &mut *addr_len }
            };
            let addr = addr as *mut u8;
            let addr_len = *output_len as usize;
            let output_buf = {
                from_user::check_mut_array(addr, addr_len)?;
                unsafe { std::slice::from_raw_parts_mut(addr, addr_len) }
            };
            Some((output_buf, output_len))
        } else {
            None
        }
    };
    // Process other input arguments
    let flags = SocketFlags::from_bits_truncate(flags);
    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    // Do accept
    let accepted_socket = socket_file.accept().await?;

    // Set the non-blocking flag
    if flags.contains(SocketFlags::SOCK_NONBLOCK) {
        /*
        let new_flags = StatusFlags::O_NONBLOCK;
        socket_file.set_status_flags(new_flags).unwrap();
        */
        todo!("implement SocketFile::set_status_flags()")
    }
    // Output the address
    if let Some((output_addr_buf, output_addr_len)) = output_addr_buf_and_len {
        /*
        let (addr_storage, addr_len) = {
            let addr = accepted_socket.addr();
            addr.to_c_storage()
        };

        // Output the address's _actual_ length
        *output_addr_len = addr_len as _;
        // Output the address content
        let addr_buf = {
            let ptr = &addr_storage as *const u8;
            let len = addr_len;
            std::slice::from_raw_parts(ptr, len)
        };
        let copy_len = output_addr_buf.len().min(addr_buf.len());
        (&mut output_addr_buf[..copy_len]).copy_from_slice(&addr_buf[..copy_len]);
        */
        todo!("implement SocketFile::addr()")
    };
    // Update the file table
    let new_fd = {
        let new_file_ref = FileRef::new_socket(accepted_socket);
        let close_on_spawn = flags.contains(SocketFlags::SOCK_CLOEXEC);
        current!().add_file(new_file_ref, close_on_spawn)
    };
    Ok(new_fd as isize)
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

    let (src_addr, src_addr_len) = socket_file.peer_addr()?.to_c_storage();
    copy_sock_addr_to_user(src_addr, src_addr_len, addr, addr_len)?;
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

    let (src_addr, src_addr_len) = socket_file.addr()?.to_c_storage();
    copy_sock_addr_to_user(src_addr, src_addr_len, addr, addr_len)?;
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

    let optlen_mut = {
        from_user::check_mut_ptr(optlen)?;
        unsafe { &mut *optlen }
    };
    let optlen = *optlen_mut;
    let optval_mut = {
        from_user::check_mut_array(optval as *mut u8, optlen as usize)?;
        unsafe { std::slice::from_raw_parts_mut(optval as *mut u8, optlen as usize) }
    };

    let mut cmd = new_getsockopt_cmd(level, optname, optlen)?;
    socket_file.ioctl(cmd.as_mut())?;
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

    let file_ref = current!().file(fd as FileDesc)?;
    let socket_file = file_ref
        .as_socket_file()
        .ok_or_else(|| errno!(EINVAL, "not a socket"))?;

    let optval = {
        from_user::check_array(optval as *const u8, optlen as usize)?;
        unsafe { std::slice::from_raw_parts(optval as *const u8, optlen as usize) }
    };

    let mut cmd = new_setsockopt_cmd(level, optname, optval)?;
    socket_file.ioctl(cmd.as_mut())?;
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
    from_user::check_array(addr as *const u8, addr_len)?;
    if addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
        return_errno!(
            EINVAL,
            "addr len cannot be greater than sockaddr_storage's size"
        );
    }

    let sockaddr_storage = {
        // Safety. The content will be initialized before function returns.
        let mut sockaddr_storage =
            unsafe { MaybeUninit::<libc::sockaddr_storage>::uninit().assume_init() };
        // Safety. The dst slice is the only mutable reference to the sockaddr_storge
        let sockaddr_dst_buf = unsafe {
            let ptr = &mut sockaddr_storage as *mut _ as *mut u8;
            let len = addr_len;
            std::slice::from_raw_parts_mut(ptr, len)
        };
        // Safety. The src slice's pointer and length has been checked.
        let sockaddr_src_buf = unsafe {
            let ptr = addr as *const u8;
            let len = addr_len;
            std::slice::from_raw_parts(ptr, len)
        };
        sockaddr_dst_buf.copy_from_slice(sockaddr_src_buf);
        sockaddr_storage
    };
    Ok(sockaddr_storage)
}

fn copy_sock_addr_to_user(
    src_addr: libc::sockaddr_storage,
    src_addr_len: usize,
    dst_addr: *mut libc::sockaddr,
    dst_addr_len: *mut libc::socklen_t,
) -> Result<()> {
    if dst_addr.is_null() {
        return Ok(());
    }
    from_user::check_ptr(dst_addr_len)?;
    from_user::check_mut_array(dst_addr as *mut u8, unsafe { *dst_addr_len } as usize)?;

    let len = std::cmp::min(src_addr_len, unsafe { *dst_addr_len } as usize);
    let sockaddr_src_buf = unsafe {
        let ptr = &src_addr as *const _ as *const u8;
        std::slice::from_raw_parts(ptr, len)
    };
    let sockaddr_dst_buf = unsafe {
        let ptr = dst_addr as *mut u8;
        std::slice::from_raw_parts_mut(ptr, len)
    };
    sockaddr_dst_buf.copy_from_slice(sockaddr_src_buf);

    unsafe { *dst_addr_len = src_addr_len as u32 };
    Ok(())
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

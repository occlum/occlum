use std::convert::TryFrom;
use std::mem::MaybeUninit;

use num_enum::TryFromPrimitive;

use super::*;
use crate::fs::StatusFlags;
use crate::prelude::*;
use crate::util::mem_util::from_user;

pub async fn do_socket(domain: c_int, type_and_flags: c_int, protocol: c_int) -> Result<isize> {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
    #[repr(i32)]
    #[allow(non_camel_case_types)]
    enum Type {
        STREAM = 1,
        DGRAM = 2,
        RAW = 3,
        RDM = 4,
        SEQPACKET = 5,
        DCCP = 6,
        PACKET = 10,
    }

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

    let src_addr = socket_file.peer_addr()?;
    copy_sock_addr_to_user(src_addr, addr, addr_len)?;
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

    let src_addr = socket_file.addr()?;
    copy_sock_addr_to_user(src_addr, addr, addr_len)?;
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
    src_addr: AnyAddr,
    dst_addr: *mut libc::sockaddr,
    dst_addr_len: *mut libc::socklen_t,
) -> Result<()> {
    if dst_addr.is_null() {
        return Ok(());
    }
    from_user::check_ptr(dst_addr_len)?;
    if unsafe { *dst_addr_len } < 0 {
        return_errno!(EINVAL, "addrlen is invalid");
    }
    from_user::check_mut_array(dst_addr as *mut u8, unsafe { *dst_addr_len } as usize)?;

    let (src_addr, src_addr_len) = src_addr.to_c_storage();
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

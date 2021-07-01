use self::addr::Addr;
use super::*;

mod addr;
mod stream;

pub use self::addr::Addr as UnixAddr;
pub use self::stream::Stream;

//TODO: rewrite this file when a new kind of uds is added
pub fn unix_socket(socket_type: SocketType, flags: FileFlags, protocol: i32) -> Result<Stream> {
    if protocol != 0 && protocol != AddressFamily::LOCAL as i32 {
        return_errno!(EPROTONOSUPPORT, "protocol is not supported");
    }

    if socket_type == SocketType::STREAM {
        Ok(Stream::new(flags))
    } else {
        return_errno!(ESOCKTNOSUPPORT, "only stream type is supported");
    }
}

pub fn socketpair(
    socket_type: SocketType,
    flags: FileFlags,
    protocol: i32,
) -> Result<(Stream, Stream)> {
    if protocol != 0 && protocol != AddressFamily::LOCAL as i32 {
        return_errno!(EPROTONOSUPPORT, "protocol is not supported");
    }

    if socket_type == SocketType::STREAM {
        Stream::socketpair(flags)
    } else {
        return_errno!(ESOCKTNOSUPPORT, "only stream type is supported");
    }
}

pub trait AsUnixSocket {
    fn as_unix_socket(&self) -> Result<&Stream>;
}

impl AsUnixSocket for FileRef {
    fn as_unix_socket(&self) -> Result<&Stream> {
        self.as_any()
            .downcast_ref::<Stream>()
            .ok_or_else(|| errno!(EBADF, "not a unix socket"))
    }
}

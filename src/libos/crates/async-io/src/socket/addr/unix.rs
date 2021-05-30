use libc::c_char;

use super::{Addr, Domain};
use crate::prelude::*;

/// A UNIX address.
#[derive(Debug, PartialEq, Eq)]
pub enum UnixAddr {
    Unnamed,
    Pathname(PathUnixAddr),
    Abstract(AbstractUnixAddr),
}

impl UnixAddr {
    /// Construct a unix address from its C counterpart.
    ///
    /// The argument `c_len` specifies the length of the valid part in the given
    /// C address.
    pub fn from_c(c_addr: &libc::sockaddr_un, c_len: usize) -> Result<Self> {
        // Sanity checks
        if c_addr.sun_family != libc::AF_UNIX as _ {
            return_errno!(EINVAL, "an unix address is expected");
        }
        if c_len < std::mem::size_of::<libc::sa_family_t>() {
            return_errno!(EINVAL, "the length of the address is too small");
        } else if c_len > std::mem::size_of::<libc::sockaddr_un>() {
            return_errno!(EINVAL, "the length of the address is too large");
        }

        if c_len == std::mem::size_of::<libc::sa_family_t>() {
            return Ok(UnixAddr::Unnamed);
        }

        let path_len = c_len - std::mem::size_of::<libc::sa_family_t>();
        if path_len == 1 {
            // Both pathname and abstract addresses require a path_len greater than 1.
            return_errno!(EINVAL, "the pathname must not be empty");
        }
        debug_assert!(path_len > 1);

        // A pathname address
        if c_addr.sun_path[0] != 0 {
            // More sanity check
            let last_char = c_addr.sun_path[path_len - 1];
            if last_char != 0 {
                return_errno!(EINVAL, "the pathname is not null-terminated");
            }

            let pathname = {
                // Safe to convert from &[c_char] to &[u8]
                let path_slice: &[u8] = unsafe {
                    let char_slice: &[c_char] = &c_addr.sun_path[..(path_len - 1)];
                    std::mem::transmute(char_slice)
                };
                let path_str = std::str::from_utf8(path_slice)
                    .map_err(|_| errno!(EINVAL, "path is not UTF8"))?;
                path_str.to_string()
            };
            Ok(UnixAddr::Pathname(PathUnixAddr(pathname)))
        }
        // An abstract address
        else {
            // Safe to convert from &[c_char] to &[u8]
            let u8_slice: &[u8] = unsafe {
                let char_slice: &[c_char] = &c_addr.sun_path[..];
                std::mem::transmute(char_slice)
            };
            Ok(UnixAddr::Abstract(AbstractUnixAddr(Vec::from(u8_slice))))
        }
    }

    pub fn to_c(&self) -> (libc::sockaddr_un, usize) {
        let sun_family = libc::AF_UNIX as _;
        let mut sun_path: [u8; 108] = [0; 108];
        let c_len = match self {
            Self::Unnamed => std::mem::size_of::<libc::sa_family_t>(),
            Self::Pathname(PathUnixAddr(path)) => {
                let path = path.as_bytes();
                sun_path[..path.len()].copy_from_slice(&path[..]);
                sun_path[path.len()] = 0;
                path.len() + 1
            }
            Self::Abstract(AbstractUnixAddr(name)) => {
                sun_path[..name.len()].copy_from_slice(&name[..]);
                name.len()
            }
        };
        // Safe to convert from [u8; N] to [i8; N]
        let sun_path = unsafe { std::mem::transmute(sun_path) };

        let c_addr = libc::sockaddr_un {
            sun_family,
            sun_path,
        };
        (c_addr, c_len)
    }
}

impl Addr for UnixAddr {
    fn domain(&self) -> Domain {
        Domain::Unix
    }
}

// TODO: should be backed by an inode, instead of a string of path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathUnixAddr(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbstractUnixAddr(Vec<u8>);

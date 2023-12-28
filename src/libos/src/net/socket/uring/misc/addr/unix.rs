use std::any::Any;

use super::{Addr, CSockAddr, Domain};
use crate::prelude::*;
use std::ffi::CStr;
use std::hash::Hash;

const MAX_PATH_LEN: usize = 108;

/// A UNIX address.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnixAddr {
    Unnamed,
    Pathname(String),
    Abstract(Vec<u8>),
}

impl UnixAddr {
    /// Construct a unix address from its C counterpart.
    ///
    /// The argument `c_len` specifies the length of the valid part in the given
    /// C address.
    pub fn from_c(c_addr: &libc::sockaddr_un, c_len: usize) -> Result<Self> {
        // Sanity checks
        if c_addr.sun_family != libc::AF_UNIX as libc::sa_family_t {
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
        debug_assert!(path_len > 1);
        if path_len == 1 {
            // Both pathname and abstract addresses require a path_len greater than 1.
            return_errno!(EINVAL, "the pathname must not be empty");
        }

        // A pathname address
        if c_addr.sun_path[0] != 0 {
            // More sanity check
            let last_char = c_addr.sun_path[path_len - 1];
            if last_char != 0 {
                return_errno!(EINVAL, "the pathname is not null-terminated");
            }

            let pathname = {
                // Safety. Converting from &[c_char] to &[i8] is harmless.
                let path_slice: &[i8] = unsafe {
                    let char_slice = &c_addr.sun_path[..(path_len - 1)];
                    std::mem::transmute(char_slice)
                };
                let path_cstr = unsafe { CStr::from_ptr(path_slice.as_ptr()) };
                if path_cstr.to_bytes_with_nul().len() > MAX_PATH_LEN {
                    return_errno!(EINVAL, "no null in the address");
                }
                path_cstr
                    .to_str()
                    .map_err(|_| errno!(EINVAL, "path is not UTF8"))?
                    .to_string()
            };
            Ok(UnixAddr::Pathname(pathname))
        }
        // An abstract address
        else {
            // Safety. Converting from &[c_char] to &[u8] is harmless.
            let u8_slice: &[u8] = unsafe {
                let char_slice = &c_addr.sun_path[1..(path_len)];
                std::mem::transmute(char_slice)
            };
            Ok(UnixAddr::Abstract(Vec::from(u8_slice)))
        }
    }

    pub fn to_c(&self) -> (libc::sockaddr_un, usize) {
        const FAMILY_LEN: usize = std::mem::size_of::<libc::sa_family_t>();

        let sun_family = libc::AF_UNIX as _;
        let mut sun_path: [u8; 108] = [0; 108];
        let c_len = match self {
            Self::Unnamed => FAMILY_LEN,
            Self::Pathname(path) => {
                let path = path.as_bytes();
                sun_path[..path.len()].copy_from_slice(&path[..]);
                sun_path[path.len()] = 0;
                FAMILY_LEN + path.len() + 1
            }
            Self::Abstract(name) => {
                sun_path[0] = 0;
                sun_path[1..name.len() + 1].copy_from_slice(&name[..]);
                FAMILY_LEN + name.len() + 1
            }
        };
        // Safety. Convert from [u8; N] to [i8; N] is harmless.
        let sun_path = unsafe { std::mem::transmute(sun_path) };

        let c_addr = libc::sockaddr_un {
            sun_family,
            sun_path,
        };
        (c_addr, c_len)
    }

    pub fn get_path_name(&self) -> Result<&str> {
        match self {
            UnixAddr::Pathname(path) => Ok(path.as_ref()),
            _ => return_errno!(EINVAL, "can't get path name for abstract or unnamed socket"),
        }
    }

    pub fn new_with_path_name(path: &str) -> Self {
        Self::Pathname(path.to_owned())
    }
}

impl Addr for UnixAddr {
    fn domain() -> Domain {
        Domain::Unix
    }

    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        if c_addr_len > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }
        // Safety. Convert from sockaddr_storage to sockaddr_un is harmless.
        let c_addr = unsafe { std::mem::transmute(c_addr) };
        Self::from_c(c_addr, c_addr_len)
    }

    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let c_un_addr = self.to_c();
        c_un_addr.to_c_storage()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_default(&self) -> bool {
        let unix_default = Self::default();
        *self == unix_default
    }
}

impl Default for UnixAddr {
    fn default() -> Self {
        Self::Unnamed
    }
}

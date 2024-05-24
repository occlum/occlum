use crate::net::socket::CSockAddr;

use super::*;
use sgx_trts::c_str::CStr;
use std::path::{Path, PathBuf};
use std::{cmp, mem, slice, str};

const MAX_PATH_LEN: usize = 108;
const SUN_FAMILY_LEN: usize = mem::size_of::<libc::sa_family_t>();
lazy_static! {
    static ref SUN_PATH_OFFSET: usize = memoffset::offset_of!(libc::sockaddr_un, sun_path);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnixAddr {
    Unnamed,
    File(Option<usize>, UnixPath), // An optional inode number and path. Use inode if there is one.
    Abstract(String),
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
            return Ok(Self::Unnamed);
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

            Ok(Self::File(None, UnixPath::new(&pathname)))
        }
        // An abstract address
        else {
            // Safety. Converting from &[c_char] to &[u8] is harmless.
            let u8_slice: &[u8] = unsafe {
                let char_slice = &c_addr.sun_path[1..(path_len)];
                std::mem::transmute(char_slice)
            };
            Ok(Self::Abstract(
                str::from_utf8(u8_slice).unwrap().to_string(),
            ))
        }
    }

    pub fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        if (c_addr_len) > std::mem::size_of::<libc::sockaddr_storage>() {
            return_errno!(EINVAL, "address length is too large");
        }
        // Safety. Convert from sockaddr_storage to sockaddr_un is harmless.
        let c_addr = unsafe { std::mem::transmute(c_addr) };
        unsafe { Self::from_c(c_addr, c_addr_len) }
    }

    pub fn copy_to_slice(&self, dst: &mut [u8]) -> usize {
        let (raw_addr, addr_len) = self.to_c();
        let src =
            unsafe { std::slice::from_raw_parts(&raw_addr as *const _ as *const u8, addr_len) };
        let copied = std::cmp::min(dst.len(), addr_len);
        dst[..copied].copy_from_slice(&src[..copied]);
        copied
    }

    pub fn raw_len(&self) -> usize {
        /// The '/0' at the end of Self::File counts
        match self.path_str() {
            Ok(str) => str.len() + 1 + *SUN_PATH_OFFSET,
            Err(_) => std::mem::size_of::<libc::sa_family_t>(),
        }
    }

    pub fn path_str(&self) -> Result<&str> {
        match self {
            Self::File(_, unix_path) => Ok(&unix_path.path_str()),
            Self::Abstract(path) => Ok(&path),
            Self::Unnamed => return_errno!(EINVAL, "can't get path name for unnamed socket"),
        }
    }

    pub fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let c_un_addr = self.to_c();
        c_un_addr.to_c_storage()
    }

    pub fn to_raw(&self) -> SockAddr {
        let (storage, addr_len) = self.to_c_storage();
        SockAddr::from_c_storage(&storage, addr_len)
    }

    fn to_c(&self) -> (libc::sockaddr_un, usize) {
        const FAMILY_LEN: usize = std::mem::size_of::<libc::sa_family_t>();

        let mut addr: libc::sockaddr_un = unsafe { mem::zeroed() };
        addr.sun_family = Domain::LOCAL as libc::sa_family_t;

        let addr_len = match self {
            Self::Unnamed => FAMILY_LEN,
            Self::File(_, unix_path) => {
                let path_str = unix_path.path_str();
                let buf_len = path_str.len();
                /// addr is initialized to all zeros and try_from_raw guarentees
                /// unix_path length is shorter than sun_path, so sun_path here
                /// will always have a null terminator
                addr.sun_path[..buf_len]
                    .copy_from_slice(unsafe { &*(path_str.as_bytes() as *const _ as *const [i8]) });
                buf_len + *SUN_PATH_OFFSET + 1
            }
            Self::Abstract(path_str) => {
                addr.sun_path[0] = 0;
                let buf_len = path_str.len() + 1;
                addr.sun_path[1..buf_len]
                    .copy_from_slice(unsafe { &*(path_str.as_bytes() as *const _ as *const [i8]) });
                buf_len + *SUN_PATH_OFFSET
            }
        };

        (addr, addr_len)
    }
}

impl Default for UnixAddr {
    fn default() -> Self {
        UnixAddr::Unnamed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnixPath {
    inner: PathBuf,
    /// Holds the cwd when a relative path is created
    cwd: Option<String>,
}

impl UnixPath {
    pub fn new(path: &str) -> Self {
        let inner = PathBuf::from(path);
        let is_absolute = inner.is_absolute();
        Self {
            inner: inner,
            cwd: if is_absolute {
                None
            } else {
                let thread = current!();
                let fs = thread.fs().read().unwrap();
                let cwd = fs.cwd().to_owned();

                Some(cwd)
            },
        }
    }

    pub fn absolute(&self) -> String {
        let path_str = self.path_str();
        if self.inner.is_absolute() {
            path_str.to_string()
        } else {
            let mut prefix = self.cwd.as_ref().unwrap().clone();
            if prefix.ends_with("/") {
                prefix.push_str(path_str);
            } else {
                prefix.push_str("/");
                prefix.push_str(path_str);
            }
            prefix
        }
    }

    pub fn path_str(&self) -> &str {
        self.inner.to_str().unwrap()
    }
}

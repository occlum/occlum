use super::*;
use std::path::{Path, PathBuf};
use std::{cmp, mem, slice, str};

const MAX_PATH_LEN: usize = 108;
const SUN_FAMILY_LEN: usize = mem::size_of::<libc::sa_family_t>();
lazy_static! {
    static ref SUN_PATH_OFFSET: usize = memoffset::offset_of!(libc::sockaddr_un, sun_path);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Addr {
    File(Option<usize>, UnixPath), // An optional inode number and path. Use inode if there is one.
    Abstract(String),
}

impl Addr {
    /// Caller should guarentee the sockaddr and addr_len are valid.
    /// The pathname should end with a '\0' within the passed length.
    /// The abstract name should both start and end with a '\0' within the passed length.
    pub unsafe fn try_from_raw(
        sockaddr: *const libc::sockaddr,
        addr_len: libc::socklen_t,
    ) -> Result<Self> {
        let addr_len = addr_len as usize;

        // TODO: support autobind to validate when addr_len == SUN_FAMILY_LEN
        if addr_len <= SUN_FAMILY_LEN {
            return_errno!(EINVAL, "the address is too short.");
        }

        if addr_len > MAX_PATH_LEN + *SUN_PATH_OFFSET {
            return_errno!(EINVAL, "the address is too long.");
        }

        if AddressFamily::try_from((*sockaddr).sa_family)? != AddressFamily::LOCAL {
            return_errno!(EINVAL, "not a valid address for unix socket");
        }

        let sockaddr = sockaddr as *const libc::sockaddr_un;
        let sun_path = (*sockaddr).sun_path;

        if sun_path[0] == 0 {
            let path_ptr = sun_path[1..(addr_len - *SUN_PATH_OFFSET)].as_ptr();
            let path_slice =
                slice::from_raw_parts(path_ptr as *const u8, addr_len - *SUN_PATH_OFFSET - 1);

            Ok(Self::Abstract(
                str::from_utf8(&path_slice).unwrap().to_string(),
            ))
        } else {
            let path_cstr = CStr::from_ptr(sun_path.as_ptr());
            if path_cstr.to_bytes_with_nul().len() > MAX_PATH_LEN {
                return_errno!(EINVAL, "no null in the address");
            }

            Ok(Self::File(None, UnixPath::new(path_cstr.to_str().unwrap())))
        }
    }

    pub fn copy_to_slice(&self, dst: &mut [u8]) -> usize {
        let (raw_addr, addr_len) = self.to_raw();
        let src =
            unsafe { std::slice::from_raw_parts(&raw_addr as *const _ as *const u8, addr_len) };
        let copied = std::cmp::min(dst.len(), addr_len);
        dst[..copied].copy_from_slice(&src[..copied]);
        copied
    }

    pub fn raw_len(&self) -> usize {
        /// The '/0' at the end of Self::File counts
        self.path_str().len()
            + 1
            + *SUN_PATH_OFFSET
    }

    pub fn path_str(&self) -> &str {
        match self {
            Self::File(_, unix_path) => &unix_path.path_str(),
            Self::Abstract(path) => &path,
        }
    }

    fn to_raw(&self) -> (libc::sockaddr_un, usize) {
        let mut addr: libc::sockaddr_un = unsafe { mem::zeroed() };
        addr.sun_family = AddressFamily::LOCAL as libc::sa_family_t;

        let addr_len = match self {
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
            let mut prefix = path_str.to_owned();
            prefix.push_str(self.cwd.as_ref().unwrap());
            prefix
        }
    }

    pub fn path_str(&self) -> &str {
        self.inner.to_str().unwrap()
    }
}

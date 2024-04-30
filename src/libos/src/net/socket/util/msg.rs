/// Socket message and its flags.
use super::*;

/// This struct is used to iterate through the control messages.
///
/// `cmsghdr` is a C struct for ancillary data object information of a unix socket.
pub struct CMessages<'a> {
    buffer: &'a [u8],
    current: Option<&'a libc::cmsghdr>,
}

impl<'a> Iterator for CMessages<'a> {
    type Item = CmsgData<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let cmsg = unsafe {
            let mut msg: libc::msghdr = core::mem::zeroed();
            msg.msg_control = self.buffer.as_ptr() as *mut _;
            msg.msg_controllen = self.buffer.len() as _;

            let cmsg = if let Some(current) = self.current {
                libc::CMSG_NXTHDR(&msg, current)
            } else {
                libc::CMSG_FIRSTHDR(&msg)
            };
            cmsg.as_ref()?
        };

        self.current = Some(cmsg);
        CmsgData::try_from_cmsghdr(cmsg)
    }
}

impl<'a> CMessages<'a> {
    pub fn from_bytes(msg_control: &'a mut [u8]) -> Self {
        Self {
            buffer: msg_control,
            current: None,
        }
    }
}

/// Control message data of variable type. The data resides next to `cmsghdr`.
pub enum CmsgData<'a> {
    ScmRights(ScmRights<'a>),
    ScmCredentials,
}

impl<'a> CmsgData<'a> {
    /// Create an `CmsgData::ScmRights` variant.
    ///
    /// # Safety
    ///
    /// `data` must contain a valid control message and the control message must be type of
    /// `SOL_SOCKET` and level of `SCM_RIGHTS`.
    unsafe fn as_rights(data: &'a mut [u8]) -> Self {
        let scm_rights = ScmRights { data };
        CmsgData::ScmRights(scm_rights)
    }

    /// Create an `CmsgData::ScmCredentials` variant.
    ///
    /// # Safety
    ///
    /// `data` must contain a valid control message and the control message must be type of
    /// `SOL_SOCKET` and level of `SCM_CREDENTIALS`.
    unsafe fn as_credentials(_data: &'a [u8]) -> Self {
        CmsgData::ScmCredentials
    }

    fn try_from_cmsghdr(cmsg: &'a libc::cmsghdr) -> Option<Self> {
        unsafe {
            let cmsg_len_zero = libc::CMSG_LEN(0) as usize;
            let data_len = (*cmsg).cmsg_len as usize - cmsg_len_zero;
            let data = libc::CMSG_DATA(cmsg);
            let data = core::slice::from_raw_parts_mut(data, data_len);

            match (*cmsg).cmsg_level {
                libc::SOL_SOCKET => match (*cmsg).cmsg_type {
                    libc::SCM_RIGHTS => Some(CmsgData::as_rights(data)),
                    libc::SCM_CREDENTIALS => Some(CmsgData::as_credentials(data)),
                    _ => None,
                },
                _ => None,
            }
        }
    }
}

/// The data unit of this control message is file descriptor(s).
///
/// The level is equal to `SOL_SOCKET` and the type is equal to `SCM_RIGHTS`.
pub struct ScmRights<'a> {
    data: &'a mut [u8],
}

impl<'a> ScmRights<'a> {
    /// Iterate and reassign each fd in data buffer, given a reassignment function.
    pub fn iter_and_reassign_fds<F>(&mut self, reassign_fd_fn: F)
    where
        F: Fn(FileDesc) -> FileDesc,
    {
        for fd_bytes in self.data.chunks_exact_mut(core::mem::size_of::<FileDesc>()) {
            let old_fd = FileDesc::from_ne_bytes(fd_bytes.try_into().unwrap());
            let reassigned_fd = reassign_fd_fn(old_fd);
            fd_bytes.copy_from_slice(&reassigned_fd.to_ne_bytes());
        }
    }

    pub fn iter_fds(&self) -> impl Iterator<Item = FileDesc> + '_ {
        self.data
            .chunks_exact(core::mem::size_of::<FileDesc>())
            .map(|fd_bytes| FileDesc::from_ne_bytes(fd_bytes.try_into().unwrap()))
    }
}

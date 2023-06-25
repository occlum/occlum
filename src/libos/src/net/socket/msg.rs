/// Socket message and its flags.
use super::*;

/// C struct for a socket message with const pointers
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct msghdr {
    pub msg_name: *const c_void,
    pub msg_namelen: libc::socklen_t,
    pub msg_iov: *const libc::iovec,
    pub msg_iovlen: size_t,
    pub msg_control: *const c_void,
    pub msg_controllen: size_t,
    pub msg_flags: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct mmsghdr {
    pub msg_hdr: msghdr,
    pub msg_len: c_uint,
}

/// C struct for a socket message with mutable pointers
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct msghdr_mut {
    pub msg_name: *mut c_void,
    pub msg_namelen: libc::socklen_t,
    pub msg_iov: *mut libc::iovec,
    pub msg_iovlen: size_t,
    pub msg_control: *mut c_void,
    pub msg_controllen: size_t,
    pub msg_flags: c_int,
}

/// MsgHdr is a memory-safe, immutable wrapper of msghdr
pub struct MsgHdr<'a> {
    name: Option<&'a [u8]>,
    iovs: Iovs<'a>,
    control: Option<&'a [u8]>,
    flags: MsgHdrFlags,
    c_self: &'a msghdr,
}

impl<'a> MsgHdr<'a> {
    /// Wrap a unsafe msghdr into a safe MsgHdr
    pub unsafe fn from_c(c_msg: &'a msghdr) -> Result<MsgHdr> {
        // Convert c_msg's (*mut T, usize)-pair fields to Option<&mut [T]>
        let name_opt_slice =
            new_optional_slice(c_msg.msg_name as *const u8, c_msg.msg_namelen as usize);
        let iovs_opt_slice = new_optional_slice(
            c_msg.msg_iov as *const libc::iovec,
            c_msg.msg_iovlen as usize,
        );
        let control_opt_slice = new_optional_slice(
            c_msg.msg_control as *const u8,
            c_msg.msg_controllen as usize,
        );

        let flags = MsgHdrFlags::from_bits_truncate(c_msg.msg_flags);

        let iovs = {
            let iovs_vec = match iovs_opt_slice {
                Some(iovs_slice) => iovs_slice
                    .iter()
                    .flat_map(|iov| new_optional_slice(iov.iov_base as *const u8, iov.iov_len))
                    .collect(),
                None => Vec::new(),
            };
            Iovs::new(iovs_vec)
        };

        Ok(Self {
            name: name_opt_slice,
            iovs: iovs,
            control: control_opt_slice,
            flags: flags,
            c_self: c_msg,
        })
    }

    pub fn get_iovs(&self) -> &Iovs {
        &self.iovs
    }

    pub fn get_name(&self) -> Option<&[u8]> {
        self.name
    }

    pub fn get_control(&self) -> Option<&[u8]> {
        self.control
    }

    pub fn get_flags(&self) -> MsgHdrFlags {
        self.flags
    }
}

/// MsgHdrMut is a memory-safe, mutable wrapper of msghdr_mut
pub struct MsgHdrMut<'a> {
    name: Option<&'a mut [u8]>,
    iovs: IovsMut<'a>,
    control: Option<&'a mut [u8]>,
    flags: MsgHdrFlags,
    c_self: &'a mut msghdr_mut,
}

// TODO: use macros to eliminate redundant code between MsgHdr and MsgHdrMut
impl<'a> MsgHdrMut<'a> {
    /// Wrap a unsafe msghdr_mut into a safe MsgHdrMut
    pub unsafe fn from_c(c_msg: &'a mut msghdr_mut) -> Result<MsgHdrMut> {
        // Convert c_msg's (*mut T, usize)-pair fields to Option<&mut [T]>
        let name_opt_slice =
            new_optional_slice_mut(c_msg.msg_name as *mut u8, c_msg.msg_namelen as usize);
        let iovs_opt_slice =
            new_optional_slice_mut(c_msg.msg_iov as *mut libc::iovec, c_msg.msg_iovlen as usize);
        let control_opt_slice =
            new_optional_slice_mut(c_msg.msg_control as *mut u8, c_msg.msg_controllen as usize);

        let flags = MsgHdrFlags::from_bits_truncate(c_msg.msg_flags);

        let iovs = {
            let iovs_vec = match iovs_opt_slice {
                Some(iovs_slice) => iovs_slice
                    .iter()
                    .flat_map(|iov| new_optional_slice_mut(iov.iov_base as *mut u8, iov.iov_len))
                    .collect(),
                None => Vec::new(),
            };
            IovsMut::new(iovs_vec)
        };

        Ok(Self {
            name: name_opt_slice,
            iovs: iovs,
            control: control_opt_slice,
            flags: flags,
            c_self: c_msg,
        })
    }

    /////////////////////////////////////////////////////////////////////////
    // Immutable interfaces (same as MsgHdr)
    /////////////////////////////////////////////////////////////////////////

    pub fn get_iovs(&self) -> &IovsMut {
        &self.iovs
    }

    pub fn get_name(&self) -> Option<&[u8]> {
        self.name.as_ref().map(|name| &name[..])
    }

    pub fn get_control(&self) -> Option<&[u8]> {
        self.control.as_ref().map(|control| &control[..])
    }

    pub fn get_flags(&self) -> MsgHdrFlags {
        self.flags
    }

    /////////////////////////////////////////////////////////////////////////
    // Mutable interfaces (unique to MsgHdrMut)
    /////////////////////////////////////////////////////////////////////////

    pub fn get_iovs_mut<'b>(&'b mut self) -> &'b mut IovsMut<'a> {
        &mut self.iovs
    }

    pub fn get_name_mut(&mut self) -> Option<&mut [u8]> {
        self.name.as_mut().map(|name| &mut name[..])
    }

    pub fn get_name_max_len(&self) -> usize {
        self.name.as_ref().map(|name| name.len()).unwrap_or(0)
    }

    pub fn set_name_len(&mut self, new_name_len: usize) -> Result<()> {
        if new_name_len > self.get_name_max_len() {
            return_errno!(EINVAL, "new_name_len is too big");
        }
        self.c_self.msg_namelen = new_name_len as libc::socklen_t;
        Ok(())
    }

    pub fn get_control_mut(&mut self) -> Option<&mut [u8]> {
        self.control.as_mut().map(|control| &mut control[..])
    }

    pub fn get_control_max_len(&self) -> usize {
        self.control
            .as_ref()
            .map(|control| control.len())
            .unwrap_or(0)
    }

    pub fn set_control_len(&mut self, new_control_len: usize) -> Result<()> {
        if new_control_len > self.get_control_max_len() {
            return_errno!(EINVAL, "new_control_len is too big");
        }
        self.c_self.msg_controllen = new_control_len;
        Ok(())
    }

    pub fn get_iovs_name_and_control_mut(
        &mut self,
    ) -> (&mut IovsMut<'a>, Option<&mut [u8]>, Option<&mut [u8]>) {
        (
            &mut self.iovs,
            self.name.as_mut().map(|name| &mut name[..]),
            self.control.as_mut().map(|control| &mut control[..]),
        )
    }

    pub fn set_flags(&mut self, flags: MsgHdrFlags) {
        self.flags = flags;
        self.c_self.msg_flags = flags.bits();
    }
}

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

unsafe fn new_optional_slice<'a, T>(slice_ptr: *const T, slice_size: usize) -> Option<&'a [T]> {
    if !slice_ptr.is_null() {
        let slice = core::slice::from_raw_parts::<T>(slice_ptr, slice_size);
        Some(slice)
    } else {
        None
    }
}

unsafe fn new_optional_slice_mut<'a, T>(
    slice_ptr: *mut T,
    slice_size: usize,
) -> Option<&'a mut [T]> {
    if !slice_ptr.is_null() {
        let slice = core::slice::from_raw_parts_mut::<T>(slice_ptr, slice_size);
        Some(slice)
    } else {
        None
    }
}

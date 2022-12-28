use crate::prelude::*;

#[derive(Debug)]
#[repr(C)]
pub struct IfConf {
    pub ifc_len: i32,
    pub ifc_buf: *const u8,
}

#[derive(Debug)]
pub enum GetIfConf {
    IfConfBuf(Vec<u8>),
    IfConfLen(usize),
}

impl GetIfConf {
    pub fn new(ifconf: &IfConf) -> Self {
        if ifconf.ifc_buf.is_null() {
            Self::IfConfLen(ifconf.ifc_len as usize)
        } else {
            let buf = {
                let mut buf = Vec::with_capacity(ifconf.ifc_len as usize);
                buf.resize(ifconf.ifc_len as usize, 0);
                buf
            };
            Self::IfConfBuf(buf)
        }
    }

    pub fn execute(&mut self, fd: HostFd) -> Result<()> {
        if let Self::IfConfBuf(buf) = self {
            if buf.len() == 0 {
                return Ok(());
            }
        }

        let mut if_conf = self.to_raw_ifconf();
        get_ifconf_by_host(fd, &mut if_conf)?;
        self.set_len(if_conf.ifc_len as usize);
        Ok(())
    }

    fn set_len(&mut self, new_len: usize) {
        match self {
            Self::IfConfLen(len) => {
                *len = new_len;
            }
            Self::IfConfBuf(buf) => {
                buf.resize(new_len, 0);
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::IfConfLen(len) => *len,
            Self::IfConfBuf(buf) => buf.len(),
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        match self {
            Self::IfConfBuf(buf) => Some(buf.as_slice()),
            Self::IfConfLen(_) => None,
        }
    }

    pub fn to_raw_ifconf(&self) -> IfConf {
        match self {
            Self::IfConfLen(len) => IfConf {
                ifc_buf: std::ptr::null(),
                ifc_len: *len as i32,
            },
            Self::IfConfBuf(buf) => IfConf {
                ifc_buf: buf.as_ptr(),
                ifc_len: buf.len() as i32,
            },
        }
    }
}

impl IoctlCmd for GetIfConf {}

const SIOCGIFCONF: u32 = 0x8912;

#[cfg(feature = "sgx")]
fn get_ifconf_by_host(fd: HostFd, if_conf: &mut IfConf) -> Result<()> {
    extern "C" {
        // Used to ioctl arguments with pointer members.
        //
        // Before the call the area the pointers points to should be assembled into
        // one continuous memory block. Then the block is repacked to ioctl arguments
        // in the ocall implementation in host.
        //
        // ret: holds the return value of ioctl in host
        // fd: the host fd for the device
        // cmd_num: request number of the ioctl
        // buf: the data to exchange with host
        // len: the size of the buf
        // recv_len: accepts transferred data length when buf is used to get data from host
        //
        fn socket_ocall_ioctl_repack(
            ret: *mut i32,
            fd: i32,
            cmd_num: i32,
            buf: *const u8,
            len: i32,
            recv_len: *mut i32,
        ) -> sgx_types::sgx_status_t;
    }

    try_libc!({
        let mut recv_len: i32 = 0;
        let mut retval: i32 = 0;
        let status = socket_ocall_ioctl_repack(
            &mut retval as *mut i32,
            fd as _,
            SIOCGIFCONF as _,
            if_conf.ifc_buf,
            if_conf.ifc_len,
            &mut recv_len as *mut i32,
        );
        assert!(status == sgx_types::sgx_status_t::SGX_SUCCESS);

        // If ifc_req is NULL, SIOCGIFCONF returns the necessary buffer
        // size in bytes for receiving all available addresses in ifc_len
        // which is irrelevant to the orginal ifc_len.
        if !if_conf.ifc_buf.is_null() {
            assert!(if_conf.ifc_len >= recv_len);
        }
        if_conf.ifc_len = recv_len;
        retval
    });

    Ok(())
}

#[cfg(not(feature = "sgx"))]
fn get_ifconf_by_host(fd: HostFd, if_conf: &mut IfConf) -> Result<()> {
    try_libc!(libc::ioctl(
        fd as _,
        SIOCGIFCONF as _,
        if_conf as *mut IfConf as *mut i32
    ));
    Ok(())
}

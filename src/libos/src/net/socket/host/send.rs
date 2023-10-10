use super::*;

#[derive(Debug)]
pub struct Sender {
    alloc: UntrustedSliceAlloc,
}

impl Sender {
    pub fn new() -> Result<Self> {
        let alloc = UntrustedSliceAlloc::new(SEND_BUF_SIZE)?;
        Ok(Self { alloc })
    }
}

impl HostSocket {
    pub fn send(&self, buf: &[u8], flags: SendFlags) -> Result<usize> {
        self.sendto(buf, flags, &None)
    }

    pub fn sendmsg<'a, 'b>(&self, msg: &'b MsgHdr<'a>, flags: SendFlags) -> Result<usize> {
        let msg_iov = msg.get_iovs();

        self.do_sendmsg(
            msg_iov.as_slices(),
            flags,
            msg.get_name(),
            msg.get_control(),
        )
    }

    pub(super) fn do_sendmsg(
        &self,
        data: &[&[u8]],
        flags: SendFlags,
        name: Option<&[u8]>,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let data_length = data.iter().map(|s| s.len()).sum();
        let mut sender: SgxMutexGuard<'_, Sender>;
        let mut ocall_alloc;
        // Allocated slice in untrusted memory region
        let u_allocator = if data_length > SEND_BUF_SIZE {
            // Ocall allocator
            ocall_alloc = UntrustedSliceAlloc::new(data_length)?;
            &mut ocall_alloc
        } else {
            // Inner allocator, lock buffer until send ocall completion
            sender = self.sender.lock().unwrap();
            &mut sender.alloc
        };

        let u_data = {
            let mut bufs = Vec::new();
            for buf in data {
                let u_slice = u_allocator.new_slice(buf)?;
                bufs.push(u_slice);
            }
            bufs
        };

        let retval = self.do_sendmsg_untrusted_data(&u_data, flags, name, control);
        u_allocator.reset();
        retval
    }

    fn do_sendmsg_untrusted_data(
        &self,
        u_data: &[UntrustedSlice],
        flags: SendFlags,
        name: Option<&[u8]>,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        // Prepare the arguments for OCall
        let mut retval: isize = 0;
        // Host socket fd
        let host_fd = self.raw_host_fd() as i32;
        // Name
        let (msg_name, msg_namelen) = name.as_ptr_and_len();
        let msg_name = msg_name as *const c_void;
        // Iovs
        let raw_iovs: Vec<libc::iovec> = u_data
            .iter()
            .map(|slice| slice.as_ref().as_libc_iovec())
            .collect();
        let (msg_iov, msg_iovlen) = raw_iovs.as_slice().as_ptr_and_len();
        // Control
        let (msg_control, msg_controllen) = control.as_ptr_and_len();
        let msg_control = msg_control as *const c_void;
        // Flags
        let raw_flags = flags.bits();

        // Do OCall
        unsafe {
            let status = occlum_ocall_sendmsg(
                &mut retval as *mut isize,
                host_fd,
                msg_name,
                msg_namelen as u32,
                msg_iov,
                msg_iovlen,
                msg_control,
                msg_controllen,
                raw_flags,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
        }
        let bytes_sent = if flags.contains(SendFlags::MSG_NOSIGNAL) {
            try_libc!(retval)
        } else {
            try_libc_may_epipe!(retval)
        };

        debug_assert!(bytes_sent >= 0);
        Ok(bytes_sent as usize)
    }
}

extern "C" {
    fn occlum_ocall_sendmsg(
        ret: *mut ssize_t,
        fd: c_int,
        msg_name: *const c_void,
        msg_namelen: libc::socklen_t,
        msg_data: *const libc::iovec,
        msg_datalen: size_t,
        msg_control: *const c_void,
        msg_controllen: size_t,
        flags: c_int,
    ) -> sgx_status_t;
}

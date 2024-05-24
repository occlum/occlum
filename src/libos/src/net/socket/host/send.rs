use super::*;

impl HostSocket {
    pub fn send(&self, buf: &[u8], flags: SendFlags) -> Result<usize> {
        self.sendto(buf, flags, None)
    }

    pub fn sendmsg(
        &self,
        data: &[&[u8]],
        flags: SendFlags,
        addr: Option<AnyAddr>,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let current = current!();
        let data_length = data.iter().map(|s| s.len()).sum();
        let mut ocall_alloc;
        // Allocated slice in untrusted memory region
        let u_allocator = if data_length > IO_BUF_SIZE {
            // Ocall allocator
            ocall_alloc = UntrustedSliceAlloc::new(data_length)?;
            ocall_alloc.guard()
        } else {
            // IO buffer per thread
            current.io_buffer()
        };

        let u_data = {
            let mut bufs = Vec::new();
            for buf in data {
                let u_slice = u_allocator.new_slice(buf)?;
                bufs.push(u_slice);
            }
            bufs
        };

        let raw_addr = addr.map(|addr| addr.to_raw());

        self.do_sendmsg_untrusted_data(
            &u_data,
            flags,
            raw_addr.as_ref().map(|addr| addr.as_slice()),
            control,
        )
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

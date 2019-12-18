use super::*;

impl SocketFile {
    // TODO: need sockaddr type to implement send/sento
    /*
    pub fn send(&self, buf: &[u8], flags: MsgFlags) -> Result<usize> {
        self.sendto(buf, flags, None)
    }

    pub fn sendto(&self, buf: &[u8], flags: MsgFlags, dest_addr: Option<&[u8]>) -> Result<usize> {
        Self::do_sendmsg(
            self.host_fd,
            &buf[..],
            flags,
            dest_addr,
            None)
    }
    */

    pub fn sendmsg<'a, 'b>(&self, msg: &'b MsgHdr<'a>, flags: MsgFlags) -> Result<usize> {
        // Copy data in iovs into a single buffer
        let data_buf = msg.get_iovs().gather_to_vec();

        self.do_sendmsg(&data_buf[..], flags, msg.get_name(), msg.get_control())
    }

    fn do_sendmsg(
        &self,
        data: &[u8],
        flags: MsgFlags,
        name: Option<&[u8]>,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let bytes_sent = try_libc!({
            // Prepare the arguments for OCall
            let mut retval: isize = 0;
            // Host socket fd
            let host_fd = self.host_fd;
            // Name
            let (msg_name, msg_namelen) = name.get_ptr_and_len();
            let msg_name = msg_name as *const c_void;
            // Data
            let msg_data = data.as_ptr();
            let msg_datalen = data.len();
            // Control
            let (msg_control, msg_controllen) = control.get_ptr_and_len();
            let msg_control = msg_control as *const c_void;
            // Flags
            let flags = flags.to_u32() as i32;

            // Do OCall
            let status = occlum_ocall_sendmsg(
                &mut retval as *mut isize,
                host_fd,
                msg_name,
                msg_namelen as u32,
                msg_data,
                msg_datalen,
                msg_control,
                msg_controllen,
                flags,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);

            retval
        });
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
        msg_data: *const u8,
        msg_datalen: size_t,
        msg_control: *const c_void,
        msg_controllen: size_t,
        flags: c_int,
    ) -> sgx_status_t;
}

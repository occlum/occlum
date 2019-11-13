use super::*;

impl SocketFile {
    // TODO: need sockaddr type to implement send/sento
    /*
    pub fn recv(&self, buf: &mut [u8], flags: MsgFlags) -> Result<usize> {
        let (bytes_recvd, _) = self.recvfrom(buf, flags, None)?;
        Ok(bytes_recvd)
    }

    pub fn recvfrom(&self, buf: &mut [u8], flags: MsgFlags, src_addr: Option<&mut [u8]>) -> Result<(usize, usize)> {
        let (bytes_recvd, src_addr_len, _, _) = self.do_recvmsg(
            &mut buf[..],
            flags,
            src_addr,
            None,
        )?;
        Ok((bytes_recvd, src_addr_len))
    }*/

    pub fn recvmsg<'a, 'b>(&self, msg: &'b mut MsgHdrMut<'a>, flags: MsgFlags) -> Result<usize> {
        // Allocate a single data buffer is big enough for all iovecs of msg.
        // This is a workaround for the OCall that takes only one data buffer.
        let mut data_buf = {
            let data_buf_len = msg.get_iovs().total_bytes();
            let data_vec = vec![0; data_buf_len];
            data_vec.into_boxed_slice()
        };

        let (bytes_recvd, namelen_recvd, controllen_recvd, flags_recvd) = {
            let data = &mut data_buf[..];
            // Acquire mutable references to the name and control buffers
            let (name, control) = msg.get_name_and_control_mut();
            // Fill the data, the name, and the control buffers
            self.do_recvmsg(data, flags, name, control)?
        };

        // Update the lengths and flags
        msg.set_name_len(namelen_recvd)?;
        msg.set_control_len(controllen_recvd)?;
        msg.set_flags(flags_recvd);

        let recv_data = &data_buf[..bytes_recvd];
        // TODO: avoid this one extra copy due to the intermediate data buffer
        msg.get_iovs_mut().scatter_copy_from(recv_data);

        Ok(bytes_recvd)
    }

    fn do_recvmsg(
        &self,
        data: &mut [u8],
        flags: MsgFlags,
        mut name: Option<&mut [u8]>,
        mut control: Option<&mut [u8]>,
    ) -> Result<(usize, usize, usize, MsgFlags)> {
        // Prepare the arguments for OCall
        // Host socket fd
        let host_fd = self.host_fd;
        // Name
        let (msg_name, msg_namelen) = name.get_mut_ptr_and_len();
        let msg_name = msg_name as *mut c_void;
        let mut msg_namelen_recvd = 0_u32;
        // Data
        let msg_data = data.as_mut_ptr();
        let msg_datalen = data.len();
        // Control
        let (msg_control, msg_controllen) = control.get_mut_ptr_and_len();
        let msg_control = msg_control as *mut c_void;
        let mut msg_controllen_recvd = 0;
        // Flags
        let flags = flags.to_u32() as i32;
        let mut msg_flags_recvd = 0;

        // Do OCall
        let retval = try_libc!({
            let mut retval = 0_isize;
            let status = ocall_recvmsg(
                &mut retval as *mut isize,
                host_fd,
                msg_name,
                msg_namelen as u32,
                &mut msg_namelen_recvd as *mut u32,
                msg_data,
                msg_datalen,
                msg_control,
                msg_controllen,
                &mut msg_controllen_recvd as *mut usize,
                &mut msg_flags_recvd as *mut i32,
                flags,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);

            // TODO: what if retval < 0 but buffers are modified by the
            // untrusted OCall? We reset the potentially tampered buffers.
            retval
        });

        // Check values returned from outside the enclave
        let bytes_recvd = {
            // Guarantted by try_libc!
            debug_assert!(retval >= 0);
            let retval = retval as usize;

            // Check bytes_recvd returned from outside the enclave
            assert!(retval <= data.len());
            retval
        };
        let msg_namelen_recvd = msg_namelen_recvd as usize;
        assert!(msg_namelen_recvd <= msg_namelen);
        assert!(msg_controllen_recvd <= msg_controllen);
        let flags_recvd = MsgFlags::from_u32(msg_flags_recvd as u32)?;

        Ok((
            bytes_recvd,
            msg_namelen_recvd,
            msg_controllen_recvd,
            flags_recvd,
        ))
    }
}

extern "C" {
    fn ocall_recvmsg(
        ret: *mut ssize_t,
        fd: c_int,
        msg_name: *mut c_void,
        msg_namelen: libc::socklen_t,
        msg_namelen_recv: *mut libc::socklen_t,
        msg_data: *mut u8,
        msg_data: size_t,
        msg_control: *mut c_void,
        msg_controllen: size_t,
        msg_controllen_recv: *mut size_t,
        msg_flags: *mut c_int,
        flags: c_int,
    ) -> sgx_status_t;
}

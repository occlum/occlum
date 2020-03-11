use super::*;
use crate::untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSliceAlloc};

impl SocketFile {
    // TODO: need sockaddr type to implement send/sento
    /*
    pub fn recv(&self, buf: &mut [u8], flags: MsgHdrFlags) -> Result<usize> {
        let (bytes_recvd, _) = self.recvfrom(buf, flags, None)?;
        Ok(bytes_recvd)
    }

    pub fn recvfrom(&self, buf: &mut [u8], flags: MsgHdrFlags, src_addr: Option<&mut [u8]>) -> Result<(usize, usize)> {
        let (bytes_recvd, src_addr_len, _, _) = self.do_recvmsg(
            &mut buf[..],
            flags,
            src_addr,
            None,
        )?;
        Ok((bytes_recvd, src_addr_len))
    }*/

    pub fn recvmsg<'a, 'b>(&self, msg: &'b mut MsgHdrMut<'a>, flags: RecvFlags) -> Result<usize> {
        // Alloc untrusted iovecs to receive data via OCall
        let msg_iov = msg.get_iovs();
        let u_slice_alloc = UntrustedSliceAlloc::new(msg_iov.total_bytes())?;
        let mut u_slices = msg_iov
            .as_slices()
            .iter()
            .map(|slice| {
                u_slice_alloc
                    .new_slice_mut(slice.len())
                    .expect("unexpected out of memory error in UntrustedSliceAlloc")
            })
            .collect();
        let mut u_iovs = IovsMut::new(u_slices);

        // Do OCall-based recvmsg
        let (bytes_recvd, namelen_recvd, controllen_recvd, flags_recvd) = {
            // Acquire mutable references to the name and control buffers
            let (name, control) = msg.get_name_and_control_mut();
            // Fill the data, the name, and the control buffers
            self.do_recvmsg(u_iovs.as_slices_mut(), flags, name, control)?
        };

        // Update the output lengths and flags
        msg.set_name_len(namelen_recvd)?;
        msg.set_control_len(controllen_recvd)?;
        msg.set_flags(flags_recvd);

        // Copy data from untrusted iovecs into the output iovecs
        let mut msg_iov = msg.get_iovs_mut();
        let mut u_iovs_iter = u_iovs
            .as_slices()
            .iter()
            .flat_map(|slice| slice.iter())
            .take(bytes_recvd);
        msg_iov.copy_from_iter(&mut u_iovs_iter);

        Ok(bytes_recvd)
    }

    fn do_recvmsg(
        &self,
        data: &mut [&mut [u8]],
        flags: RecvFlags,
        mut name: Option<&mut [u8]>,
        mut control: Option<&mut [u8]>,
    ) -> Result<(usize, usize, usize, MsgHdrFlags)> {
        // Prepare the arguments for OCall
        // Host socket fd
        let host_fd = self.host_fd;
        // Name
        let (msg_name, msg_namelen) = name.as_mut_ptr_and_len();
        let msg_name = msg_name as *mut c_void;
        let mut msg_namelen_recvd = 0_u32;
        // Iovs
        let mut raw_iovs: Vec<libc::iovec> =
            data.iter().map(|slice| slice.as_libc_iovec()).collect();
        let (msg_iov, msg_iovlen) = raw_iovs.as_mut_slice().as_mut_ptr_and_len();
        // Control
        let (msg_control, msg_controllen) = control.as_mut_ptr_and_len();
        let msg_control = msg_control as *mut c_void;
        let mut msg_controllen_recvd = 0;
        // Flags
        let flags = flags.bits();
        let mut msg_flags_recvd = 0;

        // Do OCall
        let retval = try_libc!({
            let mut retval = 0_isize;
            let status = occlum_ocall_recvmsg(
                &mut retval as *mut isize,
                host_fd,
                msg_name,
                msg_namelen as u32,
                &mut msg_namelen_recvd as *mut u32,
                msg_iov,
                msg_iovlen,
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
            let max_bytes_recvd = data.iter().map(|x| x.len()).sum();
            assert!(retval <= max_bytes_recvd);
            retval
        };
        let msg_namelen_recvd = msg_namelen_recvd as usize;
        assert!(msg_namelen_recvd <= msg_namelen);
        assert!(msg_controllen_recvd <= msg_controllen);
        let flags_recvd = MsgHdrFlags::from_bits(msg_flags_recvd).unwrap();

        Ok((
            bytes_recvd,
            msg_namelen_recvd,
            msg_controllen_recvd,
            flags_recvd,
        ))
    }
}

extern "C" {
    fn occlum_ocall_recvmsg(
        ret: *mut ssize_t,
        fd: c_int,
        msg_name: *mut c_void,
        msg_namelen: libc::socklen_t,
        msg_namelen_recv: *mut libc::socklen_t,
        msg_data: *mut libc::iovec,
        msg_datalen: size_t,
        msg_control: *mut c_void,
        msg_controllen: size_t,
        msg_controllen_recv: *mut size_t,
        msg_flags: *mut c_int,
        flags: c_int,
    ) -> sgx_status_t;
}

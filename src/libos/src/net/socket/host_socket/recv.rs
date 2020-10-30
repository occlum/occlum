use super::*;
use crate::untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSliceAlloc};

impl HostSocket {
    pub fn recv(&self, buf: &mut [u8], flags: RecvFlags) -> Result<usize> {
        let (bytes_recvd, _) = self.recvfrom(buf, flags)?;
        Ok(bytes_recvd)
    }

    pub fn recvmsg<'a, 'b>(&self, msg: &'b mut MsgHdrMut<'a>, flags: RecvFlags) -> Result<usize> {
        // Do OCall-based recvmsg
        let (bytes_recvd, namelen_recvd, controllen_recvd, flags_recvd) = {
            // Acquire mutable references to the name and control buffers
            let (iovs, name, control) = msg.get_iovs_name_and_control_mut();
            // Fill the data, the name, and the control buffers
            self.do_recvmsg(iovs.as_slices_mut(), flags, name, control)?
        };

        // Update the output lengths and flags
        msg.set_name_len(namelen_recvd)?;
        msg.set_control_len(controllen_recvd)?;
        msg.set_flags(flags_recvd);

        Ok(bytes_recvd)
    }

    pub(super) fn do_recvmsg(
        &self,
        data: &mut [&mut [u8]],
        flags: RecvFlags,
        mut name: Option<&mut [u8]>,
        mut control: Option<&mut [u8]>,
    ) -> Result<(usize, usize, usize, MsgHdrFlags)> {
        let data_length = data.iter().map(|s| s.len()).sum();
        let u_allocator = UntrustedSliceAlloc::new(data_length)?;
        let mut u_data = {
            let mut bufs = Vec::new();
            for ref buf in data.iter() {
                bufs.push(u_allocator.new_slice_mut(buf.len())?);
            }
            bufs
        };
        let retval = self.do_recvmsg_untrusted_data(&mut u_data, flags, name, control)?;

        let mut remain = retval.0;
        for (i, buf) in data.iter_mut().enumerate() {
            if remain >= buf.len() {
                buf.copy_from_slice(u_data[i]);
                remain -= buf.len();
            } else {
                buf[0..remain].copy_from_slice(&u_data[i][0..remain]);
                break;
            }
        }
        Ok(retval)
    }

    fn do_recvmsg_untrusted_data(
        &self,
        data: &mut [&mut [u8]],
        flags: RecvFlags,
        mut name: Option<&mut [u8]>,
        mut control: Option<&mut [u8]>,
    ) -> Result<(usize, usize, usize, MsgHdrFlags)> {
        // Prepare the arguments for OCall
        // Host socket fd
        let host_fd = self.raw_host_fd() as i32;
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
        let raw_flags = flags.bits();
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
                raw_flags,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);

            // TODO: what if retval < 0 but buffers are modified by the
            // untrusted OCall? We reset the potentially tampered buffers.
            retval
        });

        let flags_recvd = MsgHdrFlags::from_bits(msg_flags_recvd).unwrap();

        // Check values returned from outside the enclave
        let bytes_recvd = {
            // Guarantted by try_libc!
            debug_assert!(retval >= 0);
            let retval = retval as usize;

            // Check bytes_recvd returned from outside the enclave
            let max_bytes_recvd = data.iter().map(|x| x.len()).sum();

            // For MSG_TRUNC recvmsg returns the real length of the packet or datagram,
            // even when it was longer than the passed buffer.
            if flags.contains(RecvFlags::MSG_TRUNC) && retval > max_bytes_recvd {
                assert!(flags_recvd.contains(MsgHdrFlags::MSG_TRUNC));
            } else {
                assert!(retval <= max_bytes_recvd);
            }
            retval
        };
        let msg_namelen_recvd = msg_namelen_recvd as usize;
        assert!(msg_namelen_recvd <= msg_namelen);
        assert!(msg_controllen_recvd <= msg_controllen);
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

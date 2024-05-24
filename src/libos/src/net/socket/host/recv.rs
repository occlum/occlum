use super::*;
use crate::untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSliceAlloc};

impl HostSocket {
    pub fn recv(&self, buf: &mut [u8], flags: RecvFlags) -> Result<usize> {
        let (bytes_recvd, _) = self.recvfrom(buf, flags)?;
        Ok(bytes_recvd)
    }

    pub fn recvmsg(
        &self,
        data: &mut [&mut [u8]],
        flags: RecvFlags,
        control: Option<&mut [u8]>,
    ) -> Result<(usize, Option<AnyAddr>, MsgFlags, usize)> {
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

        let mut u_data = {
            let mut bufs = Vec::new();
            for ref buf in data.iter() {
                let u_slice = u_allocator.new_slice_mut(buf.len())?;
                bufs.push(u_slice);
            }
            bufs
        };
        let retval = self.do_recvmsg_untrusted_data(&mut u_data, flags, control)?;

        let mut remain = retval.0;
        for (i, buf) in data.iter_mut().enumerate() {
            if remain >= buf.len() {
                u_data[i].write_to_slice(buf)?;
                remain -= buf.len();
            } else {
                u_data[i].write_to_slice(&mut buf[0..remain])?;
                break;
            }
        }
        Ok(retval)
    }

    fn do_recvmsg_untrusted_data(
        &self,
        data: &mut [UntrustedSlice],
        flags: RecvFlags,
        mut control: Option<&mut [u8]>,
    ) -> Result<(usize, Option<AnyAddr>, MsgFlags, usize)> {
        // Prepare the arguments for OCall
        let host_fd = self.raw_host_fd() as i32;
        let mut addr = SockAddr::default();
        let mut msg_name = addr.as_mut_ptr();
        let mut msg_namelen = addr.len();
        let mut msg_namelen_recvd = 0_u32;

        // Iovs
        let mut raw_iovs: Vec<libc::iovec> = data
            .iter()
            .map(|slice| slice.as_ref().as_libc_iovec())
            .collect();
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
                msg_name as _,
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

        let flags_recvd = MsgFlags::from_bits(msg_flags_recvd).unwrap();

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
                assert!(flags_recvd.contains(MsgFlags::MSG_TRUNC));
            } else {
                assert!(retval <= max_bytes_recvd);
            }
            retval
        };
        let msg_namelen_recvd = msg_namelen_recvd as usize;

        let raw_addr = (msg_namelen_recvd != 0).then(|| {
            addr.set_len(msg_namelen_recvd);
            addr
        });

        let addr = raw_addr.map(|addr| AnyAddr::Raw(addr));

        assert!(msg_namelen_recvd <= msg_namelen);
        assert!(msg_controllen_recvd <= msg_controllen);
        Ok((bytes_recvd, addr, flags_recvd, msg_controllen_recvd))
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

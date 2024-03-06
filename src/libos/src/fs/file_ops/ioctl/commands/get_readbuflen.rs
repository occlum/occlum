use super::*;

impl_ioctl_cmd! {
    pub struct GetReadBufLen<Input=(), Output=i32> {}
}

impl GetReadBufLen {
    pub fn execute(&mut self, host_fd: FileDesc) -> Result<()> {
        let mut buflen: i32 = 0;

        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::FIONREAD as i32,
                &mut buflen as *mut i32 as *mut c_void,
                std::mem::size_of::<i32>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });

        trace!("read buf len = {:?}", buflen);
        self.set_output(buflen);
        Ok(())
    }
}

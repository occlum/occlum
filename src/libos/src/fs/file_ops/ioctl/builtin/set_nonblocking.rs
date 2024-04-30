use super::*;

impl_ioctl_cmd! {
    pub struct SetNonBlocking<Input=i32, Output=()> {}
}

impl SetNonBlocking {
    pub fn execute(&mut self, host_fd: FileDesc) -> Result<()> {
        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::FIONBIO as i32,
                self.input() as *const i32 as *mut c_void,
                std::mem::size_of::<i32>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });

        Ok(())
    }
}

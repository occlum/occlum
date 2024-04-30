use super::*;

#[derive(Default, Clone, Copy, Debug)]
#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

impl_ioctl_cmd! {
    pub struct SetWinSize<Input=WinSize, Output=()> {}
}

impl SetWinSize {
    pub fn execute(&self, host_fd: FileDesc) -> Result<()> {
        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::TIOCSWINSZ as i32,
                self.input() as *const WinSize as *mut c_void,
                std::mem::size_of::<WinSize>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        Ok(())
    }
}

impl_ioctl_cmd! {
    pub struct GetWinSize<Input=(), Output=WinSize> {}
}

impl GetWinSize {
    pub fn execute(&mut self, host_fd: FileDesc) -> Result<()> {
        let mut winsize: WinSize = Default::default();

        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::TIOCGWINSZ as i32,
                &mut winsize as *mut WinSize as *mut c_void,
                std::mem::size_of::<WinSize>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });

        if winsize.ws_row == 0 || winsize.ws_col == 0 {
            warn!(
                "window size: row: {:?}, col: {:?}",
                winsize.ws_row, winsize.ws_col
            );
        }
        self.set_output(winsize);
        Ok(())
    }
}

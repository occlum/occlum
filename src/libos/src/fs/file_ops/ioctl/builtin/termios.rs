use super::*;

const NCCS: usize = 32;

type TcflagT = u32;
type CcT = u8;
type SpeedT = u32;

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct Termios {
    pub c_iflag: TcflagT,
    pub c_oflag: TcflagT,
    pub c_cflag: TcflagT,
    pub c_lflag: TcflagT,
    pub c_line: CcT,
    pub c_cc: [CcT; NCCS],
    pub c_ispeed: SpeedT,
    pub c_ospeed: SpeedT,
}

async_io::impl_ioctl_cmd! {
    pub struct TcGets<Input=(), Output=Termios> {}
}

impl TcGets {
    pub fn execute(&mut self, host_fd: FileDesc) -> Result<()> {
        let mut termios: Termios = Default::default();

        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::TCGETS as i32,
                &mut termios as *mut Termios as *mut c_void,
                std::mem::size_of::<Termios>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });

        self.set_output(termios);
        Ok(())
    }
}

async_io::impl_ioctl_cmd! {
    pub struct TcSets<Input=Termios, Output=()> {}
}

impl TcSets {
    pub fn execute(&self, host_fd: FileDesc) -> Result<()> {
        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::TCSETS as i32,
                self.input() as *const Termios as *mut c_void,
                std::mem::size_of::<Termios>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        Ok(())
    }
}

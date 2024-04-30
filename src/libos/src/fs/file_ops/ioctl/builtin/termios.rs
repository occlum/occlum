use super::*;

/*
 The termios structure used in the Linux kernel is not the same as we use in the glibc. Thus, we have two
 definitions here.
*/

const KERNEL_NCCS: usize = 19;
const NCCS: usize = 32;

type TcflagT = u32;
type CcT = u8;
type SpeedT = u32;

// Corresponds to the definition in glibc: sysdeps/unix/sysv/linux/kernel_termios.h
#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct KernelTermios {
    pub c_iflag: TcflagT,
    pub c_oflag: TcflagT,
    pub c_cflag: TcflagT,
    pub c_lflag: TcflagT,
    pub c_line: CcT,
    pub c_cc: [CcT; KERNEL_NCCS],
}

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

impl KernelTermios {
    fn to_termios(&self) -> Termios {
        let mut c_cc = [0; NCCS];
        c_cc[..KERNEL_NCCS].copy_from_slice(&self.c_cc);
        Termios {
            c_iflag: self.c_iflag,
            c_oflag: self.c_oflag,
            c_cflag: self.c_cflag,
            c_lflag: self.c_lflag,
            c_line: self.c_line,
            c_cc: c_cc,
            c_ispeed: 0,
            c_ospeed: 0,
        }
    }
}

impl Termios {
    fn to_kernel_termios(&self) -> KernelTermios {
        let mut c_cc = [0; KERNEL_NCCS];
        c_cc.copy_from_slice(&self.c_cc[..KERNEL_NCCS]);

        KernelTermios {
            c_iflag: self.c_iflag,
            c_oflag: self.c_oflag,
            c_cflag: self.c_cflag,
            c_lflag: self.c_lflag,
            c_line: self.c_line,
            c_cc: c_cc,
        }
    }
}

impl_ioctl_cmd! {
    pub struct TcGets<Input=(), Output=KernelTermios> {}
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

        let kernel_termios = termios.to_kernel_termios();
        trace!("kernel termios = {:?}", kernel_termios);
        self.set_output(kernel_termios);
        Ok(())
    }
}

impl_ioctl_cmd! {
    pub struct TcSets<Input=KernelTermios, Output=()> {}
}

impl TcSets {
    pub fn execute(&self, host_fd: FileDesc) -> Result<()> {
        let kernel_termios = self.input();
        let termios = kernel_termios.to_termios();
        try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd as i32,
                BuiltinIoctlNum::TCSETS as i32,
                &termios as *const Termios as *mut c_void,
                std::mem::size_of::<Termios>(),
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        Ok(())
    }
}

//! Define builtin ioctls and provide utilities for non-builtin ioctls.
//!
//! A builtin ioctl is defined as part of the OS kernel and is used by various
//! OS sub-system. In contrast, an non-builtin ioctl is specific to a device or
//! driver.

use super::*;

pub use self::builtin::*;
pub use self::non_builtin::{NonBuiltinIoctlCmd, StructuredIoctlArgType, StructuredIoctlNum};

#[macro_use]
mod macros;
mod builtin;
mod non_builtin;

/// This is the centralized place to define built-in ioctls.
///
/// By giving the names, numbers, and argument types of built-in ioctls,
/// the macro below generates the corresponding code of `BuiltinIoctlNum` and
/// `IoctlCmd`.
///
/// To add a new built-in ioctl, just follow the convention as shown
/// by existing built-in ioctls.
impl_ioctl_nums_and_cmds! {
    // Format:
    // ioctl_name => (ioctl_num, ioctl_type_arg)

    // Get window size
    TIOCGWINSZ => (0x5413, mut WinSize),
    // Set window size
    TIOCSWINSZ => (0x5414, WinSize),
    // Set the nonblocking mode for socket
    FIONBIO => (0x5421, i32),
    // If the given terminal was the controlling terminal of the calling process, give up this
    // controlling terminal. If the process was session leader, then send SIGHUP and SIGCONT to
    // the foreground process group and all processes in the current session lose their controlling
    // terminal
    TIOCNOTTY => (0x5422, ()),
    // Get the number of bytes in the input buffer
    FIONREAD => (0x541B, mut i32),
    // Low-level access to Linux network devices on man7/netdevice.7
    // Only non-privileged operations are supported for now
    SIOCGIFNAME => (0x8910, mut IfReq),
    SIOCGIFCONF => (0x8912, mut IfConf),
    SIOCGIFFLAGS => (0x8913, mut IfReq),
    SIOCGIFADDR => (0x8915, mut IfReq),
    SIOCGIFDSTADDR => (0x8917, mut IfReq),
    SIOCGIFBRDADDR => (0x8919, mut IfReq),
    SIOCGIFNETMASK => (0x891B, mut IfReq),
    SIOCGIFMTU => (0x8921, mut IfReq),
    SIOCGIFHWADDR => (0x8927, mut IfReq),
    SIOCGIFINDEX => (0x8933, mut IfReq),
    SIOCGIFPFLAGS => (0x8935, mut IfReq),
    SIOCGIFTXQLEN => (0x8942, mut IfReq),
    SIOCGIFMAP => (0x8970, mut IfReq),
}

/// This is the centralized place to add sanity checks for the argument values
/// of built-in ioctls.
///
/// Sanity checks are mostly useful when the argument values are returned from
/// the untrusted host OS.
impl<'a> IoctlRawCmd<'a> {
    pub fn to_safe_ioctlcmd(&self) -> Result<Box<dyn IoctlCmd>> {
        match self {
            IoctlRawCmd::TIOCGWINSZ(_) => Ok(Box::new(GetWinSize::new(()))),
            IoctlRawCmd::TIOCSWINSZ(winsize_ref) => {
                let winsize = **winsize_ref;
                Ok(Box::new(SetWinSize::new(winsize)))
            }
            IoctlRawCmd::NonBuiltin(inner) => {
                let nonbuiltin_cmd =
                    unsafe { NonBuiltinIoctlCmd::new(*inner.cmd_num(), inner.arg_ptr() as _)? };
                Ok(Box::new(nonbuiltin_cmd))
            }
            _ => {
                return_errno!(EINVAL, "unsupported cmd");
            }
        }
    }

    pub fn copy_output_from_safe(&mut self, cmd: &dyn IoctlCmd) {
        match self {
            IoctlRawCmd::TIOCGWINSZ(winsize_mut) => {
                let cmd = cmd.downcast_ref::<GetWinSize>().unwrap();
                **winsize_mut = *cmd.output().unwrap();
            }
            _ => {}
        }
    }
}

pub fn do_ioctl(fd: FileDesc, raw_cmd: &mut IoctlRawCmd) -> Result<i32> {
    debug!("ioctl: fd: {}, cmd: {:?}", fd, raw_cmd);
    let file_ref = current!().file(fd)?;
    let mut cmd = raw_cmd.to_safe_ioctlcmd()?;
    file_ref.ioctl(cmd.as_mut())?;
    raw_cmd.copy_output_from_safe(cmd.as_ref());
    Ok(0)
}

extern "C" {
    pub fn occlum_ocall_ioctl(
        ret: *mut i32,
        fd: c_int,
        request: c_int,
        arg: *mut c_void,
        len: size_t,
    ) -> sgx_status_t;
}

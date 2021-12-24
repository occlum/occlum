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

    // Get terminal attributes
    TCGETS => (0x5401, mut KernelTermios), // ignore
    TCSETS => (0x5402, KernelTermios),
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
    // Don't close on exec
    FIONCLEX => (0x5450, ()),
    // Set close on exec
    FIOCLEX => (0x5451, ()),
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
impl<'a> IoctlCmd<'a> {
    pub fn validate_arg_and_ret_vals(&self, ret: i32) -> Result<()> {
        match self {
            IoctlCmd::TIOCGWINSZ(winsize_ref) => {
                // ws_row and ws_col are usually not zeros
                if winsize_ref.ws_row == 0 || winsize_ref.ws_col == 0 {
                    warn!(
                        "window size: row: {:?}, col: {:?}",
                        winsize_ref.ws_row, winsize_ref.ws_col
                    );
                }
            }
            IoctlCmd::FIONREAD(nread_ref) => {
                if (**nread_ref < 0) {
                    return_errno!(EINVAL, "invalid data from host");
                }
            }
            _ => {}
        }

        // Current ioctl commands all return zero
        if ret != 0 {
            return_errno!(EINVAL, "return value should be zero");
        }
        Ok(())
    }
}

pub fn do_ioctl(fd: FileDesc, cmd: &mut IoctlCmd) -> Result<i32> {
    debug!("ioctl: fd: {}, cmd: {:?}", fd, cmd);
    let current = current!();
    let file_ref = current.file(fd)?;
    let mut file_table = current.files().lock().unwrap();
    let mut entry = file_table.get_entry_mut(fd)?;
    match cmd {
        IoctlCmd::FIONCLEX(_) => {
            entry.set_close_on_spawn(false);
            return Ok(0);
        }
        IoctlCmd::FIOCLEX(_) => {
            entry.set_close_on_spawn(true);
            return Ok(0);
        }
        _ => return file_ref.ioctl(cmd),
    }
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

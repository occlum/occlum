//! Define builtin ioctls and provide utilities for non-builtin ioctls.
//!
//! A builtin ioctl is defined as part of the OS kernel and is used by various
//! OS sub-system. In contrast, an non-builtin ioctl is specific to a device or
//! driver.

use super::*;

pub use self::builtin::WinSize;
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
    // If the given terminal was the controlling terminal of the calling process, give up this
    // controlling terminal. If the process was session leader, then send SIGHUP and SIGCONT to
    // the foreground process group and all processes in the current session lose their controlling
    // terminal
    TIOCNOTTY => (0x5422, ()),
    // Get the number of bytes in the input buffer
    FIONREAD => (0x541B, mut i32),
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
    let file_ref = current!().file(fd)?;
    file_ref.ioctl(cmd)
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

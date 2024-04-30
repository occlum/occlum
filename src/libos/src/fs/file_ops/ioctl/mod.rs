//! Define builtin ioctls and provide utilities for non-builtin ioctls.
//!
//! A builtin ioctl is defined as part of the OS kernel and is used by various
//! OS sub-system. In contrast, an non-builtin ioctl is specific to a device or
//! driver.

use super::*;

use self::builtin::*;
pub use self::builtin::{
    GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, GetWinSize, IfConf, IoctlCmd, SetNonBlocking,
    SetWinSize, TcGets, TcSets,
};
pub use self::non_builtin::{NonBuiltinIoctlCmd, StructuredIoctlArgType, StructuredIoctlNum};
use crate::util::mem_util::from_user;

#[macro_use]
mod macros;
mod builtin;
mod non_builtin;

/// This is the centralized place to define built-in ioctls.
///
/// By giving the names, numbers, and argument types of built-in ioctls,
/// the macro below generates the corresponding code of `BuiltinIoctlNum` and
/// `IoctlRawCmd`.
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
impl<'a> IoctlRawCmd<'a> {
    pub fn to_safe_ioctlcmd(&self) -> Result<Box<dyn IoctlCmd>> {
        Ok(match self {
            IoctlRawCmd::TCGETS(_) => Box::new(TcGets::new(())),
            IoctlRawCmd::TCSETS(termios_ref) => {
                let termios = **termios_ref;
                Box::new(TcSets::new(termios))
            }
            IoctlRawCmd::TIOCGWINSZ(_) => Box::new(GetWinSize::new(())),
            IoctlRawCmd::TIOCSWINSZ(winsize_ref) => {
                let winsize = **winsize_ref;
                Box::new(SetWinSize::new(winsize))
            }
            IoctlRawCmd::NonBuiltin(inner) => {
                let nonbuiltin_cmd =
                    unsafe { NonBuiltinIoctlCmd::new(*inner.cmd_num(), inner.arg_ptr() as _)? };
                Box::new(nonbuiltin_cmd)
            }
            IoctlRawCmd::FIONBIO(non_blocking) => Box::new(SetNonBlocking::new(**non_blocking)),
            IoctlRawCmd::FIONREAD(_) => Box::new(GetReadBufLen::new(())),
            IoctlRawCmd::FIONCLEX(_) => Box::new(SetCloseOnExec::new(false)),
            IoctlRawCmd::FIOCLEX(_) => Box::new(SetCloseOnExec::new(true)),
            IoctlRawCmd::SIOCGIFCONF(ifconf_mut) => {
                if !ifconf_mut.ifc_buf.is_null() {
                    if ifconf_mut.ifc_len < 0 {
                        return_errno!(EINVAL, "invalid ifc_len");
                    }
                    from_user::check_array(ifconf_mut.ifc_buf, ifconf_mut.ifc_len as usize)?;
                }
                Box::new(GetIfConf::new(ifconf_mut))
            }
            IoctlRawCmd::SIOCGIFFLAGS(req)
            | IoctlRawCmd::SIOCGIFNAME(req)
            | IoctlRawCmd::SIOCGIFADDR(req)
            | IoctlRawCmd::SIOCGIFDSTADDR(req)
            | IoctlRawCmd::SIOCGIFBRDADDR(req)
            | IoctlRawCmd::SIOCGIFNETMASK(req)
            | IoctlRawCmd::SIOCGIFMTU(req)
            | IoctlRawCmd::SIOCGIFHWADDR(req)
            | IoctlRawCmd::SIOCGIFINDEX(req)
            | IoctlRawCmd::SIOCGIFPFLAGS(req)
            | IoctlRawCmd::SIOCGIFTXQLEN(req)
            | IoctlRawCmd::SIOCGIFMAP(req) => {
                Box::new(GetIfReqWithRawCmd::new(self.cmd_num(), **req))
            }
            _ => {
                return_errno!(EINVAL, "unsupported cmd");
            }
        })
    }

    pub fn copy_output_from_safe(&mut self, cmd: &dyn IoctlCmd) {
        match self {
            IoctlRawCmd::TCGETS(termios_mut) => {
                let cmd = cmd.downcast_ref::<TcGets>().unwrap();
                **termios_mut = *cmd.output().unwrap();
            }
            IoctlRawCmd::TIOCGWINSZ(winsize_mut) => {
                let cmd = cmd.downcast_ref::<GetWinSize>().unwrap();
                **winsize_mut = *cmd.output().unwrap();
            }
            IoctlRawCmd::FIONREAD(len_mut) => {
                let cmd = cmd.downcast_ref::<GetReadBufLen>().unwrap();
                **len_mut = *cmd.output().unwrap();
            }
            IoctlRawCmd::SIOCGIFCONF(ifconf_mut) => {
                let cmd = cmd.downcast_ref::<GetIfConf>().unwrap();
                ifconf_mut.ifc_len = cmd.len() as i32;
                if !ifconf_mut.ifc_buf.is_null() {
                    let mut raw_buf = unsafe {
                        std::slice::from_raw_parts_mut(
                            ifconf_mut.ifc_buf as _,
                            ifconf_mut.ifc_len as _,
                        )
                    };
                    raw_buf.copy_from_slice(cmd.as_slice().unwrap());
                }
            }
            IoctlRawCmd::SIOCGIFNAME(ifreq_mut)
            | IoctlRawCmd::SIOCGIFFLAGS(ifreq_mut)
            | IoctlRawCmd::SIOCGIFADDR(ifreq_mut)
            | IoctlRawCmd::SIOCGIFDSTADDR(ifreq_mut)
            | IoctlRawCmd::SIOCGIFBRDADDR(ifreq_mut)
            | IoctlRawCmd::SIOCGIFNETMASK(ifreq_mut)
            | IoctlRawCmd::SIOCGIFMTU(ifreq_mut)
            | IoctlRawCmd::SIOCGIFHWADDR(ifreq_mut)
            | IoctlRawCmd::SIOCGIFINDEX(ifreq_mut)
            | IoctlRawCmd::SIOCGIFPFLAGS(ifreq_mut)
            | IoctlRawCmd::SIOCGIFTXQLEN(ifreq_mut)
            | IoctlRawCmd::SIOCGIFMAP(ifreq_mut) => {
                let cmd = cmd.downcast_ref::<GetIfReqWithRawCmd>().unwrap();
                **ifreq_mut = *cmd.output().unwrap();
            }
            _ => {}
        }
    }
}

pub fn do_ioctl(fd: FileDesc, raw_cmd: &mut IoctlRawCmd<'_>) -> Result<i32> {
    debug!("ioctl: fd: {}, cmd: {:?}", fd, raw_cmd);
    let current = current!();
    let file_ref = current.file(fd)?;
    let mut cmd = raw_cmd.to_safe_ioctlcmd()?;

    if cmd.is::<SetCloseOnExec>() {
        let is_close_on_exec = cmd.downcast_ref::<SetCloseOnExec>().unwrap().input();
        let mut file_table = current.files().lock();
        let entry = file_table.get_entry_mut(fd)?;
        entry.set_close_on_spawn(*is_close_on_exec);
        return Ok(0);
    }

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

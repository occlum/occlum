use alloc::string::String;
use alloc::vec::Vec;
use std::ffi::CString;
use std::os::raw::c_char;

use super::super::time::timer_slack::TIMERSLACK;
use super::thread::ThreadName;
use crate::prelude::*;
use crate::util::mem_util::from_user::{check_array, clone_cstring_safely};

#[macro_use]
mod macros;

// Note:
// PrctlCmd has implied lifetime parameter `'a`
impl_prctl_nums_and_cmds! {
    // Format:
    // prctl_name => (prctl_num, prctl_type_arg, ...
    PR_SET_NAME => (15, ThreadName),
    PR_GET_NAME => (16, (&'a mut [u8])),
    PR_SET_TIMERSLACK => (29, u64),
    PR_GET_TIMERSLACK => (30, ()),
}

impl<'a> PrctlCmd<'a> {
    pub fn from_raw(cmd: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> Result<PrctlCmd<'a>> {
        Ok(match cmd {
            PR_SET_NAME => {
                check_array(arg2 as *const u8, ThreadName::max_len())?;
                let raw_name =
                    unsafe { std::slice::from_raw_parts(arg2 as *const u8, ThreadName::max_len()) };
                let name = ThreadName::from_slice(raw_name);
                PrctlCmd::PR_SET_NAME(name)
            }
            PR_GET_NAME => {
                let buf_checked = {
                    check_array(arg2 as *mut u8, ThreadName::max_len())?;
                    unsafe {
                        std::slice::from_raw_parts_mut(arg2 as *mut u8, ThreadName::max_len())
                    }
                };
                PrctlCmd::PR_GET_NAME(buf_checked)
            }
            PR_SET_TIMERSLACK => PrctlCmd::PR_SET_TIMERSLACK(arg2),
            PR_GET_TIMERSLACK => PrctlCmd::PR_GET_TIMERSLACK(()),
            _ => {
                debug!("prctl cmd num: {}", cmd);
                return_errno!(EINVAL, "unsupported prctl command");
            }
        })
    }
}

pub fn do_prctl(cmd: PrctlCmd) -> Result<isize> {
    debug!("prctl: {:?}", cmd);

    let current = current!();
    match cmd {
        PrctlCmd::PR_SET_NAME(name) => {
            current.set_name(name);
        }
        PrctlCmd::PR_GET_NAME(c_buf) => {
            let name = current.name();
            c_buf.copy_from_slice(name.as_slice());
        }
        PrctlCmd::PR_SET_TIMERSLACK(nanoseconds) => {
            return_errno!(
                EINVAL,
                "Setting timer slack for different libos process is not supported"
            );
        }
        PrctlCmd::PR_GET_TIMERSLACK(()) => {
            let nanoseconds = (*TIMERSLACK).to_u32();
            return Ok(nanoseconds as isize);
        }
        _ => return_errno!(EINVAL, "Prctl command not supported"),
    }

    Ok(0)
}

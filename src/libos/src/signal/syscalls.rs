use std::time::Duration;

use super::constants::*;
use super::do_sigprocmask::MaskOp;
use super::signals::FaultSignal;
use super::{sigaction_t, siginfo_t, sigset_t, stack_t, SigAction, SigNum, SigSet, SigStack};
use crate::prelude::*;
use crate::process::ProcessFilter;
use crate::syscall::CpuContext;
use crate::time::timespec_t;
use crate::util::mem_util::from_user;

pub fn do_rt_sigaction(
    signum_c: c_int,
    new_sa_c: *const sigaction_t,
    old_sa_c: *mut sigaction_t,
) -> Result<isize> {
    // C types -> Rust types
    let signum = SigNum::from_u8(signum_c as u8)?;
    let new_sa = {
        if !new_sa_c.is_null() {
            let new_sa_c = unsafe { &*new_sa_c };
            let new_sa = SigAction::from_c(new_sa_c)?;
            Some(new_sa)
        } else {
            None
        }
    };
    let mut old_sa_c = {
        if !old_sa_c.is_null() {
            let old_sa_c = unsafe { &mut *old_sa_c };
            Some(old_sa_c)
        } else {
            None
        }
    };

    // Do sigaction
    let old_sa = super::do_sigaction::do_rt_sigaction(signum, new_sa)?;

    // Retrieve old sigaction_t, if needed
    if let Some(old_sa_c) = old_sa_c {
        *old_sa_c = old_sa.to_c();
    }
    Ok(0)
}

pub fn do_rt_sigreturn(user_context: *mut CpuContext) -> Result<isize> {
    let user_context = unsafe { &mut *user_context };
    super::do_sigreturn::do_rt_sigreturn(user_context)?;
    Ok(0)
}

pub fn do_kill(pid: i32, sig: c_int) -> Result<isize> {
    let process_filter = match pid {
        pid if pid < -1 => ProcessFilter::WithPgid((-pid) as pid_t),
        -1 => ProcessFilter::WithAnyPid,
        0 => {
            let pgid = current!().process().pgid();
            ProcessFilter::WithPgid(pgid)
        }
        pid if pid > 0 => ProcessFilter::WithPid(pid as pid_t),
        _ => unreachable!(),
    };
    let signum = SigNum::from_u8(sig as u8)?;
    super::do_kill::do_kill(process_filter, signum)?;
    Ok(0)
}

pub fn do_tkill(tid: pid_t, sig: c_int) -> Result<isize> {
    let signum = SigNum::from_u8(sig as u8)?;
    super::do_kill::do_tgkill(None, tid, signum)?;
    Ok(0)
}

pub fn do_tgkill(pid: i32, tid: pid_t, sig: c_int) -> Result<isize> {
    let pid = if pid >= 0 { Some(pid as pid_t) } else { None };
    let signum = SigNum::from_u8(sig as u8)?;
    super::do_kill::do_tgkill(pid, tid, signum)?;
    Ok(0)
}

pub fn do_rt_sigprocmask(
    how: c_int,
    set_ptr: *const sigset_t,
    oldset_ptr: *mut sigset_t,
    sigset_size: usize,
) -> Result<isize> {
    if sigset_size != std::mem::size_of::<sigset_t>() {
        return_errno!(EINVAL, "unexpected sigset size");
    }
    let op_and_set = {
        if !set_ptr.is_null() {
            let op = MaskOp::from_u32(how as u32)?;
            let set = unsafe { &*set_ptr };
            Some((op, set))
        } else {
            None
        }
    };
    let old_set = {
        if !oldset_ptr.is_null() {
            Some(unsafe { &mut *oldset_ptr })
        } else {
            None
        }
    };
    super::do_sigprocmask::do_rt_sigprocmask(op_and_set, old_set)?;
    Ok(0)
}

pub fn do_rt_sigpending(buf_ptr: *mut sigset_t, buf_size: usize) -> Result<isize> {
    let buf: &mut sigset_t = {
        if buf_size < std::mem::size_of::<sigset_t>() {
            return_errno!(EINVAL, "buf is not big enough");
        }
        if buf_ptr.is_null() {
            return_errno!(EINVAL, "ptr must not be null");
        }
        unsafe { &mut *buf_ptr }
    };
    let pending = super::do_sigpending::do_sigpending()?;
    *buf = pending.to_c();
    Ok(0)
}

pub fn do_sigaltstack(
    new_ss_c: *const stack_t,
    old_ss_c: *mut stack_t,
    user_context: *const CpuContext,
) -> Result<isize> {
    // C types -> Rust types
    let new_ss = {
        if !new_ss_c.is_null() {
            from_user::check_ptr(new_ss_c)?;
            let new_ss_c = unsafe { &*new_ss_c };
            let new_ss = SigStack::from_c(new_ss_c)?;
            Some(new_ss)
        } else {
            None
        }
    };
    let mut old_ss_c = {
        if !old_ss_c.is_null() {
            from_user::check_mut_ptr(old_ss_c)?;
            let old_ss_c = unsafe { &mut *old_ss_c };
            Some(old_ss_c)
        } else {
            None
        }
    };
    let user_context = unsafe { &*user_context };

    // Do sigaltstack
    let old_ss = super::do_sigaltstack::do_sigaltstack(&new_ss, user_context)?;

    // Retrieve old signal stack, if needed
    if let Some(old_ss_c) = old_ss_c {
        *old_ss_c = old_ss.to_c();
    }
    Ok(0)
}

pub fn do_rt_sigtimedwait(
    mask_ptr: *const sigset_t,
    info_ptr: *mut siginfo_t,
    timeout_ptr: *const timespec_t,
    mask_size: usize,
) -> Result<isize> {
    let mask: SigSet = {
        if mask_size < std::mem::size_of::<sigset_t>() {
            return_errno!(EINVAL, "mask size is not big enough");
        }
        if mask_ptr.is_null() {
            return_errno!(EINVAL, "ptr must not be null");
        }
        SigSet::from_c(unsafe { *mask_ptr })
    };
    let info: &mut siginfo_t = {
        if info_ptr.is_null() {
            return_errno!(EINVAL, "ptr must not be null");
        }
        unsafe { &mut *info_ptr }
    };
    let timeout: Option<Duration> = {
        if timeout_ptr.is_null() {
            None
        } else {
            let timeout = timespec_t::from_raw_ptr(timeout_ptr)?;
            Some(timeout.as_duration())
        }
    };

    *info = super::do_sigtimedwait::do_sigtimedwait(mask, timeout.as_ref())?;
    Ok(0)
}

use super::*;
use crate::util::mem_util::from_user::*;
use std::convert::TryFrom;

pub async fn do_timerfd_create(clockid: clockid_t, flags: i32) -> Result<isize> {
    debug!("clockid {}, flags {} ", clockid, flags);

    let clockid = ClockId::try_from(clockid)?;
    match clockid {
        crate::time::ClockId::CLOCK_REALTIME | crate::time::ClockId::CLOCK_MONOTONIC => {}
        _ => {
            return_errno!(EINVAL, "invalid clockid");
        }
    }
    let timer_create_flags =
        TimerCreationFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;

    let file_ref = {
        let timer = TimerFile::new(clockid, timer_create_flags)?;
        FileRef::new_timer(timer)
    };

    let fd = current!().add_file(
        file_ref,
        timer_create_flags.contains(TimerCreationFlags::TFD_CLOEXEC),
    );
    Ok(fd as isize)
}

pub async fn do_timerfd_settime(
    fd: FileDesc,
    flags: i32,
    new_value_ptr: *const itimerspec_t,
    old_value_ptr: *mut itimerspec_t,
) -> Result<isize> {
    check_ptr(new_value_ptr)?;
    let new_value = itimerspec_t::from_raw_ptr(new_value_ptr)?;
    let timer_set_flags =
        TimerSetFlags::from_bits(flags).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;

    let file_ref = current!().file(fd as FileDesc)?;
    let timerfd = file_ref
        .as_timer_file()
        .ok_or_else(|| errno!(EINVAL, "not a timer fd"))?;

    let new_value: TimerfileDurations = new_value.into();
    let old_value = itimerspec_t::from(timerfd.set_time(timer_set_flags, &new_value)?);
    debug!(
        "flags {}, new value {:?}, old value {:?} ",
        flags, new_value, old_value
    );
    if !old_value_ptr.is_null() {
        check_mut_ptr(old_value_ptr)?;
        unsafe {
            old_value_ptr.write(old_value);
        }
    }
    Ok(0)
}

pub async fn do_timerfd_gettime(fd: FileDesc, curr_value_ptr: *mut itimerspec_t) -> Result<isize> {
    check_mut_ptr(curr_value_ptr)?;
    let file_ref = current!().file(fd as FileDesc)?;
    let timerfd = file_ref
        .as_timer_file()
        .ok_or_else(|| errno!(EINVAL, "not a timer fd"))?;
    let curr_value = itimerspec_t::from(timerfd.time()?);
    debug!("current value {:?}", curr_value);
    unsafe {
        curr_value_ptr.write(curr_value);
    }
    Ok(0)
}

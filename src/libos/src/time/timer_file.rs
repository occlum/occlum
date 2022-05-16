use super::*;
use crate::fs::{AccessMode, Events, IoctlCmd, Observer, Pollee, Poller, StatusFlags};
use async_rt::task::{JoinHandle, SpawnOptions};
use async_rt::{wait::WaiterQueue, waiter_loop};
use atomic::{Atomic, Ordering};
use std::sync::{Arc, SgxMutex as Mutex};
use std::time::Duration;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
enum TimerFdStatus {
    STOP = 0,
    RUN = 1,
}

#[derive(Debug)]
pub struct TimerFile {
    clockid: ClockId,
    flags: Atomic<TimerCreationFlags>,
    inner: Arc<Mutex<TimerInner>>,
    // Waiter queue to wait for the expired event
    exp_waiters: Arc<WaiterQueue>,
    pollee: Arc<Pollee>,
}

#[derive(Debug)]
struct TimerInner {
    // Initial expiration
    it_value: Duration,
    // Timer interval
    interval: Duration,
    // Next expected expire time, absolute time
    next_exp: Duration,
    // Timer expired count, +1 for every expire
    // Reset to 0 for read
    exp_cnt: usize,
    status: TimerFdStatus,
    // if true, timerfd started with just once expire
    one_shot: bool,
    task_handle: Option<JoinHandle<()>>,
}

impl TimerInner {
    pub fn new() -> Self {
        Self {
            it_value: Duration::default(),
            interval: Duration::default(),
            next_exp: Duration::default(),
            exp_cnt: 0,
            status: TimerFdStatus::STOP,
            one_shot: false,
            task_handle: None,
        }
    }
}

impl Drop for TimerFile {
    fn drop(&mut self) {
        trace!("TimerFile Drop");
        let inner_c = self.inner.clone();
        let mut inner = inner_c.lock().unwrap();
        if inner.task_handle.is_some() {
            inner
                .task_handle
                .as_ref()
                .unwrap()
                .task()
                .tirqs()
                .put_req(0);
        }
    }
}

impl TimerFile {
    pub fn new(clockid: ClockId, flags: TimerCreationFlags) -> Result<Self> {
        Ok(Self {
            clockid,
            flags: Atomic::new(flags),
            inner: Arc::new(Mutex::new(TimerInner::new())),
            exp_waiters: Arc::new(WaiterQueue::new()),
            pollee: Arc::new(Pollee::new(Events::empty())),
        })
    }

    // The inner implementation of syscall timerfd_settime.
    pub fn set_time(
        &self,
        flags: TimerSetFlags,
        new_value: &TimerfileDurations,
    ) -> Result<TimerfileDurations> {
        let cur_time =
            timespec_t::from(vdso_time::clock_gettime(self.clockid).unwrap()).as_duration();

        let inner_c = self.inner.clone();
        let mut inner = inner_c.lock().unwrap();
        let ret = TimerfileDurations {
            it_interval: inner.interval,
            it_value: inner.it_value,
        };

        // Check if the it_value is 0 which means stop the timer
        if new_value.it_value.is_zero() == true {
            debug!("TimerFd: stop timer");
            inner.status = TimerFdStatus::STOP;
            if inner.task_handle.is_some() {
                // If timer task started, send signal to end the task
                inner
                    .task_handle
                    .as_ref()
                    .unwrap()
                    .task()
                    .tirqs()
                    .put_req(0);
            }

            inner.task_handle = None;

            return Ok(ret);
        }

        // Transfer the initial expired time to absolute time
        let exp_time = match flags {
            TimerSetFlags::TFD_TIMER_ABSTIME => new_value.it_value,
            _ => cur_time.checked_add(new_value.it_value).unwrap(),
        };

        // Transfer the initial expired time to relative duration time.
        // The relative time it_value will be used in wait_timeout.
        let it_value = match exp_time.checked_sub(cur_time) {
            Some(duration) => duration,
            _ => Duration::new(0, 0),
        };

        inner.interval = new_value.it_interval;
        inner.it_value = it_value;
        inner.exp_cnt = 0;
        inner.next_exp = exp_time;
        inner.one_shot = new_value.it_interval.is_zero();

        // If no timer task started, start it
        if inner.task_handle.is_none() {
            // Drop Mutex inner here.
            // Will acquire it in fn self.start_timer_task again.
            drop(inner);
            self.start_timer_task(it_value);
        }

        return Ok(ret);
    }

    // The inner implementation of syscall timerfd_gettime.
    pub fn time(&self) -> Result<TimerfileDurations> {
        let mut ret_time = TimerfileDurations::default();
        let inner_c = self.inner.clone();
        let mut inner = inner_c.lock().unwrap();
        let status = inner.status;
        let interval = inner.interval;
        let exp_cnt = inner.exp_cnt;
        let next_exp = inner.next_exp;

        // Do not need it anymore
        drop(inner);

        ret_time.it_interval = interval;

        // Return if timer stop
        if status == TimerFdStatus::STOP {
            ret_time.it_value = Duration::new(0, 0);
            return Ok(ret_time);
        }

        let cur_time =
            timespec_t::from(vdso_time::clock_gettime(self.clockid).unwrap()).as_duration();

        match next_exp.checked_sub(cur_time) {
            Some(left) => ret_time.it_value = left,
            // Error
            _ => {
                return_errno!(EAGAIN, "Timerfd not started");
            }
        };

        Ok(ret_time)
    }

    fn start_timer_task(&self, it_value: Duration) -> Result<()> {
        let mut timeout = it_value;
        let inner_c = self.inner.clone();
        let mut inner = inner_c.lock().unwrap();
        let interval = inner.interval;

        // Check if the async loop block needs break (one_shot = true)
        let one_shot = inner.one_shot;

        // Start background poll task to monitor timerfd events
        let join_handle = SpawnOptions::new({
            let pollee = self.pollee.clone();
            let exp_waiters = self.exp_waiters.clone();
            let inner_c = self.inner.clone();

            let expired_closure = move |inner_c: &Arc<Mutex<TimerInner>>, is_stop| {
                let mut inner = inner_c.lock().unwrap();
                inner.exp_cnt += 1;

                if is_stop {
                    inner.status = TimerFdStatus::STOP;
                    inner.task_handle = None;
                } else {
                    // Set next expire time
                    inner.next_exp = inner.next_exp.checked_add(timeout).unwrap();
                }

                pollee.add_events(Events::IN);
                exp_waiters.wake_all();
            };

            async move {
                loop {
                    let waiter = Waiter::new();
                    // The initial timeout is the it_value.
                    let res = waiter.wait_timeout(Some(&mut timeout)).await;
                    match res {
                        Err(e) => {
                            if e.errno() == EINTR {
                                // Error or Stopped by tirq
                                break;
                            } else if one_shot == true {
                                // One-shot timer, no need loop
                                trace!("timerfd one-shot triggerred");
                                expired_closure(&inner_c, true);
                                break;
                            } else {
                                trace!("timerfd timer expired");
                                // If it is not one-shot, the following timeout changes to interval.
                                timeout = interval;
                                expired_closure(&inner_c, false);
                            }
                        }
                        _ => {
                            panic!("impossible as there is no waker or timeout");
                        }
                    }
                }

                trace!("Timerfd poll task end");
            }
        })
        .priority(async_rt::sched::SchedPriority::Low)
        .spawn();

        inner.status = TimerFdStatus::RUN;
        inner.task_handle = Some(join_handle);

        Ok(())
    }
}

bitflags! {
    pub struct TimerCreationFlags: i32 {
        /// Provides semaphore-like semantics for reads from the new file descriptor
        /// Non-blocking
        const TFD_NONBLOCK  = 1 << 11;
        /// Close on exec
        const TFD_CLOEXEC   = 1 << 19;
    }
}

bitflags! {
    pub struct TimerSetFlags: i32 {
        const TFD_TIMER_ABSTIME = 1 << 0;
        const TFD_TIMER_CANCEL_ON_SET = 1 << 1;
    }
}

impl TimerFile {
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() < 8 {
            return_errno!(EINVAL, "buffer is too small");
        }

        let flags = self.flags.load(Ordering::Relaxed);
        let inner_c = self.inner.clone();
        let exp_waiters = &self.exp_waiters.clone();
        waiter_loop!(exp_waiters, {
            let mut inner = inner_c.lock().unwrap();

            if inner.exp_cnt > 0 {
                let count = inner.exp_cnt;
                // Reset expired count
                inner.exp_cnt = 0;
                let bytes = count.to_ne_bytes();
                let buf = &mut buf[0..8];
                buf.copy_from_slice(&bytes);

                self.pollee.del_events(Events::IN);
                return Ok(buf.len());
            }

            if inner.status == TimerFdStatus::STOP
                || flags.contains(TimerCreationFlags::TFD_NONBLOCK)
            {
                return_errno!(EAGAIN, "try again");
            }
        })
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        return_errno!(EINVAL, "timer fds do not support readv");
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        return_errno!(EINVAL, "timer fds do not support write");
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        return_errno!(EINVAL, "timer fds do not support write");
    }

    pub fn access_mode(&self) -> AccessMode {
        // We consider all timer fds read-only
        AccessMode::O_RDONLY
    }

    pub fn status_flags(&self) -> StatusFlags {
        let flags = self.flags.load(Ordering::Relaxed);

        if flags.contains(TimerCreationFlags::TFD_NONBLOCK) {
            StatusFlags::O_NONBLOCK
        } else {
            StatusFlags::empty()
        }
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        if new_flags.is_nonblocking() {
            self.flags
                .store(TimerCreationFlags::TFD_NONBLOCK, Ordering::Relaxed);
        } else {
            self.flags
                .store(TimerCreationFlags::empty(), Ordering::Relaxed);
        }

        Ok(())
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        return_errno!(EINVAL, "timer fds do not support ioctl");
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        self.pollee.poll(mask, poller)
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        self.pollee.register_observer(observer, mask);
        Ok(())
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        self.pollee
            .unregister_observer(observer)
            .ok_or_else(|| errno!(ENOENT, "the observer is not registered"))
    }
}

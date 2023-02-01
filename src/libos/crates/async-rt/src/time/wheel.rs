use super::entry::TimerWheelEntry;
use super::Instant;
use crate::executor::EXECUTOR;
use crate::prelude::*;
use hierarchical_hash_wheel_timer::wheels::quad_wheel::QuadWheelWithOverflow;
use hierarchical_hash_wheel_timer::wheels::Skip;
use spin::mutex::MutexGuard;

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        // For SGX environment, the timer is woken from the untrusted world, thus, the waker memory must be in untrusted world.
        use sgx_untrusted_alloc::UntrustedBox;
    } else {
        use std::boxed::Box as UntrustedBox;
        use libc::syscall;
    }
}

const WAKER_MAGIC_NUM: u32 = 0xff;
const IDLE_SLEEP_TIME: u32 = 10000; // 10000 ms = 10 s. If the timer wheel is idle, try to sleep for 10s.
const SKIP_TIME_THRESHOLD: u32 = 50; // ms. The timer wheel will go to sleep if there is no timer expiration in 50ms.

// TODO: Calculate the time for one OCALL and should sleep shorter to be more accurate.

lazy_static! {
    pub static ref TIMER_WHEEL_WAKER: UntrustedBox<u32> = UntrustedBox::new(WAKER_MAGIC_NUM);
    pub static ref TIMER_WHEEL: TimerWheel = TimerWheel::new();
}

pub fn run_timer_wheel_thread() {
    TIMER_WHEEL.set_running();
    loop {
        {
            // Lock the status.
            let mut status = TIMER_WHEEL.status().lock();
            debug_assert!(*status == TimerWheelStatus::Running);
            let result = TIMER_WHEEL.try_make_progress();
            if let Some(entries) = result.expired_timers {
                drop(status);
                // Don't hold lock when firing timers.
                TimerWheel::fire(entries);
            } else if let Some(skip) = result.skip {
                let timeout = Duration::from_millis(skip as u64);
                *status = TimerWheelStatus::Asleep(timeout);
                drop(status);

                debug!("Timer Wheel: will sleep {:?}", timeout);
                let ret = futex_wait_timeout(&TIMER_WHEEL_WAKER, &timeout, WAKER_MAGIC_NUM);

                // If timedout, set running by itself. If woken up, the status has been set by waker thread.
                if ret.is_err() {
                    debug!("Timer Wheel: timeout");
                    TIMER_WHEEL.set_running();
                } else {
                    debug!("Timer Wheel: woken up");
                }
            }
        }

        // The timer wheel will make progress during the insertion
        TIMER_WHEEL.insert_pending_entries();

        if EXECUTOR.is_shutdown() {
            break;
        }
    }
}

pub fn wake_timer_wheel(timeout: &Duration) {
    loop {
        let mut status = TIMER_WHEEL.status().lock();
        match *status {
            TimerWheelStatus::Idle | TimerWheelStatus::Running => return,
            TimerWheelStatus::Asleep(duration) => {
                // If timeout is greater than sleep time, there is no need to wake up the timer wheel.
                // It will be updated when it gets timeout.
                if timeout < &duration {
                    // Must loop to gurantee the timer wheel is woken up
                    debug!("Timer Wheel: try waking");
                    if let Ok(num) = futex_wake(&TIMER_WHEEL_WAKER) {
                        if num > 0 {
                            *status = TimerWheelStatus::Running;
                            break;
                        }
                    }
                } else {
                    return;
                } // end if timeout < &duration
            }
        }
        // Let timer wheel thread to update the status.
        drop(status);
    }
}

#[derive(Debug, PartialEq)]
enum TimerWheelStatus {
    Idle,
    Running,
    Asleep(Duration),
}

/// The interface of hierarchical timer wheel. The resolution of time is 1ms.
pub struct TimerWheel {
    // hierarchical timer wheel.
    wheel: Mutex<QuadWheelWithOverflow<TimerWheelEntry>>,
    // current ticks, one tick means 1ms.
    ticks: AtomicU64,
    // start time of the timerwheel.
    start: Instant,
    // status of the timerwheel.
    status: Mutex<TimerWheelStatus>,
    // Pending entries before adding to the wheel
    pending_entries: Mutex<Vec<TimerWheelEntry>>,
}

impl TimerWheel {
    pub fn new() -> Self {
        Self {
            wheel: Mutex::new(QuadWheelWithOverflow::default()),
            ticks: AtomicU64::new(0),
            start: Instant::now(),
            status: Mutex::new(TimerWheelStatus::Idle),
            pending_entries: Mutex::new(Vec::new()),
        }
    }

    /// Get the status of the timer wheel
    fn status(&self) -> &Mutex<TimerWheelStatus> {
        &self.status
    }

    /// Set the timer wheel to running status
    pub fn set_running(&self) {
        *self.status.lock() = TimerWheelStatus::Running
    }

    /// Set the timer wheel to asleep status
    pub fn set_asleep(&self, sleep_time: Duration) {
        *self.status.lock() = TimerWheelStatus::Asleep(sleep_time)
    }

    /// Insert a new timer entry to the wheel pending list
    pub fn insert_entry(&self, entry: TimerWheelEntry, timeout: Duration) -> u64 {
        let mut pending_entries = self.pending_entries.lock();
        pending_entries.push(entry);

        let elapsed = self.start.elapsed().as_millis() as u64;
        wake_timer_wheel(&timeout);
        debug!("insert timer entry, timeout = {:?}", timeout);
        elapsed
    }

    pub fn insert_pending_entries(&self) {
        let mut pending_entries = self.pending_entries.lock();
        while pending_entries.len() > 0 {
            let pending_entry = pending_entries.pop().unwrap();
            let timeout_entries = {
                let mut guard = self.wheel.lock();
                // Try to make progress to assure that the wheel is up to date.
                let timeout_entries = self.make_progress_locked(&mut guard);
                let remained_timeout = pending_entry.remained_duration();

                // The minimum resolution of QuadWheelWithOverflow is 1 ms.
                // Based on experiments, the timer is very likely to expire about 1 ms earlier, which truly follows the minimum resolution.
                // But this is a unacceptable behavior for many test suite, because elapsed time could be smaller than timeout time.
                // Thus, we wait one more milli-second here. For timeout less than 1 ms, we try to wait more than 1 ms.
                if remained_timeout == Duration::ZERO {
                    Self::fire(vec![pending_entry]);
                } else if remained_timeout <= Duration::MILLISECOND {
                    let timeout = Duration::MILLISECOND;
                    let _ = guard.insert_with_delay(pending_entry, timeout);
                } else {
                    let timeout = remained_timeout + Duration::MILLISECOND;
                    let _ = guard.insert_with_delay(pending_entry, timeout);
                }
                timeout_entries
            };

            Self::fire(timeout_entries);
        }
    }

    /// Try to move the timerwheel forward and fire expired timers.
    /// Return the skip time if any.
    pub fn try_make_progress(&self) -> ProgressResult {
        if let Some(mut wheel_guard) = self.wheel.try_lock() {
            let entries = self.make_progress_locked(&mut wheel_guard);
            if entries.len() > 0 {
                drop(wheel_guard);
                return ProgressResult::expired_timers_exist(entries);
            }

            let skip = wheel_guard.can_skip();
            match skip {
                // In the next few ms, there is no expired timers.
                Skip::Millis(ms) => {
                    if ms >= SKIP_TIME_THRESHOLD {
                        return ProgressResult::can_sleep(ms);
                    }
                }
                // There is no timer at all.
                Skip::Empty => {
                    return ProgressResult::can_sleep(IDLE_SLEEP_TIME);
                }
                // Can't skip. Keep making progress
                Skip::None => {}
            };
        }

        return ProgressResult::default();
    }

    pub fn make_progress(&self) {
        let mut wheel_guard = self.wheel.lock();
        let entries = self.make_progress_locked(&mut wheel_guard);
        drop(wheel_guard);
        Self::fire(entries);
    }

    /// Try to move the timerwheel forward and return all expired timers.
    ///
    /// The returned timers must be fired by `fire()`!
    fn make_progress_locked(
        &self,
        wheel_guard: &mut MutexGuard<QuadWheelWithOverflow<TimerWheelEntry>>,
    ) -> Vec<TimerWheelEntry> {
        let elapsed = self.start.elapsed().as_millis() as u64;
        // calculate the step that we need move forward, in most times, it should be 0 or 1.
        let diff = elapsed - self.latest_ticks();
        self.ticks.store(elapsed, Ordering::Release);
        let mut entries = Vec::new();
        for _ in 0..diff {
            entries.append(&mut wheel_guard.tick())
        }
        entries
    }

    /// Fire expired timers.
    fn fire(entries: Vec<TimerWheelEntry>) {
        entries.iter().for_each(|e| e.fire());
    }

    /// Get latest ticks.
    pub fn latest_ticks(&self) -> u64 {
        self.ticks.load(Ordering::Relaxed)
    }
}

// When trying to make progress, there are several results:
// (1) skip: some, expired_timers: none
//      the timer wheel thread can sleep for a while
// (2) skip: none, expired_timers: some
//      there are some expired timers to fire
// (3) skip: none, expired_timers: none
//      there is no expired timers right now, but there will be in the near future.
//      so the timer wheel can't sleep
// (4) skip: some, expired_timers: some
//      this is impossible
#[derive(Debug, Default)]
pub struct ProgressResult {
    skip: Option<u32>, // ms
    expired_timers: Option<Vec<TimerWheelEntry>>,
}

impl ProgressResult {
    fn can_sleep(skip: u32) -> Self {
        Self {
            skip: Some(skip),
            ..Default::default()
        }
    }

    fn expired_timers_exist(entries: Vec<TimerWheelEntry>) -> Self {
        Self {
            expired_timers: Some(entries),
            ..Default::default()
        }
    }
}

fn futex_wait_timeout(uaddr: &UntrustedBox<u32>, timeout: &Duration, val: u32) -> Result<()> {
    let mut ret: libc::c_int = 0;
    let mut errno: libc::c_int = 0;
    let uaddr = &**uaddr as *const u32;
    let mut timeout = libc::timespec {
        tv_sec: timeout.as_secs() as i64,
        tv_nsec: timeout.subsec_nanos() as i64,
    };

    cfg_if::cfg_if! {
        if #[cfg(feature = "sgx")] {
            extern "C" {
                fn ocall_futex_wait_timeout(
                    ret: *mut libc::c_int,
                    errno: *mut libc::c_int,
                    uaddr: *const u32,
                    timeout: *mut libc::timespec,
                    val: libc::c_uint,
                ) -> sgx_types::sgx_status_t;
            }
            unsafe {
                let sgx_status = ocall_futex_wait_timeout(&mut ret as *mut _, &mut errno as *mut _, uaddr, &mut timeout as *mut _, val);
                assert!(sgx_status == sgx_types::sgx_status_t::SGX_SUCCESS);
            };
        } else {
            unsafe {
                ret = syscall(libc::SYS_futex, uaddr, libc::FUTEX_WAIT, val, &mut timeout as *mut _, 0, 0) as libc::c_int;
                errno = *libc::__errno_location();
            }
        }
    }
    if ret == 0 {
        // Woken up
        return Ok(());
    } else {
        return_errno!(Errno::from(errno as u32), "futex wait timeout error");
    }
}

fn futex_wake(uaddr: &UntrustedBox<u32>) -> Result<usize> {
    let mut ret: libc::c_int = 0;
    let mut errno: libc::c_int = 0;
    let uaddr = &**uaddr as *const u32;

    cfg_if::cfg_if! {
        if #[cfg(feature = "sgx")] {
            extern "C" {
                fn ocall_futex_wake(
                    ret: *mut libc::c_int,
                    errno: *mut libc::c_int,
                    uaddr: *const u32,
                ) -> sgx_types::sgx_status_t;
            }
            unsafe {
                let sgx_status = ocall_futex_wake(&mut ret as *mut _, &mut errno as *mut _, uaddr);
                assert!(sgx_status == sgx_types::sgx_status_t::SGX_SUCCESS);
            };
        } else {
            unsafe {
                ret = syscall(libc::SYS_futex, uaddr, libc::FUTEX_WAKE, 1, 0, 0, 0) as libc::c_int;
                errno = *libc::__errno_location();
            }
        }
    }

    if ret < 0 {
        return_errno!(Errno::from(errno as u32), "futex wake failure");
    } else {
        return Ok(ret as usize);
    }
}

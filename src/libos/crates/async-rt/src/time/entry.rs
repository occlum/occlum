use super::wheel::TIMER_WHEEL;
use super::DURATION_ZERO;
use crate::prelude::*;
use core::task::Waker;

/// The state of timer.
#[derive(Debug)]
pub enum TimerState {
    /// This timer has not been inserted to the timer wheel.
    Init,
    /// This timer has been inserted to the timer wheel and waits timeout.
    Started(StartedInner),
    /// This timer has expired, which means the wheel has triggered the timer.
    Expired,
    /// This timer has been cancelled, and records elapsed time between started and cancelled state.
    Cancelled(Duration),
}

#[derive(Debug)]
pub struct StartedInner {
    start_ticks: u64,
    waker: Option<Waker>,
}

impl StartedInner {
    pub fn new(start_ticks: u64, waker: Waker) -> Self {
        Self {
            // This will be updated when the timer entry is inserted to the timer wheel.
            start_ticks: start_ticks,
            waker: Some(waker),
        }
    }

    pub fn wake(&mut self) {
        self.waker.take().unwrap().wake();
    }

    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }

    pub fn elapsed_time(&self) -> Duration {
        let elapsed_ticks = TIMER_WHEEL.latest_ticks() - self.start_ticks;
        Duration::from_millis(elapsed_ticks)
    }
}

/// The shared data structure of timer.
#[derive(Debug)]
pub struct TimerShared {
    state: TimerState,
    timeout: Duration,
}

/// The wrapper of `TimerShared` with `Send` and `Sync`.
pub type TimerSharedRef = Arc<Mutex<TimerShared>>;

impl TimerShared {
    pub fn new(timeout: Duration) -> Self {
        Self {
            state: TimerState::Init,
            timeout,
        }
    }

    /// Transfer to expired state and wake up the timer.
    pub fn fire(&mut self) {
        match &mut self.state {
            // This timer has been cancelled, do nothing here.
            TimerState::Cancelled(_) => {}
            // Transfer to expired state and wake up the timer.
            TimerState::Started(inner) => {
                // Since TimerShared is guarded by the lock, we can wake before setting expired state
                inner.wake();
                self.state = TimerState::Expired;
            }
            _ => panic!("Can not fire timer in init or expired state"),
        }
    }

    /// Transfer to cancelled state
    pub fn cancel(&mut self) {
        match &self.state {
            TimerState::Init => {
                self.state = TimerState::Cancelled(DURATION_ZERO);
            }
            TimerState::Started(_) => {
                // Timer wheel may not be updated, the elapsed time could overflow. Just return zero.
                self.state = TimerState::Cancelled(DURATION_ZERO);
            }
            TimerState::Expired => {}
            TimerState::Cancelled(_) => panic!("Can not cancel twice"),
        }
    }

    /// Get the remained duration.
    pub fn remained_duration(&self) -> Duration {
        match &self.state {
            TimerState::Init => self.timeout,
            TimerState::Started(inner) => {
                // If timeout is less than 1ms, we will wait 1ms instead.
                // It might cause the timeout is less than elapsed_time.
                return self.timeout.saturating_sub(inner.elapsed_time());
            }
            TimerState::Expired => DURATION_ZERO,
            TimerState::Cancelled(elapsed) => self.timeout.saturating_sub(*elapsed),
        }
    }
}

/// The entry used by user to get remained duration.
#[derive(Debug)]
pub struct TimerEntry(TimerSharedRef);

/// The entry used for `await` or `poll`.
#[derive(Debug)]
pub struct TimerFutureEntry(TimerSharedRef);

/// The entry used for timer wheel. This entry will be inserted to the timer wheel.
/// When the timer wheel trigger this entry, the wheel will invoke `fire()` to fire the timer.
#[derive(Debug)]
pub struct TimerWheelEntry(TimerSharedRef);

impl TimerEntry {
    pub fn new(timeout: Duration) -> Self {
        Self(Arc::new(Mutex::new(TimerShared::new(timeout))))
    }

    // Get the remained duration.
    pub fn remained_duration(&self) -> Duration {
        let shared = self.0.lock();
        shared.remained_duration()
    }
}

impl TimerFutureEntry {
    pub fn new(entry: &TimerEntry) -> Self {
        Self(entry.0.clone())
    }
}

impl Future for TimerFutureEntry {
    type Output = ();

    /// Return Ready if the timer is expired, or Pending.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared = self.0.lock();
        match &mut shared.state {
            TimerState::Init => {
                // If the timeout is zero, return directly.
                if shared.timeout.is_zero() {
                    shared.state = TimerState::Expired;
                    return Poll::Ready(());
                }

                let entry = TimerWheelEntry(self.0.clone());

                // Insert the timer entry to the timer wheel pending list.
                // Previously, there is too much work in "insert_entry", including making process for the timer wheel and fire timeout timers,
                // which makes the poll slow and can cause the wait_timeout to fail to respond to other events.
                // Now the heavy work is offloaded to the timer thread. And the current poll thread only inserts the timer into the pending list.
                // The timer will then be inserted into the timer wheel by the timer thread when the timer thread is woken.
                let start_tick = TIMER_WHEEL.insert_entry(entry, shared.timeout);

                // Transfer to started state, set waker. start_ticks will be updated when inserting to the timer wheel.
                let inner = StartedInner::new(start_tick, cx.waker().clone());
                shared.state = TimerState::Started(inner);
                Poll::Pending
            }
            TimerState::Started(inner) => {
                inner.set_waker(cx.waker().clone());
                Poll::Pending
            }
            TimerState::Expired => Poll::Ready(()),
            TimerState::Cancelled(_) => panic!("Can not poll cancelled timer entry"),
        }
    }
}

impl Drop for TimerFutureEntry {
    /// Make sure that the state is Init, Expired or Cancelled after drop.
    /// If the state is Started, cancel the timer before dropped.
    fn drop(&mut self) {
        let mut shared = self.0.lock();
        if let TimerState::Started(_) = shared.state {
            shared.cancel();
        }
    }
}

impl TimerWheelEntry {
    /// Fire the timer.
    pub fn fire(&self) {
        let mut shared = self.0.lock();
        shared.fire();
    }

    pub fn remained_duration(&self) -> Duration {
        let shared = self.0.lock();
        shared.remained_duration()
    }
}

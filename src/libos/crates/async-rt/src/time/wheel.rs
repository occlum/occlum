use super::entry::TimerWheelEntry;
use super::Instant;
use crate::prelude::*;
use spin::mutex::MutexGuard;

use hierarchical_hash_wheel_timer::wheels::quad_wheel::QuadWheelWithOverflow;

lazy_static! {
    pub static ref TIMER_WHEEL: TimerWheel = {
        use crate::sched::{yield_, SchedPriority};
        use crate::task::SpawnOptions;

        let wheel = TimerWheel::new();

        // TODO: Don't always run this task when no useful tasks to run and no timeouts to expire.
        SpawnOptions::new(async {
            loop {
                TIMER_WHEEL.try_make_progress();
                yield_().await;
            }
        }).priority(SchedPriority::Low).spawn();

        wheel
    };
}

/// The interface of hierarchical timer wheel. The resolution of time is 1ms.
pub struct TimerWheel {
    // hierarchical timer wheel.
    wheel: Mutex<QuadWheelWithOverflow<TimerWheelEntry>>,
    // current ticks, one tick means 1ms.
    ticks: AtomicU64,
    // start time of the timerwheel.
    start: Instant,
}

impl TimerWheel {
    pub fn new() -> Self {
        Self {
            wheel: Mutex::new(QuadWheelWithOverflow::default()),
            ticks: AtomicU64::new(0),
            start: Instant::now(),
        }
    }

    /// Insert a new timer entry to the wheel and return the ticks when the entry is inserted.
    pub fn insert_entry(&self, entry: TimerWheelEntry, timeout: Duration) -> u64 {
        let mut guard = self.wheel.lock();

        // Try to make progress to assure that the wheel is up to date.
        let entries = self.make_progress(&mut guard);

        // The minimum resolution of QuadWheelWithOverflow is 1 ms.
        // If the timeout less than 1 ms, wait 1ms instead.
        let timeout = core::cmp::max(timeout, Duration::MILLISECOND);
        guard.insert_with_delay(entry, timeout).unwrap();
        let insert_ticks = self.latest_ticks();

        drop(guard);
        Self::fire(entries);

        insert_ticks
    }

    /// Try to move the timerwheel forward and fire expired timers.
    pub fn try_make_progress(&self) {
        if let Some(mut wheel_guard) = self.wheel.try_lock() {
            let entries = self.make_progress(&mut wheel_guard);
            drop(wheel_guard);
            Self::fire(entries);
        }
    }

    /// Try to move the timerwheel forward and return all expired timers.
    ///
    /// The returned timers must be fired by `fire()`!
    fn make_progress(
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

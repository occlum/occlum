use crate::prelude::*;
use core::ops::{Add, AddAssign, Sub, SubAssign};
use vdso_time::{clock_gettime, ClockId};

lazy_static! {
    static ref LAST_NOW: Mutex<Instant> = Mutex::new(Instant(DURATION_ZERO));
}

pub const DURATION_ZERO: Duration = Duration::from_nanos(0);

/// A measurement of a monotonically nondecreasing clock. Opaque and useful only with Duration.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Instant(pub(crate) Duration);

impl Instant {
    /// Returns an instant corresponding to “now”.
    ///
    /// Rust std says that the clock of linux + x86 environment is real-monotonic.
    /// When sgx feature enabled, we are in linux + x86 env, the clock should be real-monotonic.
    /// However, the clock in sgx env is untrusted, we can not guarantee that clock is real-monotonic.
    /// To ensure the instant is monotonically nondecreasing, We keep a global "latest now" instance
    /// which is returned instead of what the OS says if the OS goes backwards.
    pub fn now() -> Self {
        let os_now = Instant(clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap());

        let mut last_now = LAST_NOW.lock();
        let now = core::cmp::max(*last_now, os_now);
        *last_now = now;

        now
    }

    /// Returns the amount of time elapsed from another instant to this one.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.0
            .checked_sub(earlier.0)
            .expect("earlier is later than self.")
    }

    /// Returns the amount of time elapsed from another instant to this one,
    /// or None if that instant is later than this one.
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.0.checked_sub(earlier.0)
    }

    /// Returns the amount of time elapsed from another instant to this one,
    /// or zero duration if that instant is later than this one.
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.0.checked_sub(earlier.0).unwrap_or(DURATION_ZERO)
    }

    /// Returns the amount of time elapsed since this instant was created,
    /// or zero duration if Instant::now() is earlier than this one.
    pub fn elapsed(&self) -> Duration {
        Instant::now() - *self
    }

    /// Returns Some(t) where t is the time self + duration if t can be represented as Instant
    /// (which means it’s inside the bounds of the underlying data structure), None otherwise.
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        Some(Instant(self.0.checked_add(duration)?))
    }

    /// Returns Some(t) where t is the time self - duration if t can be represented as Instant
    /// (which means it’s inside the bounds of the underlying data structure), None otherwise.
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        Some(Instant(self.0.checked_sub(duration)?))
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Duration {
        self.duration_since(other)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, other: Duration) -> Instant {
        self.checked_add(other)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, other: Duration) -> Instant {
        self.checked_sub(other)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

#[cfg(test)]
mod tests {
    use super::{Duration, Instant};
    use std::thread;
    use test::Bencher;

    #[test]
    fn test_instant() {
        let start = Instant::now();

        let ms = 100;
        thread::sleep(Duration::from_millis(ms));

        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() as u64 >= ms - 1);
        assert!(elapsed.as_millis() as u64 <= ms + 1);
    }

    #[bench]
    fn bench_instant(b: &mut Bencher) {
        b.iter(|| Instant::now());
    }
}

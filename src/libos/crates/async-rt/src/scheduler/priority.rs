use std::ops::Add;

/// The priority of a schedulable entity.
///
/// The values of priorities range from 0 to 31 (inclusive).
/// The higher the priorities, the greater portion of the CPU time
/// a schedulable entity is assigned.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Priority(u8);

impl Priority {
    /// Create a new priority.
    ///
    /// The value must be between 0 and 31. If out of the range,
    /// the method panics.
    pub fn new(val: u8) -> Self {
        debug_assert!(Self::min_val() <= val && val <= Self::max_val());
        Self(val)
    }

    /// Create a new priority.
    ///
    /// # Safety
    ///
    /// If the given value is out of the valid range, the method
    /// causes undefined behaviors.
    pub const unsafe fn new_unchecked(val: u8) -> Self {
        Self(val)
    }

    /// Get the value of a priority.
    pub fn val(&self) -> u8 {
        self.0
    }

    /// The highest priority.
    pub const HIGHEST: Priority = unsafe { Self::new_unchecked(Self::max_val()) };

    /// A relatively high priority.
    pub const HIGH: Priority =
        unsafe { Self::new_unchecked((Self::max_val() + Self::mid_val()) / 2) };

    /// The normal priority.
    pub const NORMAL: Priority = unsafe { Self::new_unchecked(Self::mid_val()) };

    /// A relatively low priority.
    pub const LOW: Priority =
        unsafe { Self::new_unchecked((Self::mid_val() + Self::min_val()) / 2) };

    /// The lowest priority.
    pub const LOWEST: Priority = unsafe { Self::new_unchecked(Self::min_val()) };

    /// Increase the priority by one.
    pub const fn inc(self) -> Self {
        if self.0 < Self::max_val() {
            Self(self.0 + 1)
        } else {
            self
        }
    }

    /// Decrease the priority by one.
    pub const fn dec(self) -> Self {
        if self.0 > Self::min_val() {
            Self(self.0 - 1)
        } else {
            self
        }
    }

    /// Return the maximum value of a priority.
    pub const fn max_val() -> u8 {
        31
    }

    /// Return the medium value of a priority.
    pub const fn mid_val() -> u8 {
        16
    }

    /// Return the minimum value of a priority.
    pub const fn min_val() -> u8 {
        0
    }

    /// The number of priorities.
    pub const fn count() -> usize {
        (Self::max_val() - Self::min_val() + 1) as usize
    }
}

impl From<Priority> for u8 {
    fn from(priority: Priority) -> u8 {
        priority.val()
    }
}

impl Add<i8> for Priority {
    type Output = Self;

    fn add(self, rhs: i8) -> Self {
        // Use i16 internally to rule out overflow
        let mut new_val = self.val() as i16 + rhs as i16;
        // Make sure the new value falls inside the valid range
        new_val = new_val
            .min(Self::max_val() as i16)
            .max(Self::min_val() as i16);
        // Safety. The new value is valid.
        unsafe { Self::new_unchecked(new_val as u8) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_values() {
        assert!(Priority::new(Priority::max_val()) == Priority::HIGHEST);
        assert!(Priority::new(Priority::min_val()) == Priority::LOWEST);
    }

    #[test]
    fn add_i8() {
        assert!(Priority::NORMAL + 1_i8 > Priority::NORMAL);
        assert!(Priority::NORMAL + (-1_i8) < Priority::NORMAL);
        assert!(Priority::NORMAL + 4_i8 + (-4_i8) == Priority::NORMAL);

        // No overflow
        assert!(Priority::HIGHEST + 1_i8 == Priority::HIGHEST);
        assert!(Priority::HIGHEST + i8::max_value() == Priority::HIGHEST);

        // No underflow
        assert!(Priority::LOWEST + -1_i8 == Priority::LOWEST);
        assert!(Priority::LOWEST + i8::min_value() == Priority::LOWEST);
    }

    #[test]
    fn check_order() {
        assert!(Priority::HIGHEST > Priority::HIGH);
        assert!(Priority::HIGH > Priority::NORMAL);
        assert!(Priority::NORMAL > Priority::LOW);
        assert!(Priority::LOW > Priority::LOWEST);
    }

    #[test]
    fn check_values() {
        assert!(Priority::HIGHEST.val() == Priority::max_val());
        assert!(Priority::LOWEST.val() == Priority::min_val());
    }
}

use crate::scheduler::{Priority, SchedState};

/// Calculate the timeslice that a scheduable entity should be assigned given
/// its `SchedState`. Currently, we only use the base priority to determine
/// the length of the timeslice. The higher the priority, the longer the timeslice.
pub fn calculate_timeslice(sched_state: &SchedState) -> u32 {
    use self::{MAX_TIMESLICE_MS as MAX, MAX_TIMESLICE_MS as MIN};
    // let prio_val = sched_state.base_prio().val();
    let prio_val = sched_state.effective_prio().val();
    if prio_val >= Priority::mid_val() {
        MAX
    } else {
        const REMAIN_PRIOS: u32 = {
            let val = (Priority::mid_val() - 1) - Priority::min_val() + 1;
            // More efficient working with a power of two
            //static_assert!(val.is_power_of_two());
            val as u32
        };
        MIN + (MAX - MIN) / REMAIN_PRIOS * (prio_val as u32)
    }
}

/// The maximum value of timeslices (in ms).
pub const MAX_TIMESLICE_MS: u32 = 50;

/// The minimum value of timeslices (in ms).
pub const MIN_TIMESLICE_MS: u32 = 5;

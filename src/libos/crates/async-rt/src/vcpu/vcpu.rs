//! Virtual CPU (vCPU).

use std::sync::atomic::{AtomicU32, Ordering};

use super::park::PARKS;

/// Set the total number of vCPUs.
///
/// The default number is 1. The user MUST invoke this API to explicitly
/// set the desired number of vCPUs _before_invoking any other APIs of this
/// crate. And this method should only be called _once_. Otherwise, this
/// method won't take any effect.
pub fn set_total(total_vcpus: u32) {
    TOTAL_VCPUS.store(total_vcpus, Ordering::Relaxed);
}

/// Get the total number of vCPUs.
pub fn get_total() -> u32 {
    TOTAL_VCPUS.load(Ordering::Relaxed)
}

static TOTAL_VCPUS: AtomicU32 = AtomicU32::new(1);

/// Get the vCPU ID of the current thread.
///
/// If the current thread is not serving as a vCPU, return None.
/// The vCPU IDs range from `0` to `crate::vcpu::set_total`.
pub fn get_current() -> Option<u32> {
    let id = CURRENT_ID.load(Ordering::Relaxed);
    // debug_assert!(id != NONE_VCPU);
    if id == NONE_VCPU {
        None
    } else {
        Some(id)
    }
}

/// Set the vCPU ID of the current thread.
pub(crate) fn set_current(current_vcpu: u32) {
    CURRENT_ID.store(current_vcpu, Ordering::Relaxed);
    PARKS.register(current_vcpu as usize);
}

/// Clear the vCPU ID of the current thread.
pub(crate) fn clear_current() {
    let current_vcpu = CURRENT_ID.swap(NONE_VCPU, Ordering::Relaxed);
    PARKS.unregister(current_vcpu as usize);
}

#[thread_local]
static CURRENT_ID: AtomicU32 = AtomicU32::new(NONE_VCPU);

const NONE_VCPU: u32 = u32::max_value();

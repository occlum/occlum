//! SchedAgent manages the CPU scheduler settings for a thread.
//!
//! # Scheduler Settings
//!
//! Currently, the only scheduler setting that SchedAgent can access and update
//! is the CPU affinity of a thread. Other settings will be added in the future.
//!
//! # The Two Modes: Attached vs Detached
//!
//! SchedAgent works in one of the two modes: the attached mode and the detached
//! mode.
//!
//! When a SchedAgent is created, it is initially in the detached mode,
//! meaning that the SchedAgent is not attached to any host OS thread. Thus,
//! any call on SchedAgent to update scheduler settings does not actually affect any
//! host OS thread; SchedAgent just records the updates.
//!
//! After SchedAgent becomes attached to some host OS thread by invoking the `attach`
//! method, all previous updates recorded during in the detached mode will
//! be applied to the host OS thread. Afterwards, all setting updates will be applied
//! immediately to the host OS thread---until SchedAgent is detached from the
//! host OS thread.

use super::cpu_set::{CpuSet, AVAIL_CPUSET};
use crate::prelude::*;
use crate::util::dirty::Dirty;
use async_rt::task::Task;

#[derive(Debug)]
pub struct SchedAgent {
    // The use of Option does not mean inner is optional. In contrast, we maintain
    // the invariant of `inner.is_some() == true`. We use Option so that we can
    // move the Inner out of SchedAgent without upsetting Rust's borrow checker.
    inner: Option<Inner>,
}

impl Clone for SchedAgent {
    /// Clone a SchedAgent in a way that works well with clone and spawn syscall.
    ///
    /// We cannot use the auto-derived implementation of clone for SchedAgent, which
    /// would copy the fields of SchedAgent bit-by-bit. The reason is two-fold.
    ///
    /// First, a SchedAgent, if in the attached mode, actually holds a host OS
    /// resource (`host_tid`). Copying a SchedAgent bit-by-bit would result in
    /// multiple instances of SchedAgent refer to the same host thread.
    ///
    /// Second, we need to ensure that the scheduler settings in a cloned SchedAgent
    /// instance will take effect when the SchedAgent is attached to a host thread.
    ///
    /// This implementation carefully handles the two points above.
    fn clone(&self) -> Self {
        let mut affinity = Dirty::new(match self.inner() {
            Inner::Detached { affinity } => affinity.as_ref().clone(),
            Inner::Attached { affinity, .. } => affinity.clone(),
        });
        if affinity.as_ref().as_slice() != AVAIL_CPUSET.as_slice() {
            affinity.set_dirty();
        }
        Self {
            inner: Some(Inner::Detached { affinity }),
        }
    }
}

#[derive(Debug, Clone)]
enum Inner {
    Detached { affinity: Dirty<CpuSet> },
    Attached { task: Arc<Task>, affinity: CpuSet },
}

impl SchedAgent {
    pub fn new() -> Self {
        let inner = Some({
            let affinity = Dirty::new(AVAIL_CPUSET.clone());
            Inner::Detached { affinity }
        });
        Self { inner }
    }

    pub fn affinity(&self) -> &CpuSet {
        match self.inner() {
            Inner::Detached { affinity } => affinity.as_ref(),
            Inner::Attached { affinity, .. } => affinity,
        }
    }

    pub fn set_affinity(&mut self, new_affinity: CpuSet) -> Result<()> {
        if new_affinity.empty() {
            return_errno!(EINVAL, "there must be at least one CPU core in the CpuSet");
        }
        if !new_affinity.is_subset_of(&AVAIL_CPUSET) {
            return_errno!(
                EINVAL,
                "one or some of the CPU cores are not available to set"
            );
        }
        match self.inner_mut() {
            Inner::Detached { affinity } => {
                *affinity.as_mut() = new_affinity;
            }
            Inner::Attached { task, affinity } => {
                update_affinity(task, &new_affinity);
                *affinity = new_affinity;
            }
        };
        Ok(())
    }

    pub fn task(&self) -> Option<Arc<Task>> {
        match self.inner() {
            Inner::Detached { .. } => None,
            Inner::Attached { task, .. } => Some(task.clone()),
        }
    }

    pub fn attach(&mut self, task: Arc<Task>) {
        self.update_inner(|inner| match inner {
            Inner::Detached { affinity } => {
                let affinity = {
                    if affinity.dirty() {
                        update_affinity(&task, affinity.as_ref())
                    }
                    affinity.unwrap()
                };
                Inner::Attached { task, affinity }
            }
            Inner::Attached { .. } => panic!("cannot attach when the agent is already attached"),
        });
    }

    pub fn detach(&mut self) {
        self.update_inner(|inner| match inner {
            Inner::Detached { .. } => panic!("cannot detach when the agent is already detached"),
            Inner::Attached { affinity, .. } => {
                let affinity = Dirty::new(affinity);
                Inner::Detached { affinity }
            }
        });
    }

    pub fn is_attached(&self) -> bool {
        match self.inner() {
            Inner::Detached { .. } => false,
            Inner::Attached { .. } => true,
        }
    }

    fn inner(&self) -> &Inner {
        self.inner.as_ref().unwrap()
    }

    fn inner_mut(&mut self) -> &mut Inner {
        self.inner.as_mut().unwrap()
    }

    fn update_inner<F>(&mut self, f: F)
    where
        F: FnOnce(Inner) -> Inner,
    {
        let old_inner = self.inner.take().unwrap();
        let new_inner = f(old_inner);
        self.inner = Some(new_inner);
    }
}

impl Default for SchedAgent {
    fn default() -> Self {
        Self::new()
    }
}

fn update_affinity(task: &Arc<Task>, affinity: &CpuSet) {
    let ncores = CpuSet::ncores();
    let mut task_affinity = task.sched_info().affinity().write();
    for (idx, bit) in affinity.iter().enumerate() {
        if idx >= ncores {
            break;
        }
        task_affinity.set(idx, *bit)
    }
}

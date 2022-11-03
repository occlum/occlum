use spin::{Mutex, MutexGuard};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::*};
use std::sync::Arc;

use crate::prelude::*;
use crate::scheduler::SchedEntity;
use crate::vcpu;

pub use self::runqueues::RunQueues;
pub use self::status_notifier::StatusNotifier;

mod runqueues;
mod status_notifier;

/// A local per-vCPU scheduler.
///
/// # Overview
///
/// Local schedulers are designed to make scheduling decisions locally,
/// eliminating lock contentions whenever possible.
///
/// A local scheduler be seen as a priority queue of schedulable entities (`SchedEntity`).
/// The more "interactive" an entity is, the higher priority it will be given.
/// I/O-bound code is regarded more interactive, while CPU-bound code is less.
/// Scheduling decisions, including determining how interactive an entity is,
/// are made based on the states (`SchedState`) of entities.
///
/// # Usage
///
/// A local scheduler can also be seen as a multi-producer, single-consumer queue.
/// Multiple threads can enqueue entities into it, but only one thread can
/// dequeue entities from it. To ensure that the number of the concurrent
/// consumers is one, a consumer need to "lock" the local scheduler.
///
/// ```ignore
/// let scheduler = LocalScheduler::new();
/// let scheduler_guard = scheduler.lock();
/// let entity = guard.dequeue();
/// ```
///
/// The `lock` method is much more lightweight than acquiring a real lock
/// because it never blocks. It is the user's responsiblity to ensure that
/// there are no concurrent threads that try to acquire the lock. If such
/// situation happens, the lock method will panic.
///
/// It is ok to enqueue entities concurrently and without using the guard.
///
/// ```ignore
/// let scheduler = LocalScheduler::new();
/// let entity = todo!("create the entity");
/// scheduler.enqueue(entity);
/// ```
///
/// # Status notification
///
/// There are two types of status that a user of local schedulers may
/// be interested.
///
/// The first one is whether a local scheduler is _idle_ or not. An idle scheduler
/// enters a busy loop in an attempt to dequeue entities. For the sake of
/// CPU efficiency, the users should assign new entities to idle schedulers.
///
/// The second one is whether a local scheduler is _sleeping_ or not.
/// After becoming idle for a period of time, the scheduler will go to sleep
/// to avoid wasting CPU cycles. The scheduler will be waken up when entities
/// are enqueued to it in the future. But it generally takes more time for
/// these entities to get executed. So it benefits the users to know whether
/// a scheduler is sleeping or not.
///
/// Upon the creation of a local scheduler, the user can provide a status
/// notifier, which are invoked by the scheduler to notifier any status changes.
pub struct LocalScheduler<E> {
    this_vcpu: u32,
    // Use two set of RunQueues internally.
    //
    // One set is the front runqueues, which contains entities that have
    // remaining timeslices.
    // The other set is the back runqueues, which containns entities that have
    // exhausted their timeslices.
    //
    // A value of u8 is saved as the index to indicate which one is the
    // front runqueues.
    rqs: Mutex<([Box<RunQueues<E>>; 2], u8)>,
    // The total number of entities in the two runqueues.
    len: AtomicU32,
    is_locked: AtomicBool,
    status_notifier: Box<dyn StatusNotifier>,
}

impl<E: SchedEntity> LocalScheduler<E> {
    /// Create a new instance.
    pub fn new<S>(this_vcpu: u32, status_notifier: S) -> Self
    where
        S: StatusNotifier + 'static,
    {
        Self {
            this_vcpu,
            rqs: Mutex::new(([Box::new(RunQueues::new()), Box::new(RunQueues::new())], 0)),
            len: AtomicU32::new(0),
            is_locked: AtomicBool::new(false),
            status_notifier: Box::new(status_notifier),
        }
    }

    /// Enqueue an entity.
    pub fn enqueue(&self, entity: &Arc<E>) {
        // To ensure that an entity can never be enqueued twice
        let sched_state = entity.sched_state();
        let already_enqueued = sched_state.set_enqueued();
        if already_enqueued {
            debug!("Failed to enqueue task");
            return;
        }

        sched_state.set_vcpu(self.this_vcpu);

        // Depending on whether the entity still has remaining timeslice,
        // enqueue it into the front or back runqueues.
        let mut rqs = self.lock_rqs();
        if sched_state.timeslice() > 0 {
            let front_rqs = rqs.front_mut();
            front_rqs.enqueue(entity.clone());
        } else {
            sched_state.assign_timeslice();

            let back_rqs = rqs.back_mut();
            back_rqs.enqueue(entity.clone());
        }

        // To ensure that len won't underflow, we need to
        // increase len before enqueueing.
        self.len.fetch_add(1, Relaxed);

        // Notify the changes of idle status
        self.status_notifier
            .notify_idle_status(self.this_vcpu, false);

        self.wake();
    }

    /// Return the length of the scheduler (if it is viewed as
    /// a single priority queue).
    pub fn len(&self) -> usize {
        self.len.load(Relaxed) as usize
    }

    fn dequeue(&self) -> Option<Arc<E>> {
        let mut rqs = self.lock_rqs();

        if self.len.load(Relaxed) == 0 {
            self.status_notifier
                .notify_idle_status(self.this_vcpu, true);
            return None;
        }

        loop {
            let front_rqs = rqs.front_mut();
            if let Some(entity) = front_rqs.try_dequeue() {
                entity.sched_state().clear_enqueued();

                // To ensure the invariant that len >= 0, we need to
                // decrease len after dequeueing.
                let old_len = self.len.fetch_sub(1, Relaxed);

                // Notify the idle status as early as possible
                debug_assert!(old_len >= 1);
                let is_idle = old_len == 1;
                if is_idle {
                    self.status_notifier
                        .notify_idle_status(self.this_vcpu, true);
                }

                return Some(entity);
            }

            self.reschedule(&mut rqs);
        }
    }

    fn reschedule(&self, rqs: &mut RqsLockGuard<'_, E>) {
        // All entities in the back runqueues have assigned timeslices
        // when they are enqueued. So now we only need to switch
        // the front and back runqueues.
        rqs.switch();
    }

    /// Drain entities that satisfy a condition.
    ///
    /// Similar to the `dequeue` method, the `drain` method removes entities
    /// from the scheduler. The dequeue pop the first tasks with highest priority,
    /// while the drain method removes specified number tasks with low priority.
    ///
    /// This method is useful for migration tasks from one vCPU to another
    /// during load balancing.
    pub fn drain<F>(&self, mut cond: F, drained: &mut Vec<Arc<E>>) -> usize
    where
        F: FnMut(&E) -> bool,
    {
        if self.len.load(Relaxed) == 0 {
            return 0;
        }

        let max_drained = drained.capacity() - drained.len();
        let mut num_drained = 0;
        let mut rqs = self.lock_rqs();

        // Drain tasks from runqueues, the drained tasks have relatively low priority.
        let mut drain_queue = |rqs: &mut RunQueues<E>| {
            if num_drained >= max_drained {
                return;
            }

            // Iterate the internal run queues in order of ascending priority
            for (q_i, q) in rqs.run_queues.iter_mut().rev().enumerate() {
                let mut i = 0;
                while i < q.len() {
                    let entity = &q[i];
                    if cond(entity) {
                        let entity = q.swap_remove_back(i).unwrap();
                        entity.sched_state().clear_enqueued();
                        self.len.fetch_sub(1, Relaxed);

                        if q.is_empty() {
                            let idx = 31 - q_i;
                            rqs.nonempty_mask &= !(1 << idx);
                        }
                        drained.push(entity);
                        num_drained += 1;
                        if num_drained >= max_drained {
                            return;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
        };
        drain_queue(rqs.back_mut());
        drain_queue(rqs.front_mut());
        num_drained
    }

    /// The running vcpu wait until being enqueued tasks
    pub fn wait_enqueue(&self) {
        // The thread should keep some time in idle state and avoid slumping into sleep state too quickly.
        // The loop time of count 5_000_000 is about 0.18 seconds in our dev machine.
        let mut count = 5_000_000;

        // In the idle status
        {
            self.status_notifier
                .notify_idle_status(self.this_vcpu, true);
            // It is expensive to sleep and wake up. So we would
            // rather spend a bit of more time spinning.
            while self.len.load(Relaxed) == 0 && count > 0 {
                count -= 1;
                core::hint::spin_loop();
            }
            self.status_notifier
                .notify_idle_status(self.this_vcpu, false);
            if count > 0 {
                return;
            }
        }

        // In the sleep status
        {
            self.status_notifier
                .notify_sleep_status(self.this_vcpu, true);
            vcpu::park();
            self.status_notifier
                .notify_sleep_status(self.this_vcpu, false);
        }
    }

    /// Wake vcpu of the local scheduler
    pub fn wake(&self) {
        // Unpark virtually costs us nothing when the dequeueing thread
        // is not sleeping. So it is ok to invoke this method everytime
        // we enqueue an entity.
        vcpu::unpark(self.this_vcpu as usize);
    }

    fn lock_rqs(&self) -> RqsLockGuard<'_, E> {
        RqsLockGuard(self.rqs.lock())
    }

    /// Lock a local scheduler, acquiring its guard.
    pub fn lock(&self) -> LocalSchedulerGuard<'_, E> {
        let has_locked = self.is_locked.swap(true, Acquire);
        assert!(!has_locked);
        LocalSchedulerGuard(self)
    }
}

struct RqsLockGuard<'a, E>(MutexGuard<'a, ([Box<RunQueues<E>>; 2], u8)>);

impl<'a, E> RqsLockGuard<'a, E> {
    /// Get the front runqueue
    pub fn front(&self) -> &RunQueues<E> {
        let front_index = self.0 .1 as usize;
        &self.0 .0[front_index]
    }

    /// Get the back runqueue
    pub fn back(&self) -> &RunQueues<E> {
        let back_index = (self.0 .1 ^ 1) as usize;
        &self.0 .0[back_index]
    }

    /// Get the front mutable runqueue
    pub fn front_mut(&mut self) -> &mut RunQueues<E> {
        let front_index = self.0 .1 as usize;
        &mut self.0 .0[front_index]
    }

    /// Get the back mutable runqueue
    pub fn back_mut(&mut self) -> &mut RunQueues<E> {
        let back_index = (self.0 .1 ^ 1) as usize;
        &mut self.0 .0[back_index]
    }

    /// Switch front and back runqueue
    pub fn switch(&mut self) {
        self.0 .1 ^= 1;
    }
}

/// A guard for a local scheduler.
///
/// With the guard, the user can dequeue entities from
/// the local scheduler. The guard cannot be sent to or shared
/// with other threads. Thus, the guard ensures that only
/// one thread can dequeue entities from a local scheduler
/// at any given time.
pub struct LocalSchedulerGuard<'a, E>(&'a LocalScheduler<E>);

impl<E> !Send for LocalSchedulerGuard<'_, E> {}
impl<E> !Sync for LocalSchedulerGuard<'_, E> {}

impl<'a, E: SchedEntity> LocalSchedulerGuard<'a, E> {
    /// Dequeue an entity.
    ///
    /// The scheduler policy is to select the entity with the highest
    /// effective priority, which reflects how "interactive" the code
    /// of an entity is.
    ///
    /// If there are no entities to dequeue, the method will block.
    pub fn dequeue(&self) -> Option<Arc<E>> {
        self.0.dequeue()
    }
}

impl<E> Deref for LocalSchedulerGuard<'_, E> {
    type Target = LocalScheduler<E>;

    fn deref(&self) -> &LocalScheduler<E> {
        self.0
    }
}

impl<E> Drop for LocalSchedulerGuard<'_, E> {
    fn drop(&mut self) {
        self.0.is_locked.store(false, Release)
    }
}

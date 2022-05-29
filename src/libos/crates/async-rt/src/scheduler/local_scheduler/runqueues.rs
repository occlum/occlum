use crate::scheduler::{Priority, SchedEntity};
use std::collections::VecDeque;
use std::mem::{self, MaybeUninit};
use std::sync::Arc;

/// Runqueues of different priorities.
///
/// When an entity is to be enqueued into `RunQueues`,
/// we will first look up its effective priority and then
/// push the entity to the runqueue that corresponds to the priority.
///
/// When an entity is to be dequeued from `RunQueues`,
/// we will visit each runqueue in order of priority and
/// see if it it non-empty. If so, an entity is dequeued from it.
///
/// # Concurrency
///
/// `Runqueues` are designed for multi producers (which enqueue entities)
/// and a single consumer (which dequeues entities). Multiple consumers'
/// dequeueing entities concurrently may cause panics, but not undefined behaviors.
/// The multi-producer-single-consumer mode is compatible with how
/// `Scheduler` works. Each scheduler is per-VCPU: only one thread is supposed
/// to dequeue entities, while multiple threads may enqueue entities.
pub struct RunQueues<E> {
    // The i-th runqueue lists the entities with an effective priority equals to i.
    pub(super) run_queues: [RunQueue<E>; Priority::count()],
    // A bitmap where each bit indicates whether a corresponding
    // runqueue has any entities.
    pub(super) nonempty_mask: u32,
}

type RunQueue<E> = VecDeque<Arc<E>>;

// Make sure the assumptions on `Priority` are held.
//static_assert!(Priority::min() as u32 == 0);
//static_assert!(Priority::count() <= mem::size_of::<AtomicU32>() * 8);

impl<E: SchedEntity> RunQueues<E> {
    /// Create an instance.
    pub fn new() -> Self {
        let run_queues: [RunQueue<E>; Priority::count()] = {
            let mut run_queues: [MaybeUninit<RunQueue<E>>; Priority::count()] =
                MaybeUninit::uninit_array();
            for rq in &mut run_queues {
                rq.write(VecDeque::with_capacity(16));
            }
            // Safety. Every element in the array has been initialized.
            unsafe { mem::transmute(run_queues) }
        };
        Self {
            run_queues,
            nonempty_mask: 0,
        }
    }

    /// Enqueue an entity.
    pub fn enqueue(&mut self, entity: Arc<E>) {
        let rq_idx = {
            let sched_state = entity.sched_state();
            let effective_prio = sched_state.effective_prio();
            effective_prio.val() as usize
        };

        let rq = &mut self.run_queues[rq_idx];
        rq.push_back(entity);
        self.nonempty_mask |= 1 << rq_idx;
    }

    /// Try to dequeue an entity.
    pub fn try_dequeue(&mut self) -> Option<Arc<E>> {
        // Get the index of the non-empty runqueue with the highest priority
        let rq_i = {
            let num_leading_zeros = self.nonempty_mask.leading_zeros();
            if num_leading_zeros >= 32 {
                return None;
            }
            (32 - num_leading_zeros - 1) as usize
        };

        // Pop an entity from the runqueue
        {
            let rq = &mut self.run_queues[rq_i];
            let entity = rq
                .pop_front()
                .expect("the mask does not return false positive results");
            // If the runqueue turns empty, then clear the mask bit
            if rq.is_empty() {
                self.nonempty_mask &= !(1 << rq_i);
            }
            Some(entity)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::SchedState;

    #[derive(Debug)]
    struct DummyEntity(SchedState);

    impl DummyEntity {
        pub fn new(base_prio: Priority) -> Arc<Self> {
            let vcpus: u32 = 1;
            Arc::new(Self(SchedState::new(vcpus, base_prio)))
        }

        pub fn prio(&self) -> Priority {
            self.0.effective_prio()
        }
    }

    impl SchedEntity for DummyEntity {
        fn sched_state(&self) -> &SchedState {
            &self.0
        }
    }

    #[test]
    fn enqueue_dequeue() {
        let mut runqueues = RunQueues::new();

        // Enqueue without any specific order
        runqueues.enqueue(DummyEntity::new(Priority::NORMAL));
        runqueues.enqueue(DummyEntity::new(Priority::LOW));
        runqueues.enqueue(DummyEntity::new(Priority::HIGH));
        runqueues.enqueue(DummyEntity::new(Priority::HIGHEST));
        runqueues.enqueue(DummyEntity::new(Priority::LOWEST));

        // Dequeue in the order of priority
        assert!(runqueues.try_dequeue().unwrap().prio() == Priority::HIGHEST);
        assert!(runqueues.try_dequeue().unwrap().prio() == Priority::HIGH);
        assert!(runqueues.try_dequeue().unwrap().prio() == Priority::NORMAL);
        assert!(runqueues.try_dequeue().unwrap().prio() == Priority::LOW);
        assert!(runqueues.try_dequeue().unwrap().prio() == Priority::LOWEST);
    }
}

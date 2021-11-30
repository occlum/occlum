use futures::task::ArcWake;

use super::Task;
use crate::prelude::*;

/// Task Interrupt ReQuests (TIRQs).
///
/// # Motivation
///
/// In the `async_rt` crate, our primary means to put a task to sleep until some
/// interesting events happen are `Waiter`, in particular, the
/// `Waiter::wait`-family method. But what if some more urgent events
/// happen that need the task immediate attention? For example,
/// a kill signal. To kill a task gracefully, we want a task
/// that is blocking on a waiter to stop waiting. For this reason, we must
/// be able to _interrupt_.
///
/// # Overview
///
/// Task Interrupt ReQuests (TIRQs) is a mechanism to interrupt tasks.
/// In a sense, TIRQs to a task is like hardware IRQs to a x86 CPU core.
/// Each task is given 64 interrupt lines. An interrupt line has a boolean status:
/// set or clear. Initially, an interrupt line is clear. If we put a TIRQ on the line
/// then the status of the line becomes set. We can put TIRQs on a line
/// multiple times, but the status of the line remains set.
/// After handling the event represented by the interrupt line (and its TIRQs),
/// a handler can clear the line.
///
/// Putting a TIRQ on an interrupt line of a task _interrupts_ the task. By interrupt,
/// we mean that the following two consequencies for any waiter executed in the
/// context of the task:
///
/// 1. If the waiter has been waiting, then the waiter will stop waiting. (The
/// return value of the wait is most likely be `Err(EINTR)`. But it could also
/// be `Ok(())` or `Err(ETIMEDOUT)`.)
/// 2. If the waiter attempts to wait, then the waiter immediately returns with
/// `Err(EINTR)`.
///
/// Either way, the task can now proceed, having a chance to deal with the
/// urgent matters that trigger the TIRQ. And after handling the urgent
/// matters, the user code should clear the corresponding TIRQs properly.
///
/// There could be many reasons to interrupt depending the specific use case.
/// The user code decides which
/// interrupt line means which kind of reason. Similar to hardware IRQs,
/// we may want to disable certain IRQs in certain circumstances. Thus,
/// we also associate a task with a TIRQ mask. A TIRQ put on a masked
/// interrupt line is called _pending_: it won't interrupt the task until
/// the line is unmasked. And when the line is unmasked, the TIRQ becomes _active_.
///
/// # Example: kill a task gracefully
///
/// Consider the following async function.
///
/// ```no_run
/// use async_rt::wait::Waiter;
/// use errno::prelude::*;
///
/// async fn blocked_code() {
///     let waiter = Waiter::new();
///     let res = waiter.wait().await;
///     match res {
///         Err(e) => {
///             assert!(e.errno() == EINTR);
///         }
///         _ => {
///             panic!("impossible as there is no waker or timeout");
///         }
///     }
/// }
/// ```
///
/// As no one is going to wake up the waiter and the wait method does not
/// take a timeout, a task that executes the function will blocked forever,
/// having no chance to run to complete.
///
/// With TIRQs, such tasks can be interrupted.
///
/// ```no_run
/// # async fn blocked_code() {}
/// # async_rt::task::block_on(async {
/// // Spawn a task that will not exit---unless receiving TIRQs
/// let handle = async_rt::task::spawn(async {
///     blocked_code().await;
/// });
///
/// // Put a TIRQ on the task, causing the task to exit
/// handle.task().tirqs().put_req(0 /* use TIRQ line #0 */);
///
/// // Wait for the task to exit
/// handle.await;
/// # });
/// ```
///
/// # Limitations
///
/// Currently, TIRQs can only interrupt tasks that are blocked on waiters.
/// A manually-written future is not interruptible by TIRQs unless the
/// future is written to respect TIRQs (as the waiters do).
/// This may be considered a limitation or a feature depending on your perspective.
pub struct Tirqs(Mutex<Inner>);

struct Inner {
    reqs: u64,
    mask: u64,
}

impl Tirqs {
    /// Create an instance of `Tirqs` associated with a task.
    ///
    /// # Safety
    ///
    /// A `Tirqs` must be associated to a task; it is meaingless
    /// to create a `Tirqs` in the wild. For this reason, we assume
    /// that any instance of `Tirqs` is created as a field of `Task`.
    /// And it is an undefined behavior to call any methods of `Tirqs`
    /// beforing inserting the `Tirqs` into a `Task` as its field.
    /// In addition, we also assume that `Task` is always wrapped inside
    /// `Arc`, which is the case in the current implementation.
    /// Failing to satisfy the two assumptions leads to undefined behaviors.
    pub(crate) unsafe fn new() -> Self {
        Self(Mutex::new(Inner { reqs: 0, mask: 0 }))
    }

    /// Has any active TIRQs.
    pub fn has_active_reqs(&self) -> bool {
        let inner = self.0.lock();
        inner.active_reqs() != 0
    }

    /// Put a TIRQ on an interrupt line.
    pub fn put_req(&self, line_id: u32) {
        assert!(line_id < 64);

        // Set the TIRQ bit
        let mut inner = self.0.lock();
        let was_inactive = (inner.active_reqs() & (1 << line_id)) == 0;
        inner.reqs |= 1 << line_id;
        let is_active = (inner.active_reqs() & (1 << line_id)) != 0;
        drop(inner);

        // Waking up the task is only necessary if the status of the TIRQ line
        // transits from inactive to active.
        if was_inactive && is_active {
            ArcWake::wake(self.task());
        }
    }

    /// Clear a TIRQ (if there is one) of the current task.
    ///
    /// Note that a task's TIRQs can only be cleared by the task itself.
    /// This eliminates some race conditions that TIRQs may be lost due to
    /// the misuse of the API.
    pub fn clear_req(line_id: u32) {
        let current = crate::task::current::get();
        current.tirqs().do_clear_req(line_id);
    }

    /// Clear a TIRQ (if there is one) of a task.
    fn do_clear_req(&self, line_id: u32) {
        assert!(line_id < 64);
        let mut inner = self.0.lock();
        inner.reqs &= !(1 << line_id);
    }

    /// Clear all TIRQs on all the interrupt lines of the current task.
    ///
    /// Note that a task's TIRQs can only be cleared by the task itself.
    /// This eliminates some race conditions that TIRQs may be lost due to
    /// the misuse of the API.
    pub fn clear_all_reqs() {
        let current = crate::task::current::get();
        current.tirqs().do_clear_all_reqs();
    }

    /// Clear all TIRQs on all the interrupt lines.
    fn do_clear_all_reqs(&self) {
        let mut inner = self.0.lock();
        inner.reqs = 0;
    }

    /// Returns the TIRQ mask of a task.
    pub fn mask(&self) -> u64 {
        let inner = self.0.lock();
        inner.mask
    }

    /// Set a new TIRQ mask for the current task, returning the old one.
    ///
    /// Note that a task's TIRQ mask can only be set by the task itself.
    /// This eliminates some race conditions that TIRQs may be lost due to
    /// the misuse of the API.
    pub fn set_mask(new_mask: u64) -> u64 {
        let current = crate::task::current::get();
        current.tirqs().do_set_mask(new_mask)
    }

    /// Set a new TIRQ mask for a task, returning the old one.
    fn do_set_mask(&self, new_mask: u64) -> u64 {
        let mut inner = self.0.lock();
        let old_mask = inner.mask;
        inner.mask = new_mask;
        drop(inner);
        old_mask
    }

    /// Clear the TIRQ mask of the current task, returning the old one.
    ///
    /// Note that a task's TIRQ mask can only be updated by the task itself.
    /// This eliminates some race conditions that TIRQs may be lost due to
    /// the misuse of the API.
    pub fn clear_mask() -> u64 {
        Self::set_mask(0)
    }

    fn task(&self) -> Arc<Task> {
        // Safety. The tirqs must be a field of a task.
        unsafe { Task::from_tirqs(self) }
    }
}

impl Inner {
    pub fn active_reqs(&self) -> u64 {
        self.reqs & !(self.reqs & self.mask)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::wait::Waiter;

    async fn blocked_code() {
        let waiter = Waiter::new();
        let res = waiter.wait().await;
        match res {
            Err(e) => {
                assert!(e.errno() == EINTR);
            }
            _ => {
                panic!("impossible as there is no waker or timeout");
            }
        }
    }

    #[test]
    pub fn terminate_task() {
        crate::task::block_on(async {
            // Spawn a task that will not exit unless receiving TIRQs
            let handle = crate::task::spawn(async {
                blocked_code().await;
            });

            // Put a TIRQ on the task, causing the task to exit
            handle.task().tirqs().put_req(0);

            // Wait for the task to exit
            handle.await;
        });
    }

    #[test]
    pub fn count_events() {
        crate::task::block_on(async {
            // Urgent events that trigger TIRQs.
            #[derive(Clone, Copy, Debug, PartialEq)]
            enum UrgentEvent {
                General, // a general urgent event
                Exit,    // a task exit event
            }
            // The TIRQ line that indicates the occurances of urgent events.
            const UE_TIRQ: u32 = 0;
            // A channel that transfer urgent eventts
            let ue_channel = Arc::new(Mutex::new(VecDeque::<UrgentEvent>::new()));

            // A task that receives and counts urgent events
            let receiver_handle = {
                let ue_channel = ue_channel.clone();
                crate::task::spawn(async move {
                    let current = crate::task::current::get();
                    let mut ue_counter = 0;
                    loop {
                        blocked_code().await;

                        // Each loop will only handle at most one urgent event.
                        // This restriction makes it easier to "lose" events.
                        // Thus, we can demonstrate one way to deal with race
                        // conditions in the use of TIRQs.
                        let should_exit = handle_one_ue(&current, &ue_channel, &mut ue_counter);
                        if should_exit {
                            return ue_counter;
                        }
                    }

                    // Handle at most one urgent event.
                    fn handle_one_ue(
                        current: &Task,
                        ue_channel: &Mutex<VecDeque<UrgentEvent>>,
                        ue_counter: &mut u32,
                    ) -> bool {
                        Tirqs::clear_req(UE_TIRQ);

                        let mut ue_channel = ue_channel.lock().unwrap();
                        let ue = match ue_channel.pop_front() {
                            Some(ue) => ue,
                            None => return false,
                        };
                        let more_ues = !ue_channel.is_empty();
                        drop(ue_channel);

                        if more_ues {
                            let tirqs = current.tirqs();
                            tirqs.put_req(UE_TIRQ);
                        }

                        let should_exit = if ue != UrgentEvent::Exit {
                            *ue_counter += 1;
                            false
                        } else {
                            true
                        };
                        should_exit
                    }
                })
            };
            let receiver = receiver_handle.task().clone();

            let sent_ues: u32 = 5000;
            let sender_handle = {
                let ue_channel = ue_channel.clone();
                crate::task::spawn(async move {
                    let receiver = receiver;
                    (0..sent_ues).for_each(|_| {
                        let mut ue_channel = ue_channel.lock().unwrap();
                        ue_channel.push_back(UrgentEvent::General);
                        drop(ue_channel);

                        receiver.tirqs().put_req(UE_TIRQ);
                    });

                    let mut ue_channel = ue_channel.lock().unwrap();
                    ue_channel.push_back(UrgentEvent::Exit);
                })
            };

            sender_handle.await;
            let received_ues = receiver_handle.await;
            assert!(received_ues == sent_ues);
        });
    }
}

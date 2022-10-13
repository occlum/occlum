use super::file_range::OverlapWith;
use super::*;

use rcore_fs::vfs::AnyExt;
use std::collections::VecDeque;

/// List of File POSIX advisory range locks.
///
/// Rule of ordering:
/// Locks are sorted by owner process, then by the starting offset.
///
/// Rule of mergeing:
/// Adjacent and overlapping locks with same owner and type will be merged.
///
/// Rule of updating:
/// New locks with different type will replace or split the overlapping locks
/// if they have same owner.
///
pub struct RangeLockList {
    inner: RwLock<VecDeque<RangeLock>>,
}

impl RangeLockList {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(VecDeque::new()),
        }
    }

    pub fn test_lock(&self, lock: &mut RangeLock) {
        debug!("test_lock with RangeLock: {:?}", lock);
        let list = self.inner.read().unwrap();
        for existing_lock in list.iter() {
            if lock.conflict_with(existing_lock) {
                // Return the information about the conflict lock
                lock.reset_with(existing_lock);
                return;
            }
        }
        // The lock could be placed at this time
        lock.set_type(RangeLockType::F_UNLCK);
    }

    pub async fn set_lock(&self, lock: &RangeLock, is_nonblocking: bool) -> Result<()> {
        debug!(
            "set_lock with RangeLock: {:?}, is_nonblocking: {}",
            lock, is_nonblocking
        );

        loop {
            let mut list = self.inner.write().unwrap();
            if let Some(conflict_lock) = list.iter_mut().find(|l| l.conflict_with(lock)) {
                if is_nonblocking {
                    return_errno!(EAGAIN, "lock conflict, try again later");
                }
                // Start to wait
                let mut waiter = Waiter::new();
                // TODO: Add deadlock detection, and returns EDEADLK
                warn!("Do not support deadlock detection, maybe wait infinitely");
                conflict_lock.enqueue_waiter(&mut waiter);
                // Ensure that we drop any locks before wait
                drop(list);
                waiter.wait().await?;
                // Wake up, let's try to set lock again
                continue;
            }
            // No conflict here, let's insert the lock
            Self::insert_lock_into_list(&mut list, lock);
            break;
        }
        Ok(())
    }

    fn insert_lock_into_list(list: &mut RwLockWriteGuard<VecDeque<RangeLock>>, lock: &RangeLock) {
        let first_same_owner_idx = match list.iter().position(|lk| lk.owner() == lock.owner()) {
            Some(idx) => idx,
            None => {
                // Can't find existing locks with same owner.
                list.push_front(lock.clone());
                return;
            }
        };
        // Insert the lock at the start position with same owner, may breaking
        // the rules of RangeLockList.
        // We will handle the inserted lock with next one to adjust the list to
        // obey the rules.
        list.insert(first_same_owner_idx, lock.clone());
        let mut pre_idx = first_same_owner_idx;
        let mut next_idx = pre_idx + 1;
        loop {
            if next_idx >= list.len() {
                break;
            }
            let pre_lock = list[pre_idx].clone();
            let next_lock = list[next_idx].clone();

            if next_lock.owner() != pre_lock.owner() {
                break;
            }
            if next_lock.type_() == pre_lock.type_() {
                // Same type
                if pre_lock.end() < next_lock.start() {
                    break;
                } else if next_lock.end() < pre_lock.start() {
                    list.swap(pre_idx, next_idx);
                    pre_idx += 1;
                    next_idx += 1;
                } else {
                    // Merge adjacent or overlapping locks
                    list[next_idx].merge_with(&pre_lock);
                    list.remove(pre_idx);
                }
            } else {
                // Different type
                if pre_lock.end() <= next_lock.start() {
                    break;
                } else if next_lock.end() <= pre_lock.start() {
                    list.swap(pre_idx, next_idx);
                    pre_idx += 1;
                    next_idx += 1;
                } else {
                    // Split overlapping locks
                    let overlap_with = pre_lock.overlap_with(&next_lock).unwrap();
                    match overlap_with {
                        OverlapWith::ToLeft => {
                            list[next_idx].set_start(pre_lock.end());
                            break;
                        }
                        OverlapWith::InMiddle => {
                            let right_lk = {
                                let mut r_lk = next_lock.clone();
                                r_lk.set_start(pre_lock.end());
                                r_lk
                            };
                            list[next_idx].set_end(pre_lock.start());
                            list.swap(pre_idx, next_idx);
                            list.insert(next_idx + 1, right_lk);
                            break;
                        }
                        OverlapWith::ToRight => {
                            list[next_idx].set_end(pre_lock.start());
                            list.swap(pre_idx, next_idx);
                            pre_idx += 1;
                            next_idx += 1;
                        }
                        OverlapWith::Includes => {
                            // New lock can replace the old one
                            list.remove(next_idx);
                        }
                    }
                }
            }
        }
    }

    pub fn unlock(&self, lock: &RangeLock) {
        debug!("unlock with RangeLock: {:?}", lock);
        let mut list = self.inner.write().unwrap();
        let mut skipped = 0;
        loop {
            let idx = match list
                .iter()
                .skip(skipped)
                .position(|lk| lk.owner() == lock.owner())
            {
                // (idx + skipped) is the original position in list
                Some(idx) => idx + skipped,
                None => break,
            };

            let existing_lock = &mut list[idx];
            let overlap_with = match lock.overlap_with(existing_lock) {
                Some(overlap_with) => overlap_with,
                None => {
                    skipped = idx + 1;
                    continue;
                }
            };

            match overlap_with {
                OverlapWith::ToLeft => {
                    existing_lock.set_start(lock.end());
                    break;
                }
                OverlapWith::InMiddle => {
                    // Split the lock
                    let right_lk = {
                        let mut r_lk = existing_lock.clone();
                        r_lk.set_start(lock.end());
                        r_lk
                    };
                    existing_lock.set_end(lock.start());
                    list.insert(idx + 1, right_lk);
                    break;
                }
                OverlapWith::ToRight => {
                    existing_lock.set_end(lock.start());
                    skipped = idx + 1;
                }
                OverlapWith::Includes => {
                    // The lock can be deleted from the list
                    list.remove(idx);
                    skipped = idx;
                }
            }
        }
    }
}

impl Default for RangeLockList {
    fn default() -> Self {
        Self::new()
    }
}

impl AnyExt for RangeLockList {}

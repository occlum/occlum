use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::intrinsics::atomic_load;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_rt::wait::{Waiter, Waker};

use crate::prelude::*;

/// `FutexOp`, `FutexFlags`, and `futex_op_and_flags_from_u32` are helper types and
/// functions for handling the versatile commands and arguments of futex system
/// call in a memory-safe way.

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq)]
pub enum FutexOp {
    FUTEX_WAIT = 0,
    FUTEX_WAKE = 1,
    FUTEX_FD = 2,
    FUTEX_REQUEUE = 3,
    FUTEX_CMP_REQUEUE = 4,
    FUTEX_WAKE_OP = 5,
    FUTEX_LOCK_PI = 6,
    FUTEX_UNLOCK_PI = 7,
    FUTEX_TRYLOCK_PI = 8,
    FUTEX_WAIT_BITSET = 9,
    FUTEX_WAKE_BITSET = 10,
}
const FUTEX_OP_MASK: u32 = 0x0000_000F;

impl FutexOp {
    pub fn from_u32(bits: u32) -> Result<FutexOp> {
        match bits {
            0 => Ok(FutexOp::FUTEX_WAIT),
            1 => Ok(FutexOp::FUTEX_WAKE),
            2 => Ok(FutexOp::FUTEX_FD),
            3 => Ok(FutexOp::FUTEX_REQUEUE),
            4 => Ok(FutexOp::FUTEX_CMP_REQUEUE),
            5 => Ok(FutexOp::FUTEX_WAKE_OP),
            6 => Ok(FutexOp::FUTEX_LOCK_PI),
            7 => Ok(FutexOp::FUTEX_UNLOCK_PI),
            8 => Ok(FutexOp::FUTEX_TRYLOCK_PI),
            9 => Ok(FutexOp::FUTEX_WAIT_BITSET),
            10 => Ok(FutexOp::FUTEX_WAKE_BITSET),
            _ => return_errno!(EINVAL, "Unknown futex op"),
        }
    }
}

bitflags! {
    pub struct FutexFlags : u32 {
        const FUTEX_PRIVATE         = 128;
        const FUTEX_CLOCK_REALTIME  = 256;
    }
}
const FUTEX_FLAGS_MASK: u32 = 0xFFFF_FFF0;

impl FutexFlags {
    pub fn from_u32(bits: u32) -> Result<FutexFlags> {
        FutexFlags::from_bits(bits).ok_or_else(|| errno!(EINVAL, "unknown futex flags"))
    }
}

pub fn futex_op_and_flags_from_u32(bits: u32) -> Result<(FutexOp, FutexFlags)> {
    let op = {
        let op_bits = bits & FUTEX_OP_MASK;
        FutexOp::from_u32(op_bits)?
    };
    let flags = {
        let flags_bits = bits & FUTEX_FLAGS_MASK;
        FutexFlags::from_u32(flags_bits)?
    };
    Ok((op, flags))
}

const FUTEX_BITSET_MATCH_ANY: u32 = 0xFFFF_FFFF;

/// Do futex wait
pub async fn futex_wait(
    futex_addr: *const i32,
    futex_val: i32,
    timeout: &Option<Duration>,
) -> Result<()> {
    futex_wait_bitset(futex_addr, futex_val, timeout, FUTEX_BITSET_MATCH_ANY).await
}

/// Do futex wait with bitset
pub async fn futex_wait_bitset(
    futex_addr: *const i32,
    futex_val: i32,
    timeout: &Option<Duration>,
    bitset: u32,
) -> Result<()> {
    debug!(
        "futex_wait_bitset addr: {:#x}, val: {}, timeout: {:?}, bitset: {:#x}",
        futex_addr as usize, futex_val, timeout, bitset
    );
    // Get and lock the futex bucket
    let futex_key = FutexKey::new(futex_addr);
    let (_, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(futex_key);
    let mut futex_bucket = futex_bucket_ref.lock().unwrap();

    // Check the futex value
    if futex_key.load_val() != futex_val {
        return_errno!(EAGAIN, "futex value does not match");
    }
    // Why we first lock the bucket then check the futex value?
    //
    // CPU 0 <Waiter>                       CPU 1 <Waker>
    // (user mode)
    // val = *futex_addr;
    // syscall(FUTEX_WAIT);
    // (kernel mode)
    // futex_wait(futex_addr, val) {
    //   bucket = get_bucket();
    //   actual_val = *futex_addr;
    //                                      (user mode)
    //                                      *futex_addr = new_val;
    //                                      syscall(FUTEX_WAKE);
    //                                      (kernel mode)
    //                                      futex_wake(futex_addr) {
    //                                        bucket = get_bucket();
    //                                        lock(bucket);
    //                                        bucket.dequeue_and_wake_items()
    //                                        unlock(bucket);
    //                                        return;
    //                                      }
    //   if actual_val == val {
    //     lock(bucket);
    //     bucket.enqueue_item();
    //     unlock(bucket);
    //     wait();
    //   }
    // }
    // If the waiter on CPU 0 does not lock the bucket before check the futex value,
    // it cannot find the transition of futex value from val to new_val and enqueue
    // to the bucket, which will cause the waiter to wait forever.

    let waiter = Waiter::new();

    let futex_item = FutexItem::new(futex_key, bitset, waiter.waker());
    futex_bucket.enqueue_item(futex_item.clone());
    // Must make sure that no locks are holded by this thread before wait
    drop(futex_bucket);

    let mut timeout = timeout.map_or(None, |t| Some(t));
    // Wait until we wake it up or reach timeout.
    if let Err(e) = waiter.wait_timeout(timeout.as_mut()).await {
        let (_, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(futex_item.key);
        let mut futex_bucket = futex_bucket_ref.lock().unwrap();
        futex_bucket.dequeue_item(&futex_item);
        return_errno!(e.errno(), "futex wait timeout or interrupted");
    }

    Ok(())
}

/// Do futex wake
pub fn futex_wake(futex_addr: *const i32, max_count: usize) -> Result<usize> {
    futex_wake_bitset(futex_addr, max_count, FUTEX_BITSET_MATCH_ANY)
}

/// Do futex wake with bitset
pub fn futex_wake_bitset(futex_addr: *const i32, max_count: usize, bitset: u32) -> Result<usize> {
    debug!(
        "futex_wake_bitset addr: {:#x}, max_count: {}, bitset: {:#x}",
        futex_addr as usize, max_count, bitset
    );

    // Get and lock the futex bucket
    let futex_key = FutexKey::new(futex_addr);
    let (_, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(futex_key);
    let mut futex_bucket = futex_bucket_ref.lock().unwrap();

    // Dequeue and wake up the items in the bucket
    let count = futex_bucket.dequeue_and_wake_items(futex_key, max_count, bitset);
    Ok(count)
}

/// Do futex requeue
pub fn futex_requeue(
    futex_addr: *const i32,
    max_nwakes: usize,
    max_nrequeues: usize,
    futex_new_addr: *const i32,
) -> Result<usize> {
    if futex_new_addr == futex_addr {
        return futex_wake(futex_addr, max_nwakes);
    }
    let futex_key = FutexKey::new(futex_addr);
    let futex_new_key = FutexKey::new(futex_new_addr);
    let (bucket_idx, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(futex_key);
    let (new_bucket_idx, futex_new_bucket_ref) = FUTEX_BUCKETS.get_bucket(futex_new_key);
    let nwakes = {
        if bucket_idx != new_bucket_idx {
            let (mut futex_bucket, mut futex_new_bucket) = {
                if bucket_idx < new_bucket_idx {
                    let mut futex_bucket = futex_bucket_ref.lock().unwrap();
                    let mut futex_new_bucket = futex_new_bucket_ref.lock().unwrap();
                    (futex_bucket, futex_new_bucket)
                } else {
                    // bucket_idx > new_bucket_idx
                    let mut futex_new_bucket = futex_new_bucket_ref.lock().unwrap();
                    let mut futex_bucket = futex_bucket_ref.lock().unwrap();
                    (futex_bucket, futex_new_bucket)
                }
            };
            let nwakes =
                futex_bucket.dequeue_and_wake_items(futex_key, max_nwakes, FUTEX_BITSET_MATCH_ANY);
            futex_bucket.requeue_items_to_another_bucket(
                futex_key,
                &mut futex_new_bucket,
                futex_new_key,
                max_nrequeues,
            );
            nwakes
        } else {
            // bucket_idx == new_bucket_idx
            let mut futex_bucket = futex_bucket_ref.lock().unwrap();
            let nwakes =
                futex_bucket.dequeue_and_wake_items(futex_key, max_nwakes, FUTEX_BITSET_MATCH_ANY);
            futex_bucket.update_item_keys(futex_key, futex_new_key, max_nrequeues);
            nwakes
        }
    };
    Ok(nwakes)
}

lazy_static! {
    // Use the same count as linux kernel to keep the same performance
    static ref BUCKET_COUNT: usize = ((1 << 8) * (*crate::sched::NCORES)).next_power_of_two();
    static ref BUCKET_MASK: usize = *BUCKET_COUNT - 1;
    static ref FUTEX_BUCKETS: FutexBucketVec = { FutexBucketVec::new(*BUCKET_COUNT) };
}

#[derive(PartialEq, Copy, Clone)]
struct FutexKey(usize);

impl FutexKey {
    pub fn new(addr: *const i32) -> FutexKey {
        FutexKey(addr as usize)
    }

    pub fn load_val(&self) -> i32 {
        unsafe { atomic_load(self.0 as *const i32) }
    }

    pub fn addr(&self) -> usize {
        self.0
    }
}

#[derive(Clone)]
struct FutexItem {
    key: FutexKey,
    bitset: u32,
    waker: Waker,
}

impl FutexItem {
    pub fn new(key: FutexKey, bitset: u32, waker: Waker) -> FutexItem {
        FutexItem { key, bitset, waker }
    }

    pub fn wake(&self) {
        self.waker.wake();
    }

    pub fn batch_wake(items: &[FutexItem]) {
        for item in items {
            item.waker.wake();
        }
    }
}

impl PartialEq for FutexItem {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

struct FutexBucket {
    queue: VecDeque<FutexItem>,
}

type FutexBucketRef = Arc<SgxMutex<FutexBucket>>;

impl FutexBucket {
    pub fn new() -> FutexBucket {
        FutexBucket {
            queue: VecDeque::new(),
        }
    }

    pub fn enqueue_item(&mut self, item: FutexItem) {
        self.queue.push_back(item);
    }

    // TODO: this is an O(N) operation. Try to make it more efficient
    pub fn dequeue_item(&mut self, futex_item: &FutexItem) -> Option<FutexItem> {
        let item_i = self.queue.iter().position(|item| *item == *futex_item);
        if item_i.is_none() {
            return None;
        }
        self.queue.remove(item_i.unwrap())
    }

    // TODO: consider using std::future to improve the readability
    pub fn dequeue_and_wake_items(
        &mut self,
        key: FutexKey,
        max_count: usize,
        bitset: u32,
    ) -> usize {
        let mut count = 0;
        let mut items_to_wake = Vec::new();

        self.queue.retain(|item| {
            if count >= max_count || key != item.key || (bitset & item.bitset) == 0 {
                true
            } else {
                items_to_wake.push(item.clone());
                count += 1;
                false
            }
        });

        FutexItem::batch_wake(&items_to_wake);
        count
    }

    pub fn update_item_keys(&mut self, key: FutexKey, new_key: FutexKey, max_count: usize) -> () {
        let mut count = 0;
        for item in self.queue.iter_mut() {
            if count == max_count {
                break;
            }
            if (*item).key == key {
                (*item).key = new_key;
                count += 1;
            }
        }
    }

    pub fn requeue_items_to_another_bucket(
        &mut self,
        key: FutexKey,
        another: &mut Self,
        new_key: FutexKey,
        max_nrequeues: usize,
    ) -> () {
        let mut count = 0;

        self.queue.retain(|item| {
            if count >= max_nrequeues || key != item.key {
                true
            } else {
                let mut new_item = item.clone();
                new_item.key = new_key;
                another.enqueue_item(new_item);
                count += 1;
                false
            }
        });
    }
}

struct FutexBucketVec {
    vec: Vec<FutexBucketRef>,
}

impl FutexBucketVec {
    pub fn new(size: usize) -> FutexBucketVec {
        let mut buckets = FutexBucketVec {
            vec: Vec::with_capacity(size),
        };
        for idx in 0..size {
            let bucket = Arc::new(SgxMutex::new(FutexBucket::new()));
            buckets.vec.push(bucket);
        }
        buckets
    }

    pub fn get_bucket(&self, key: FutexKey) -> (usize, FutexBucketRef) {
        let idx = *BUCKET_MASK & {
            // The addr is the multiples of 4, so we ignore the last 2 bits
            let addr = key.addr() >> 2;
            let mut s = DefaultHasher::new();
            addr.hash(&mut s);
            s.finish() as usize
        };
        (idx, self.vec[idx].clone())
    }
}

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::intrinsics::atomic_load;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::prelude::*;
use crate::time::{timespec_t, ClockID};

/// `FutexOp`, `FutexFlags`, and `futex_op_and_flags_from_u32` are helper types and
/// functions for handling the versatile commands and arguments of futex system
/// call in a memory-safe way.
#[derive(PartialEq)]
#[allow(non_camel_case_types)]
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

#[derive(Debug, Copy, Clone)]
pub struct FutexTimeout {
    clock_id: ClockID,
    ts: timespec_t,
    absolute_time: bool,
}

impl FutexTimeout {
    pub fn new(clock_id: ClockID, ts: timespec_t, absolute_time: bool) -> Self {
        Self {
            clock_id,
            ts,
            absolute_time,
        }
    }

    pub fn clock_id(&self) -> &ClockID {
        &self.clock_id
    }

    pub fn ts(&self) -> &timespec_t {
        &self.ts
    }

    pub fn absolute_time(&self) -> bool {
        self.absolute_time
    }
}

/// Do futex wait
pub fn futex_wait(
    futex_addr: *const i32,
    futex_val: i32,
    timeout: &Option<FutexTimeout>,
) -> Result<()> {
    futex_wait_bitset(futex_addr, futex_val, timeout, FUTEX_BITSET_MATCH_ANY)
}

/// Do futex wait with bitset
pub fn futex_wait_bitset(
    futex_addr: *const i32,
    futex_val: i32,
    timeout: &Option<FutexTimeout>,
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
    // If the waiter on CPU 0 does not lock the bucket before check the futex velue,
    // it cannot find the transition of futex value from val to new_val and enqueue
    // to the bucket, which will cause the waiter to wait forever.

    let futex_item = FutexItem::new(futex_key, bitset);
    futex_bucket.enqueue_item(futex_item.clone());

    // Must make sure that no locks are holded by this thread before wait
    drop(futex_bucket);
    futex_item.wait(timeout)
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

#[derive(Clone, PartialEq)]
struct FutexItem {
    key: FutexKey,
    bitset: u32,
    waiter: WaiterRef,
}

impl FutexItem {
    pub fn new(key: FutexKey, bitset: u32) -> FutexItem {
        FutexItem {
            key,
            bitset,
            waiter: Arc::new(Waiter::new()),
        }
    }

    pub fn wake(&self) {
        self.waiter().wake()
    }

    pub fn wait(&self, timeout: &Option<FutexTimeout>) -> Result<()> {
        if let Err(e) = self.waiter.wait_timeout(&timeout) {
            let (_, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(self.key);
            let mut futex_bucket = futex_bucket_ref.lock().unwrap();
            futex_bucket.dequeue_item(self);
            return_errno!(e.errno(), "futex wait timeout or interrupted");
        }
        Ok(())
    }

    pub fn waiter(&self) -> &WaiterRef {
        &self.waiter
    }

    pub fn batch_wake(items: &[FutexItem]) {
        let waiters: Vec<&WaiterRef> = items.iter().map(|item| item.waiter()).collect();
        Waiter::batch_wake(&waiters);
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

#[derive(Debug)]
struct Waiter {
    thread: *const c_void,
    is_woken: AtomicBool,
}

type WaiterRef = Arc<Waiter>;

impl Waiter {
    pub fn new() -> Waiter {
        Waiter {
            thread: unsafe { sgx_thread_get_self() },
            is_woken: AtomicBool::new(false),
        }
    }

    pub fn wait_timeout(&self, timeout: &Option<FutexTimeout>) -> Result<()> {
        let current = unsafe { sgx_thread_get_self() };
        if current != self.thread {
            return Ok(());
        }
        while self.is_woken.load(Ordering::SeqCst) == false {
            if let Err(e) = wait_event_timeout(self.thread, timeout) {
                self.is_woken.store(true, Ordering::SeqCst);
                return_errno!(e.errno(), "wait_timeout error");
            }
        }
        Ok(())
    }

    pub fn wake(&self) {
        if self.is_woken().fetch_or(true, Ordering::SeqCst) == false {
            set_events(&[self.thread])
        }
    }

    pub fn thread(&self) -> *const c_void {
        self.thread
    }

    pub fn is_woken(&self) -> &AtomicBool {
        &self.is_woken
    }

    pub fn batch_wake(waiters: &[&WaiterRef]) {
        let threads: Vec<*const c_void> = waiters
            .iter()
            .filter_map(|waiter| {
                // Only wake up items that are not woken.
                // Set the item to be woken if it is not woken.
                if waiter.is_woken().fetch_or(true, Ordering::SeqCst) == false {
                    Some(waiter.thread())
                } else {
                    None
                }
            })
            .collect();

        set_events(&threads);
    }
}

impl PartialEq for Waiter {
    fn eq(&self, other: &Self) -> bool {
        self.thread == other.thread
    }
}

unsafe impl Send for Waiter {}
unsafe impl Sync for Waiter {}

fn wait_event_timeout(thread: *const c_void, timeout: &Option<FutexTimeout>) -> Result<()> {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    let (clockbit, ts_ptr, absolute_time) = timeout
        .as_ref()
        .map(|timeout| {
            let clockbit = match timeout.clock_id() {
                ClockID::CLOCK_REALTIME => FutexFlags::FUTEX_CLOCK_REALTIME.bits() as i32,
                _ => 0,
            };
            (
                clockbit,
                timeout.ts() as *const timespec_t,
                timeout.absolute_time() as i32,
            )
        })
        .unwrap_or((0, 0 as *const _, 0));
    let mut errno: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_wait_untrusted_event_timeout_ocall(
            &mut ret as *mut c_int,
            thread,
            clockbit,
            ts_ptr,
            absolute_time,
            &mut errno as *mut c_int,
        );
        assert!(sgx_ret == 0);
        assert!(ret == 0);
    }
    if errno != 0 {
        // Do sanity check here, only possible errnos here are ETIMEDOUT, EAGAIN and EINTR
        assert!(
            (timeout.is_some() && errno == Errno::ETIMEDOUT as i32)
                || errno == Errno::EINTR as i32
                || errno == Errno::EAGAIN as i32
        );
        return_errno!(
            Errno::from(errno as u32),
            "sgx_thread_wait_untrusted_event_timeout_ocall error"
        );
    }
    Ok(())
}

fn set_events(threads: &[*const c_void]) {
    if threads.is_empty() {
        return;
    }

    let mut ret: c_int = 0;
    let sgx_ret = unsafe {
        sgx_thread_set_multiple_untrusted_events_ocall(
            &mut ret as *mut c_int,
            threads.as_ptr(),
            threads.len(),
        )
    };

    assert!(
        ret == 0 && sgx_ret == 0,
        "ERROR: sgx_thread_set_multiple_untrusted_events_ocall failed"
    );
}

extern "C" {
    fn sgx_thread_get_self() -> *const c_void;

    fn sgx_thread_wait_untrusted_event_timeout_ocall(
        ret: *mut c_int,
        self_thread: *const c_void,
        clockbit: i32,
        ts: *const timespec_t,
        absolute_time: i32,
        errno: *mut c_int,
    ) -> c_int;

    fn sgx_thread_set_multiple_untrusted_events_ocall(
        ret: *mut c_int,
        waiters: *const *const c_void,
        total: usize,
    ) -> c_int;
}

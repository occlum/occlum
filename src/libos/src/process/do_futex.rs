use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::intrinsics::atomic_load;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::prelude::*;
use crate::time::timespec_t;

/// `FutexOp`, `FutexFlags`, and `futex_op_and_flags_from_u32` are helper types and
/// functions for handling the versatile commands and arguments of futex system
/// call in a memory-safe way.

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

/// Do futex wait
pub fn futex_wait(
    futex_addr: *const i32,
    futex_val: i32,
    timeout: &Option<timespec_t>,
) -> Result<()> {
    info!(
        "futex_wait addr: {:#x}, val: {}, timeout: {:?}",
        futex_addr as usize, futex_val, timeout
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

    let futex_item = FutexItem::new(futex_key);
    futex_bucket.enqueue_item(futex_item.clone());

    // Must make sure that no locks are holded by this thread before wait
    drop(futex_bucket);
    futex_item.wait_timeout(timeout)
}

/// Do futex wake
pub fn futex_wake(futex_addr: *const i32, max_count: usize) -> Result<usize> {
    // Get and lock the futex bucket
    let futex_key = FutexKey::new(futex_addr);
    let (_, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(futex_key);
    let mut futex_bucket = futex_bucket_ref.lock().unwrap();

    // Dequeue and wake up the items in the bucket
    let count = futex_bucket.dequeue_and_wake_items(futex_key, max_count);
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
            let nwakes = futex_bucket.dequeue_and_wake_items(futex_key, max_nwakes);
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
            let nwakes = futex_bucket.dequeue_and_wake_items(futex_key, max_nwakes);
            futex_bucket.update_item_keys(futex_key, futex_new_key, max_nrequeues);
            nwakes
        }
    };
    Ok(nwakes)
}

// Make sure futex bucket count is the power of 2
const BUCKET_COUNT: usize = 1 << 8;
const BUCKET_MASK: usize = BUCKET_COUNT - 1;

lazy_static! {
    static ref FUTEX_BUCKETS: FutexBucketVec = { FutexBucketVec::new(BUCKET_COUNT) };
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
    waiter: WaiterRef,
}

impl FutexItem {
    pub fn new(key: FutexKey) -> FutexItem {
        FutexItem {
            key: key,
            waiter: Arc::new(Waiter::new()),
        }
    }

    pub fn wake(&self) {
        self.waiter.wake()
    }

    pub fn wait_timeout(&self, timeout: &Option<timespec_t>) -> Result<()> {
        match timeout {
            None => self.waiter.wait(),
            Some(ts) => {
                if let Err(e) = self.waiter.wait_timeout(&ts) {
                    let (_, futex_bucket_ref) = FUTEX_BUCKETS.get_bucket(self.key);
                    let mut futex_bucket = futex_bucket_ref.lock().unwrap();
                    futex_bucket.dequeue_item(self);
                    return_errno!(e.errno(), "futex wait with timeout error");
                }
                Ok(())
            }
        }
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

    pub fn dequeue_item(&mut self, futex_item: &FutexItem) -> Option<FutexItem> {
        let item_i = self.queue.iter().position(|item| *item == *futex_item);
        if item_i.is_none() {
            return None;
        }
        self.queue.swap_remove_back(item_i.unwrap())
    }

    pub fn dequeue_and_wake_items(&mut self, key: FutexKey, max_count: usize) -> usize {
        let mut count = 0;
        let mut idx = 0;
        while count < max_count && idx < self.queue.len() {
            if key == self.queue[idx].key {
                if let Some(item) = self.queue.swap_remove_back(idx) {
                    item.wake();
                    count += 1;
                }
            } else {
                idx += 1;
            }
        }
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
        let mut idx = 0;
        while count < max_nrequeues && idx < self.queue.len() {
            if key == self.queue[idx].key {
                if let Some(mut item) = self.queue.swap_remove_back(idx) {
                    item.key = new_key;
                    another.enqueue_item(item);
                    count += 1;
                }
            } else {
                idx += 1;
            }
        }
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
        let idx = BUCKET_MASK & {
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

    pub fn wait(&self) -> Result<()> {
        let current = unsafe { sgx_thread_get_self() };
        if current != self.thread {
            return Ok(());
        }
        while self.is_woken.load(Ordering::SeqCst) == false {
            wait_event(self.thread);
        }
        Ok(())
    }

    pub fn wait_timeout(&self, timeout: &timespec_t) -> Result<()> {
        let current = unsafe { sgx_thread_get_self() };
        if current != self.thread {
            return Ok(());
        }
        while self.is_woken.load(Ordering::SeqCst) == false {
            if let Err(e) = wait_event_timeout(self.thread, timeout) {
                self.is_woken.store(true, Ordering::SeqCst);
                // Do sanity check here, only possible errnos here are ETIMEDOUT, EAGAIN and EINTR
                debug_assert!(e.errno() == ETIMEDOUT || e.errno() == EAGAIN || e.errno() == EINTR);
                return_errno!(e.errno(), "wait_timeout error");
            }
        }
        Ok(())
    }

    pub fn wake(&self) {
        if self.is_woken.fetch_or(true, Ordering::SeqCst) == false {
            set_event(self.thread);
        }
    }
}

impl PartialEq for Waiter {
    fn eq(&self, other: &Self) -> bool {
        self.thread == other.thread
    }
}

unsafe impl Send for Waiter {}
unsafe impl Sync for Waiter {}

fn wait_event(thread: *const c_void) {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_wait_untrusted_event_ocall(&mut ret as *mut c_int, thread);
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: sgx_thread_wait_untrusted_event_ocall failed");
    }
}

fn wait_event_timeout(thread: *const c_void, timeout: &timespec_t) -> Result<()> {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    let mut errno: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_wait_untrusted_event_timeout_ocall(
            &mut ret as *mut c_int,
            thread,
            timeout.sec(),
            timeout.nsec(),
            &mut errno as *mut c_int,
        );
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: sgx_thread_wait_untrusted_event_timeout_ocall failed");
    }
    if errno != 0 {
        return_errno!(
            Errno::from(errno as u32),
            "sgx_thread_wait_untrusted_event_timeout_ocall error"
        );
    }
    Ok(())
}

fn set_event(thread: *const c_void) {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_set_untrusted_event_ocall(&mut ret as *mut c_int, thread);
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: sgx_thread_set_untrusted_event_ocall failed");
    }
}

extern "C" {
    fn sgx_thread_get_self() -> *const c_void;

    /* Go outside and wait on my untrusted event */
    fn sgx_thread_wait_untrusted_event_ocall(ret: *mut c_int, self_thread: *const c_void) -> c_int;

    fn sgx_thread_wait_untrusted_event_timeout_ocall(
        ret: *mut c_int,
        self_thread: *const c_void,
        sec: c_long,
        nsec: c_long,
        errno: *mut c_int,
    ) -> c_int;

    /* Wake a thread waiting on its untrusted event */
    fn sgx_thread_set_untrusted_event_ocall(ret: *mut c_int, waiter_thread: *const c_void)
        -> c_int;
}

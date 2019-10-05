use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

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
pub fn futex_wait(futex_addr: *const i32, futex_val: i32) -> Result<()> {
    let futex_key = FutexKey::new(futex_addr);
    let futex_item = FUTEX_TABLE.lock().unwrap().get_or_new_item(futex_key);

    futex_item.wait(futex_val);

    FUTEX_TABLE.lock().unwrap().put_item(futex_item);
    Ok(())
}

/// Do futex wake
pub fn futex_wake(futex_addr: *const i32, max_count: usize) -> Result<usize> {
    let futex_key = FutexKey::new(futex_addr);
    let futex_item = FUTEX_TABLE.lock().unwrap().get_item(futex_key)?;
    let count = futex_item.wake(max_count);
    FUTEX_TABLE.lock().unwrap().put_item(futex_item);
    Ok(count)
}

lazy_static! {
    static ref FUTEX_TABLE: SgxMutex<FutexTable> = { SgxMutex::new(FutexTable::new()) };
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
struct FutexKey(usize);

impl FutexKey {
    pub fn new(addr: *const i32) -> FutexKey {
        FutexKey(addr as usize)
    }

    pub fn load_val(&self) -> i32 {
        unsafe { *(self.0 as *const i32) }
    }
}

struct FutexItem {
    key: FutexKey,
    queue: SgxMutex<VecDeque<WaiterRef>>,
}

impl FutexItem {
    pub fn new(key: FutexKey) -> FutexItem {
        FutexItem {
            key: key,
            queue: SgxMutex::new(VecDeque::new()),
        }
    }

    pub fn wake(&self, max_count: usize) -> usize {
        let mut queue = self.queue.lock().unwrap();
        let mut count = 0;
        while count < max_count {
            let waiter = {
                let waiter_option = queue.pop_front();
                if waiter_option.is_none() {
                    break;
                }
                waiter_option.unwrap()
            };
            waiter.wake();
            count += 1;
        }
        count
    }

    pub fn wait(&self, futex_val: i32) -> () {
        let mut queue = self.queue.lock().unwrap();
        if self.key.load_val() != futex_val {
            return;
        }

        let waiter = Arc::new(Waiter::new());
        queue.push_back(waiter.clone());
        drop(queue);

        // Must make sure that no locks are holded by this thread before sleep
        waiter.wait();
    }
}

type FutexItemRef = Arc<FutexItem>;

struct FutexTable {
    table: HashMap<FutexKey, FutexItemRef>,
}

impl FutexTable {
    pub fn new() -> FutexTable {
        FutexTable {
            table: HashMap::new(),
        }
    }

    pub fn get_or_new_item(&mut self, key: FutexKey) -> FutexItemRef {
        let table = &mut self.table;
        let item = table
            .entry(key)
            .or_insert_with(|| Arc::new(FutexItem::new(key)));
        item.clone()
    }

    pub fn get_item(&mut self, key: FutexKey) -> Result<FutexItemRef> {
        let table = &mut self.table;
        table
            .get_mut(&key)
            .map(|item| item.clone())
            .ok_or_else(|| errno!(ENOENT, "futex key cannot be found"))
    }

    pub fn put_item(&mut self, item: FutexItemRef) {
        let table = &mut self.table;
        // If there are only two references, one is the given argument, the
        // other in the table, then it is time to release the futex item.
        // This is because we are holding the lock of futex table and the
        // reference count cannot be possibly increased by other threads.
        if Arc::strong_count(&item) == 2 {
            // Release the last but one reference
            let key = item.key;
            drop(item);
            // Release the last reference
            table.remove(&key);
        }
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

    pub fn wait(&self) {
        while self.is_woken.load(Ordering::SeqCst) != true {
            wait_event(self.thread);
        }
    }

    pub fn wake(&self) {
        self.is_woken.store(true, Ordering::SeqCst);
        set_event(self.thread);
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

    /* Wake a thread waiting on its untrusted event */
    fn sgx_thread_set_untrusted_event_ocall(ret: *mut c_int, waiter_thread: *const c_void)
        -> c_int;
}

/// A wait/wakeup mechanism that connects wait4 and exit system calls.
use crate::prelude::*;

#[derive(Debug)]
pub struct Waiter<D, R>
where
    D: Sized + Copy,
    R: Sized + Copy,
{
    inner: Arc<SgxMutex<WaiterInner<D, R>>>,
    thread: *const c_void,
}

unsafe impl<D, R> Send for Waiter<D, R>
where
    D: Sized + Copy,
    R: Sized + Copy,
{
}

#[derive(Debug)]
struct WaiterInner<D, R>
where
    D: Sized + Copy,
    R: Sized + Copy,
{
    is_woken: bool,
    data: D,
    result: Option<R>,
}

impl<D, R> Waiter<D, R>
where
    D: Sized + Copy,
    R: Sized + Copy,
{
    pub fn new(data: &D) -> Waiter<D, R> {
        Waiter {
            thread: unsafe { sgx_thread_get_self() },
            inner: Arc::new(SgxMutex::new(WaiterInner {
                is_woken: false,
                data: *data,
                result: None,
            })),
        }
    }

    pub fn get_data(&self) -> D {
        self.inner.lock().unwrap().data
    }

    pub fn sleep_until_woken_with_result(self) -> R {
        while !self.inner.lock().unwrap().is_woken {
            unsafe {
                wait_event(self.thread);
            }
        }

        self.inner.lock().unwrap().result.unwrap()
    }
}

#[derive(Debug)]
pub struct WaitQueue<D, R>
where
    D: Sized + Copy,
    R: Sized + Copy,
{
    waiters: Vec<Waiter<D, R>>,
}

impl<D, R> WaitQueue<D, R>
where
    D: Sized + Copy,
    R: Sized + Copy,
{
    pub fn new() -> WaitQueue<D, R> {
        WaitQueue {
            waiters: Vec::new(),
        }
    }

    pub fn add_waiter(&mut self, waiter: &Waiter<D, R>) -> () {
        self.waiters.push(Waiter {
            thread: waiter.thread,
            inner: waiter.inner.clone(),
        });
    }

    pub fn del_and_wake_one_waiter<F>(&mut self, cond: F) -> usize
    where
        F: Fn(&D) -> Option<R>,
    {
        let mut waiters = &mut self.waiters;
        let del_waiter_i = {
            let waiter_i = waiters.iter().position(|waiter| {
                let mut waiter_inner = waiter.inner.lock().unwrap();
                if let Some(waiter_result) = cond(&waiter_inner.data) {
                    waiter_inner.is_woken = true;
                    waiter_inner.result = Some(waiter_result);
                    true
                } else {
                    false
                }
            });
            if waiter_i.is_none() {
                return 0;
            }
            waiter_i.unwrap()
        };
        let del_waiter = waiters.swap_remove(del_waiter_i);
        set_event(del_waiter.thread);
        1
    }
}

fn wait_event(thread: *const c_void) {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_wait_untrusted_event_ocall(&mut ret as *mut c_int, thread);
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: OCall failed!");
    }
}

fn set_event(thread: *const c_void) {
    let mut ret: c_int = 0;
    let mut sgx_ret: c_int = 0;
    unsafe {
        sgx_ret = sgx_thread_set_untrusted_event_ocall(&mut ret as *mut c_int, thread);
    }
    if ret != 0 || sgx_ret != 0 {
        panic!("ERROR: OCall failed!");
    }
}

extern "C" {
    fn sgx_thread_get_self() -> *const c_void;

    /* Go outside and wait on my untrusted event */
    fn sgx_thread_wait_untrusted_event_ocall(ret: *mut c_int, self_thread: *const c_void) -> c_int;

    /* Wake a thread waiting on its untrusted event */
    fn sgx_thread_set_untrusted_event_ocall(ret: *mut c_int, waiter_thread: *const c_void)
        -> c_int;

    /* Wake a thread waiting on its untrusted event, and wait on my untrusted event */
    fn sgx_thread_setwait_untrusted_events_ocall(
        ret: *mut c_int,
        waiter_thread: *const c_void,
        self_thread: *const c_void,
    ) -> c_int;

    /* Wake multiple threads waiting on their untrusted events */
    fn sgx_thread_set_multiple_untrusted_events_ocall(
        ret: *mut c_int,
        waiter_threads: *const *const c_void,
        total: size_t,
    ) -> c_int;
}

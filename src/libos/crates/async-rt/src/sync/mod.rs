use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use errno::prelude::*;
use std::convert::{TryFrom, TryInto};
use std::hint;
use std::sync::atomic::Ordering;

mod mutex;
mod rwlock;

pub use mutex::{Mutex, MutexGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    const TEST_VPUS: u32 = 4;

    #[ctor::ctor]
    fn auto_init_executor() {
        crate::vcpu::set_total(TEST_VPUS);
    }

    #[test]
    fn test_mutex() {
        use crate::sync::Mutex;
        use crate::wait::Waiter;

        async fn test_lock() {
            let mutex = Arc::new(Mutex::new(1));
            info!("mutex = {:?}", mutex);
            let c_mutex = mutex.clone();
            let waiter = Waiter::new();
            let waker = waiter.waker();
            let mut duration = Duration::from_millis(300);

            crate::task::spawn(async move {
                let mut inner = c_mutex.lock().await;
                waker.wake().unwrap();
                let inner_waiter = Waiter::new();
                info!("child thread waiting: {:?}", inner);
                inner_waiter.wait_timeout(Some(&mut duration)).await;
                *inner = 2;
            });

            info!("Main thread running");
            waiter.wait().await.unwrap();
            info!("Main thread wake up");
            let mut inner = mutex.lock().await;
            assert!(*inner == 2);
            *inner = 3;
            drop(inner);

            let inner = mutex.lock().await;
            assert!(*inner == 3);
        }

        async fn test_lock_compete() {
            let counter = 1;
            let atomic_counter = Arc::new(AtomicU32::new(counter));
            let mutex = Arc::new(Mutex::new(counter));
            let waiter = Waiter::new();
            let waker = waiter.waker();
            let c_mutex = mutex.clone();
            let guard = mutex.lock().await;

            for i in 0..2 {
                let mutex = c_mutex.clone();
                let atomic_counter = atomic_counter.clone();
                let waker = waker.clone();
                crate::task::spawn(async move {
                    info!("spawn task {:?}", i);
                    let mut guard = mutex.lock().await;
                    info!("task {:?} got lock", i);
                    assert!(*guard == atomic_counter.load(Ordering::Acquire));
                    // update both counter
                    atomic_counter.fetch_add(1, Ordering::Release);
                    *guard += 1;

                    info!("task {:?} check done, counter = {:?}", i, *guard);
                    if *guard == 3 {
                        waker.wake();
                    }
                });
            }

            let tmp = *guard;
            drop(guard);
            info!("main thread drop lock");

            // wait for other threads to finish
            waiter.wait().await;

            let guard = mutex.lock().await;
            info!("main thread got lock");
            assert!(*guard == atomic_counter.load(Ordering::Acquire));
            assert!(*guard == tmp + 2);
        }

        async fn test_hold_lock() {
            let mutex = Arc::new(Mutex::new(1));
            let c_mutex = mutex.clone();

            let n = mutex.lock().await;
            assert_eq!(*n, 1);

            crate::task::spawn(async move {
                // Can't get lock in another task
                assert!(c_mutex.try_lock().is_err());
            })
            .await;

            assert_eq!(*n, 1);
            drop(n);
        }

        async fn test_into_inner() {
            let t_mutex_a = Mutex::new(1);
            let mut t_mutex_b = Mutex::new(1);

            assert!(t_mutex_a.into_inner() == 1);
            let mut_inner = t_mutex_b.get_mut();
            *mut_inner = 2;
            assert!(*t_mutex_b.lock().await == 2);
            assert!(t_mutex_b.into_inner() == 2);
        }

        crate::task::block_on(test_lock());
        crate::task::block_on(test_lock_compete());
        crate::task::block_on(test_hold_lock());
        crate::task::block_on(test_into_inner());
    }

    #[test]
    fn test_rwlock() {
        use crate::sync::RwLock;
        use crate::wait::Waiter;

        async fn test_share_read() {
            let lock = Arc::new(RwLock::new(1));
            let c_lock = lock.clone();

            let n = lock.read().await;
            assert_eq!(*n, 1);

            crate::task::spawn(async move {
                // While main has an active read lock, we acquire one too.
                let r = c_lock.read().await;
                assert_eq!(*r, 1);
            })
            .await;

            // Drop the guard after the spawned task finishes.
            drop(n);
        }

        async fn test_write() {
            let lock = Arc::new(RwLock::new(1));
            let c_lock = lock.clone();

            let n = lock.write().await;
            assert_eq!(*n, 1);

            crate::task::spawn(async move {
                // Can't get lock in another task
                assert!(c_lock.try_write().is_err());
                assert!(c_lock.try_read().is_err());
            })
            .await;

            assert_eq!(*n, 1);
            drop(n);
        }

        async fn test_wake_readers() {
            let rwlock = Arc::new(RwLock::new(1));
            let c_rwlock = rwlock.clone();
            let counter = Arc::new(AtomicU32::new(0));

            let mut n = rwlock.write().await;

            for i in 0..3 {
                let rwlock = c_rwlock.clone();
                let counter = counter.clone();
                crate::task::spawn(async move {
                    let n = rwlock.read().await;
                    assert_eq!(*n, 2);
                    counter.fetch_add(1, Ordering::Relaxed);
                });
            }

            // hold the lock for a while
            let waiter = Waiter::new();
            let mut duration = Duration::from_millis(300);
            waiter.wait_timeout(Some(&mut duration)).await;

            *n = 2;
            drop(n);

            loop {
                // All three tasks should be woken up.
                if counter.load(Ordering::Relaxed) == 3 {
                    break;
                }
                crate::scheduler::yield_now().await;
            }
        }

        async fn test_into_inner() {
            let rwlock_a = RwLock::new(1);
            let mut rwlock_b = RwLock::new(1);

            assert!(rwlock_a.into_inner() == 1);
            let mut_inner = rwlock_b.get_mut();
            *mut_inner = 2;
            assert!(*rwlock_b.read().await == 2);
            assert!(rwlock_b.into_inner() == 2);
        }

        crate::task::block_on(test_share_read());
        crate::task::block_on(test_write());
        crate::task::block_on(test_wake_readers());
        crate::task::block_on(test_into_inner());
    }
}

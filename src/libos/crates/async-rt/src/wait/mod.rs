mod macros;
mod waiter;
mod waiter_queue;

pub use self::macros::AutoWaiter;
pub use self::waiter::{Waiter, Waker};
pub use self::waiter_queue::WaiterQueue;

#[cfg(test)]
mod tests {
    use self::queue::Queue;
    use super::*;
    use crate::prelude::*;

    mod queue {
        use super::*;
        use std::collections::VecDeque;
        use std::sync::Mutex;

        pub struct Queue<T> {
            buf: Mutex<VecDeque<T>>,
            consumers: WaiterQueue,
            producers: WaiterQueue,
        }

        impl<T: Copy> Queue<T> {
            pub fn with_capacity(capacity: usize) -> Self {
                assert!(capacity > 0);
                Self {
                    buf: Mutex::new(VecDeque::with_capacity(capacity)),
                    consumers: WaiterQueue::new(),
                    producers: WaiterQueue::new(),
                }
            }

            pub async fn produce(&self, item: T, mut timeout: Option<&mut Duration>) -> Result<()> {
                crate::waiter_loop!(&self.producers, timeout, {
                    let mut buf = self.buf.lock().unwrap();
                    if buf.len() < buf.capacity() {
                        buf.push_back(item);
                        drop(buf);

                        self.consumers.wake_one();
                        return Ok(());
                    }
                })
            }

            pub async fn consume(&self, mut timeout: Option<&mut Duration>) -> Result<T> {
                crate::waiter_loop!(&self.consumers, timeout, {
                    let mut buf = self.buf.lock().unwrap();
                    if buf.len() > 0 {
                        let item = buf.pop_front().unwrap();
                        drop(buf);

                        self.producers.wake_one();
                        return Ok(item);
                    }
                })
            }

            pub fn len(&self) -> usize {
                let buf = self.buf.lock().unwrap();
                buf.len()
            }

            pub fn capacity(&self) -> usize {
                let buf = self.buf.lock().unwrap();
                buf.capacity()
            }
        }
    }

    #[test]
    fn produce_and_consume() {
        crate::task::block_on(async {
            const QUEUE_LEN: usize = 4;
            let queue = Arc::new(Queue::<usize>::with_capacity(QUEUE_LEN));
            let num_items: usize = 1024;

            let producer_task = {
                let queue = queue.clone();
                crate::task::spawn(async move {
                    let mut timeout = Some(Duration::from_millis(500));
                    for i in 0..num_items {
                        queue.produce(i, timeout.as_mut()).await.unwrap();
                    }
                })
            };
            let consumer_task = {
                let queue = queue.clone();
                crate::task::spawn(async move {
                    for i in 0..num_items {
                        assert!(queue.consume(None).await.unwrap() == i);
                    }
                })
            };

            producer_task.await;
            consumer_task.await;
        });
    }

    #[test]
    fn wait_timeout_err() {
        crate::task::block_on(async {
            let ms = 100;
            let mut timeout = Duration::from_millis(ms);
            let start = std::time::Instant::now();
            imagined_blocking_func1(Some(&mut timeout)).await;
            assert!(timeout.is_zero());
            assert!(start.elapsed().as_millis() as u64 >= ms - 1);

            let mut timeout = Duration::from_millis(ms);
            let start = std::time::Instant::now();
            imagined_blocking_func2(Some(&mut timeout)).await;
            assert!(timeout.is_zero());
            assert!(start.elapsed().as_millis() as u64 >= ms - 1);

            // case: timeout less than 1ms.
            let mut timeout = Duration::from_nanos(10);
            let start = std::time::Instant::now();
            imagined_blocking_func1(Some(&mut timeout)).await;
            assert!(timeout.is_zero());
            assert!(start.elapsed().as_millis() < 2);
        });
    }

    #[test]
    fn wait_timeout_ok() {
        crate::task::block_on(async {
            let waiter = Waiter::new();
            let waker = waiter.waker();

            let sleep_time = 10;
            let timeout_time = 100;
            let join_handle = crate::task::spawn(async move {
                let mut timeout = Duration::from_millis(timeout_time);
                assert!(waiter.wait_timeout(Some(&mut timeout)).await.is_ok());
            });

            crate::sched::yield_().await;
            std::thread::sleep(Duration::from_millis(sleep_time));
            waker.wake();
            join_handle.await;
        });
    }

    #[test]
    fn wait_none() {
        crate::task::block_on(async {
            let waiter = Waiter::new();
            let waker = waiter.waker();

            let join_handle = crate::task::spawn(async move {
                waiter.wait_timeout::<Duration>(None).await.unwrap();
            });

            crate::sched::yield_().await;
            waker.wake();
            join_handle.await;
        });
    }

    async fn imagined_blocking_func1(timeout: Option<&mut Duration>) {
        assert!(timeout.is_some());

        let waiter = Waiter::new();
        let res = waiter.wait_timeout(timeout).await;

        // timeout expired.
        assert!(res.is_err());
    }

    async fn imagined_blocking_func2(timeout: Option<&mut Duration>) {
        assert!(timeout.is_some());
        let mut timeout = timeout.map_or(None, |t| Some(t));

        let waiter = Waiter::new();
        loop {
            if let Err(_) = waiter.wait_timeout(timeout.as_mut()).await {
                // timeout expired, return.
                break;
            }
        }
    }
}

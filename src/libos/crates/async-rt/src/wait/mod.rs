mod macros;
mod waiter;
mod waiter_queue;

pub use self::macros::AutoWaiter;
pub use self::waiter::Waiter;
pub use self::waiter_queue::WaiterQueue;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use self::queue::Queue;
    use super::*;

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

            pub async fn produce(&self, item: T) {
                crate::waiter_loop!(&self.producers, {
                    let mut buf = self.buf.lock().unwrap();
                    if buf.len() < buf.capacity() {
                        buf.push_back(item);
                        drop(buf);

                        self.consumers.wake_one();
                        return;
                    }
                });
            }

            pub async fn consume(&self) -> T {
                crate::waiter_loop!(&self.consumers, {
                    let mut buf = self.buf.lock().unwrap();
                    if buf.len() > 0 {
                        let item = buf.pop_front().unwrap();
                        drop(buf);

                        self.producers.wake_one();
                        return item;
                    }
                });
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
                    for i in 0..num_items {
                        queue.produce(i).await;
                    }
                })
            };
            let consumer_task = {
                let queue = queue.clone();
                crate::task::spawn(async move {
                    for i in 0..num_items {
                        assert!(queue.consume().await == i);
                    }
                })
            };

            producer_task.await;
            consumer_task.await;
        });
    }
}

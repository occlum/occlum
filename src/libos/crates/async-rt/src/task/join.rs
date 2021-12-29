use alloc::sync::Weak;
use core::marker::PhantomData;
use core::task::Waker;

use crate::prelude::*;
use crate::task::Task;

#[derive(Debug)]
pub struct JoinHandle<T: Send + 'static> {
    state: Arc<Mutex<JoinState<T>>>,
    task: Arc<Task>,
    phantom: PhantomData<T>,
}

impl<T: Send + 'static> JoinHandle<T> {
    pub(crate) fn new(state: Arc<Mutex<JoinState<T>>>, task: Arc<Task>) -> Self {
        Self {
            state: state,
            task: task,
            phantom: PhantomData,
        }
    }

    pub fn task(&self) -> &Arc<Task> {
        &self.task
    }
}

impl<T: Send + 'static> Unpin for JoinHandle<T> {}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();
        if let Some(output) = state.take_output(cx) {
            Poll::Ready(output)
        } else {
            Poll::Pending
        }
    }
}

pub struct OutputHandle<T: Send + 'static> {
    state: Weak<Mutex<JoinState<T>>>,
    phantom: PhantomData<T>,
}

impl<T: Send + 'static> OutputHandle<T> {
    pub fn new(state: &Arc<Mutex<JoinState<T>>>) -> Self {
        Self {
            state: Arc::downgrade(state),
            phantom: PhantomData,
        }
    }

    pub fn set(self, output: T) {
        if let Some(state) = self.state.upgrade() {
            let mut state = state.lock();
            state.set_output(output);
        }
    }
}

// The state of a task that is to be joined.
#[derive(Debug)]
pub enum JoinState<T: Send + 'static> {
    Init,
    Pending(Waker),
    Ready(T),
    Finish,
}

impl<T: Send + 'static> JoinState<T> {
    pub fn new() -> Self {
        JoinState::Init
    }

    pub fn set_output(&mut self, value: T) {
        *self = match self {
            JoinState::Init => JoinState::Ready(value),
            JoinState::Pending(waker) => {
                waker.wake_by_ref();
                JoinState::Ready(value)
            }
            JoinState::Ready(_) | JoinState::Finish => {
                panic!("a task's output must not be set twice");
            }
        };
    }

    pub fn take_output(&mut self, cx: &mut Context<'_>) -> Option<T> {
        match self {
            JoinState::Init | JoinState::Pending(_) => {
                *self = JoinState::Pending(cx.waker().clone());
                None
            }
            JoinState::Ready(value) => {
                drop(value);
                let mut result = JoinState::Finish;
                core::mem::swap(self, &mut result);
                if let JoinState::Ready(value) = result {
                    Some(value)
                } else {
                    unreachable!();
                }
            }
            JoinState::Finish => {
                panic!("a task's output must not be taken again");
            }
        }
    }
}
